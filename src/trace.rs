//! Opt-in record of where inferred types come from (`tyda trace`).
//!
//! Recording is off by default. Every hook checks [`enabled`] first, so a normal run pays
//! one relaxed atomic load per hook and keeps no extra state. When enabled, hooks push
//! evidence (call sites, ivar writes, and method bodies before resolution) into one
//! process-wide store, and [`write_jsonl`] joins it with the final registry into one JSON
//! record per output declaration.

pub mod query;

use std::cell::RefCell;
use std::io::{self, Write};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::{Value, json};

use crate::registry::{CallSite, ClassData, TypeRegistry};
use crate::types::{MethodSig, RecordKey, SharedPath, Sym, Type};

static ENABLED: AtomicBool = AtomicBool::new(false);
static STORE: Mutex<Option<TraceStore>> = Mutex::new(None);

/// Where the statement or call being inferred is: file, 1-based line, enclosing method id.
#[derive(Clone)]
struct SiteInfo {
    file: Option<SharedPath>,
    line: u32,
    method: Option<String>,
}

type Site = Option<SiteInfo>;

thread_local! {
    static SITE: RefCell<Site> = const { RefCell::new(None) };
}

/// Starts recording for the rest of the process.
pub fn enable() {
    *lock_store() = Some(TraceStore::default());
    ENABLED.store(true, Ordering::Relaxed);
}

#[inline]
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

fn lock_store() -> std::sync::MutexGuard<'static, Option<TraceStore>> {
    STORE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn with_store(f: impl FnOnce(&mut TraceStore)) {
    if let Some(store) = lock_store().as_mut() {
        f(store);
    }
}

/// Restores the outer site on drop, so a nested call reports its own line.
pub struct SiteGuard(Site);

impl Drop for SiteGuard {
    fn drop(&mut self) {
        let outer = self.0.take();
        SITE.with(|cell| *cell.borrow_mut() = outer);
    }
}

/// Marks evidence recorded until the guard drops as coming from `file:line` in `method`.
pub fn enter_site(file: Option<&SharedPath>, line: u32, method: Option<String>) -> SiteGuard {
    let site = SiteInfo {
        file: file.cloned(),
        line,
        method,
    };
    SiteGuard(SITE.with(|cell| cell.replace(Some(site))))
}

/// Evidence is recorded only while inferring source, not while merging registries.
fn current_site() -> Site {
    SITE.with(|cell| cell.borrow().clone())
}

/// (owner, method name, singleton)
type MethodKey = (Sym, Sym, bool);

/// Call sites, and their argument types, by the method they dispatch to.
type CallSitesByOwner<'a> = FxHashMap<MethodKey, Vec<&'a FxHashSet<Evidence>>>;
type CallTypesByOwner<'a> = FxHashMap<MethodKey, Vec<&'a FxHashSet<(Option<String>, Type)>>>;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Evidence {
    file: Option<String>,
    line: u32,
    method: Option<String>,
    text: String,
}

impl Evidence {
    fn at(site: SiteInfo, text: String) -> Self {
        Self {
            file: site.file.map(|file| file.to_string()),
            line: site.line,
            method: site.method,
            text,
        }
    }
}

#[derive(Default)]
struct TraceStore {
    /// Call sites by receiver class and method name.
    call_sites: FxHashMap<MethodKey, FxHashSet<Evidence>>,
    /// Argument and block types of those call sites that still hold refs, with the
    /// calling method: the callee's parameter and block types come from them.
    call_types: FxHashMap<MethodKey, FxHashSet<(Option<String>, Type)>>,
    /// Ivar writes by (owner, ivar, singleton).
    ivar_writes: FxHashMap<MethodKey, FxHashSet<Evidence>>,
    /// Source method returns as the bodies left them, before global resolution.
    bodies: FxHashMap<MethodKey, Type>,
    /// Method returns read while inferring each method body, by the reading method's id.
    body_reads: FxHashMap<String, FxHashSet<(Sym, Sym)>>,
}

pub fn record_call_site(receiver_class: &str, site: &CallSite) {
    let Some(at) = current_site() else {
        return;
    };
    let key = (
        Sym::new(receiver_class),
        Sym::new(site.method_name.as_ref()),
        site.method_is_singleton,
    );
    let caller = at.method.clone();
    let typed: Vec<(Option<String>, Type)> = site
        .arg_types
        .iter()
        .chain(site.keyword_arg_types.values())
        .chain(site.block.iter().map(|block| &block.return_type))
        .filter(|ty| has_ref(ty))
        .map(|ty| (caller.clone(), ty.clone()))
        .collect();
    let evidence = Evidence::at(at, format_call_args(site));
    with_store(|store| {
        store.call_sites.entry(key).or_default().insert(evidence);
        if !typed.is_empty() {
            store.call_types.entry(key).or_default().extend(typed);
        }
    });
}

fn has_ref(ty: &Type) -> bool {
    match ty {
        Type::MethodReturnRef(..)
        | Type::ReceiverMethodRef(..)
        | Type::IvarRef(_)
        | Type::GlobalVariableRef(_) => true,
        Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => {
            parts.iter().any(has_ref)
        }
        Type::Generic { args, .. } => args.iter().any(has_ref),
        Type::Array(Some(inner))
        | Type::TruthyProjection(inner)
        | Type::FalsyProjection(inner)
        | Type::NonNilProjection(inner)
        | Type::SplatRef(inner)
        | Type::PatternIndexRef(inner, _)
        | Type::PatternRestRef(inner)
        | Type::PatternTrailingRef(inner, _)
        | Type::PatternKeyRef(inner, _)
        | Type::PatternKeyRestRef(inner, _) => has_ref(inner),
        Type::Hash(key, value) => [key, value].into_iter().flatten().any(|ty| has_ref(ty)),
        Type::DeferredSafeNavigation(receiver, value) => has_ref(receiver) || has_ref(value),
        Type::Record(fields) => fields.iter().any(|field| has_ref(&field.value)),
        _ => false,
    }
}

pub fn record_ivar_write(owner: &str, ivar: &str, singleton: bool, ty: &Type) {
    let Some(at) = current_site() else {
        return;
    };
    let key = (Sym::new(owner), Sym::new(ivar), singleton);
    let evidence = Evidence::at(at, symbolic(ty));
    with_store(|store| {
        store.ivar_writes.entry(key).or_default().insert(evidence);
    });
}

pub fn record_bodies(bodies: impl IntoIterator<Item = (Sym, Sym, bool, Type)>) {
    with_store(|store| {
        for (owner, method, singleton, ty) in bodies {
            store.bodies.entry((owner, method, singleton)).or_insert(ty);
        }
    });
}

/// A method return read while inferring a body. Inference may already know the callee's
/// type, so the body need not keep a reference to it.
pub fn record_body_read(owner: Sym, method: Sym) {
    let Some(reader) =
        SITE.with(|cell| cell.borrow().as_ref().and_then(|site| site.method.clone()))
    else {
        return;
    };
    with_store(|store| {
        store
            .body_reads
            .entry(reader)
            .or_default()
            .insert((owner, method));
    });
}

fn format_call_args(site: &CallSite) -> String {
    let mut parts: Vec<String> = site.arg_types.iter().map(symbolic).collect();
    for (name, ty) in site.keyword_arg_types.iter() {
        parts.push(format!("{name}: {}", symbolic(ty)));
    }
    let mut text = format!("({})", parts.join(", "));
    if let Some(block) = &site.block {
        text.push_str(&format!(" {{ -> {} }}", symbolic(&block.return_type)));
    }
    text
}

/// Like `Display`, but keeps unresolved references visible instead of printing `untyped`.
pub fn symbolic(ty: &Type) -> String {
    let mut out = String::new();
    write_symbolic(ty, &mut out);
    out
}

fn write_symbolic(ty: &Type, out: &mut String) {
    let join = |parts: &[Type], sep: &str, out: &mut String| {
        for (index, part) in parts.iter().enumerate() {
            if index > 0 {
                out.push_str(sep);
            }
            write_symbolic(part, out);
        }
    };
    match ty {
        Type::ParamRef(index) => out.push_str(&format!("<param {index}>")),
        Type::KeywordParamRef(name) => out.push_str(&format!("<param {name}:>")),
        Type::IvarRef(name) => out.push_str(&format!("<{name}>")),
        Type::GlobalVariableRef(name) => out.push_str(&format!("<{name}>")),
        Type::MethodReturnRef(owner, method) => out.push_str(&format!("<{owner}#{method}>")),
        Type::ReceiverMethodRef(receiver, method) => {
            out.push('<');
            write_symbolic(receiver, out);
            out.push_str(&format!(".{method}>"));
        }
        Type::BlockReturnRef => out.push_str("<block return>"),
        Type::DeferredSafeNavigation(receiver, value) => {
            out.push_str("<&. ");
            write_symbolic(receiver, out);
            out.push_str(" => ");
            write_symbolic(value, out);
            out.push('>');
        }
        Type::TruthyProjection(inner) => projection("truthy", inner, out),
        Type::FalsyProjection(inner) => projection("falsy", inner, out),
        Type::NonNilProjection(inner) => projection("non-nil", inner, out),
        Type::PatternIndexRef(subject, index) => pattern(subject, &index.to_string(), out),
        Type::PatternRestRef(subject) => pattern(subject, "*", out),
        Type::PatternTrailingRef(subject, index) => {
            pattern(subject, &format!("-{}", index + 1), out)
        }
        Type::PatternKeyRef(subject, key) => {
            let key = match key.as_ref() {
                RecordKey::Symbol(name) => format!(":{name}"),
                RecordKey::String(name) => format!("{name:?}"),
            };
            pattern(subject, &key, out)
        }
        Type::PatternKeyRestRef(subject, _) => pattern(subject, "**", out),
        Type::Union(parts) => join(parts, " | ", out),
        Type::Intersection(parts) => join(parts, " & ", out),
        Type::Tuple(parts) => {
            out.push('[');
            join(parts, ", ", out);
            out.push(']');
        }
        Type::Array(Some(elem)) => {
            out.push_str("Array[");
            write_symbolic(elem, out);
            out.push(']');
        }
        Type::Hash(Some(key), Some(value)) => {
            out.push_str("Hash[");
            write_symbolic(key, out);
            out.push_str(", ");
            write_symbolic(value, out);
            out.push(']');
        }
        Type::Generic { base, args } => {
            out.push_str(base.as_str());
            out.push('[');
            join(args, ", ", out);
            out.push(']');
        }
        Type::Record(fields) => {
            out.push('{');
            for (index, field) in fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                match &field.key {
                    RecordKey::Symbol(name) => out.push_str(&format!("{name}: ")),
                    RecordKey::String(name) => out.push_str(&format!("{name:?} => ")),
                }
                write_symbolic(&field.value, out);
            }
            out.push('}');
        }
        _ => out.push_str(&ty.to_string()),
    }
}

/// A destructured part of `subject`, e.g. `<<param 0>.split>[1]>` for `_, x = a.split`.
fn pattern(subject: &Type, part: &str, out: &mut String) {
    out.push('<');
    write_symbolic(subject, out);
    out.push_str(&format!("[{part}]>"));
}

fn projection(name: &str, inner: &Type, out: &mut String) {
    out.push_str(&format!("<{name} "));
    write_symbolic(inner, out);
    out.push('>');
}

pub fn method_id(owner: &str, method: &str, singleton: bool) -> String {
    if singleton {
        format!("{owner}.{method}")
    } else {
        format!("{owner}#{method}")
    }
}

/// `Foo#bar` / `Foo.bar` to (`Foo`, singleton).
fn split_method_id(id: &str) -> Option<(&str, bool)> {
    id.rsplit_once('#')
        .map(|(owner, _)| (owner, false))
        .or_else(|| id.rsplit_once('.').map(|(owner, _)| (owner, true)))
}

pub fn ivar_id(owner: &str, ivar: &str, singleton: bool) -> String {
    if singleton {
        format!("{owner}.{ivar}")
    } else {
        format!("{owner}{ivar}")
    }
}

fn constant_id(owner: &str, name: &str) -> String {
    if owner == "Object" {
        name.to_string()
    } else {
        format!("{owner}::{name}")
    }
}

/// Writes one record with `id` first, so each line starts with the declaration it describes
/// (the other keys keep serde_json's sorted order).
fn write_line<W: Write>(writer: &mut W, id: &str, mut record: Value) -> io::Result<()> {
    if let Some(object) = record.as_object_mut() {
        object.remove("id");
    }
    let rest = serde_json::to_string(&record)?;
    writer.write_all(b"{\"id\":")?;
    serde_json::to_writer(&mut *writer, id)?;
    match rest.strip_prefix('{').filter(|rest| *rest != "}") {
        Some(rest) => {
            writer.write_all(b",")?;
            writer.write_all(rest.as_bytes())?;
        }
        None => writer.write_all(b"}")?,
    }
    writer.write_all(b"\n")
}

fn evidence_json(evidence: &FxHashSet<Evidence>) -> Vec<Value> {
    let mut sorted: Vec<&Evidence> = evidence.iter().collect();
    sorted.sort();
    sorted
        .into_iter()
        .map(|evidence| {
            json!({
                "file": evidence.file,
                "line": (evidence.line > 0).then_some(evidence.line),
                "in": evidence.method,
                "type": evidence.text,
            })
        })
        .collect()
}

fn sorted_dedup(mut ids: Vec<String>, own_id: &str) -> Vec<String> {
    ids.retain(|id| id != own_id);
    ids.sort();
    ids.dedup();
    ids
}

/// Writes one JSON object per rendered method, ivar, and constant, then one per
/// non-rendered method that a rendered declaration depends on.
pub fn write_jsonl<W: Write>(registry: &TypeRegistry, writer: &mut W) -> io::Result<()> {
    let store = lock_store().take().unwrap_or_default();
    let dispatch_owner = |(receiver, method, singleton): &MethodKey| -> MethodKey {
        let owner = registry
            .first_method_call_owner_cached(receiver.as_str(), method.as_str(), *singleton)
            .map_or(*receiver, |(owner, _)| Sym::new(owner.as_ref()));
        (owner, *method, *singleton)
    };
    let mut call_sites_by_owner = CallSitesByOwner::default();
    for (key, evidence) in &store.call_sites {
        call_sites_by_owner
            .entry(dispatch_owner(key))
            .or_default()
            .push(evidence);
    }
    let mut call_types_by_owner = CallTypesByOwner::default();
    for (key, types) in &store.call_types {
        call_types_by_owner
            .entry(dispatch_owner(key))
            .or_default()
            .push(types);
    }
    let mut emitter = Emitter {
        registry,
        store: &store,
        call_sites_by_owner,
        call_types_by_owner,
        emitted: FxHashSet::default(),
        referenced: Vec::new(),
    };
    for constant in registry.build_output_top_level_constants() {
        emitter.write_constant("Object", &constant, writer)?;
    }
    for class_name in registry.output_class_names() {
        let Some(class) = registry.build_output_class_info(class_name.as_str()) else {
            continue;
        };
        let Some(data) = registry.class_data_for(class_name.as_str()) else {
            continue;
        };
        if class.name != "Object" {
            for constant in &class.constants {
                emitter.write_constant(&class.name, constant, writer)?;
            }
        }
        // The renderer keeps the last of same-named declarations, in declaration order.
        let mut seen = FxHashSet::default();
        let mut methods: Vec<&MethodSig> = class
            .methods
            .iter()
            .rev()
            .filter(|method| crate::rbs::render::is_rendered_method(method))
            .filter(|method| seen.insert((method.name.as_str(), method.is_singleton)))
            .collect();
        methods.reverse();
        for method in methods {
            emitter.write_method(&class.name, data, method, writer)?;
        }
        emitter.write_ivars(&class.name, data, writer)?;
    }
    emitter.write_referenced(writer)
}

struct Emitter<'a> {
    registry: &'a TypeRegistry,
    store: &'a TraceStore,
    call_sites_by_owner: CallSitesByOwner<'a>,
    call_types_by_owner: CallTypesByOwner<'a>,
    emitted: FxHashSet<String>,
    referenced: Vec<String>,
}

impl Emitter<'_> {
    /// Declarations a type still refers to. A method on a receiver known only after resolution is
    /// named by the receiver's resolved class, and every method by the class defining it.
    fn collect_refs(&self, owner: &str, singleton: bool, ty: &Type, refs: &mut Vec<String>) {
        match ty {
            Type::MethodReturnRef(class, method) => {
                refs.push(self.method_ref(class.as_str(), method.as_str(), None));
            }
            Type::ReceiverMethodRef(receiver, method) => {
                let resolved = self
                    .registry
                    .resolve_deferred_refs_for_context(owner, singleton, receiver);
                let receivers = match &resolved {
                    Type::Union(parts) => parts.as_slice(),
                    other => std::slice::from_ref(other),
                };
                for receiver in receivers {
                    match receiver {
                        Type::Class(class) | Type::Generic { base: class, .. } => {
                            refs.push(self.method_ref(
                                class.as_str(),
                                method.as_str(),
                                Some(false),
                            ));
                        }
                        Type::Singleton(class) => {
                            refs.push(self.method_ref(class.as_str(), method.as_str(), Some(true)));
                        }
                        Type::SelfType | Type::InstanceType => {
                            refs.push(self.method_ref(owner, method.as_str(), Some(singleton)));
                        }
                        _ => {}
                    }
                }
                self.collect_refs(owner, singleton, receiver, refs);
            }
            Type::IvarRef(name) => refs.push(ivar_id(owner, name.as_str(), singleton)),
            Type::GlobalVariableRef(name) => refs.push(name.to_string()),
            Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => {
                for part in parts {
                    self.collect_refs(owner, singleton, part, refs);
                }
            }
            Type::Generic { args, .. } => {
                for arg in args.iter() {
                    self.collect_refs(owner, singleton, arg, refs);
                }
            }
            Type::Array(Some(inner))
            | Type::TruthyProjection(inner)
            | Type::FalsyProjection(inner)
            | Type::NonNilProjection(inner)
            | Type::SplatRef(inner)
            | Type::PatternIndexRef(inner, _)
            | Type::PatternRestRef(inner)
            | Type::PatternTrailingRef(inner, _)
            | Type::PatternKeyRef(inner, _)
            | Type::PatternKeyRestRef(inner, _) => self.collect_refs(owner, singleton, inner, refs),
            Type::Hash(key, value) => {
                for inner in [key, value].into_iter().flatten() {
                    self.collect_refs(owner, singleton, inner, refs);
                }
            }
            Type::DeferredSafeNavigation(receiver, value) => {
                self.collect_refs(owner, singleton, receiver, refs);
                self.collect_refs(owner, singleton, value, refs);
            }
            Type::Record(fields) => {
                for field in fields {
                    self.collect_refs(owner, singleton, &field.value, refs);
                }
            }
            _ => {}
        }
    }

    /// The ivar's type as methods read it. A parameter written in `initialize` takes the
    /// type its callers pass, as an attr reader of it renders.
    fn ivar_type(&self, owner: &str, ivar: &str, singleton: bool) -> Option<Type> {
        if singleton {
            return self.registry.lookup_singleton_ivar_type(owner, ivar);
        }
        let stored = self.registry.lookup_ivar_type(owner, ivar);
        match stored {
            None | Some(Type::Untyped | Type::ParamRef(_) | Type::KeywordParamRef(_)) => self
                .registry
                .infer_attr_type_from_initialize(owner, ivar)
                .or(stored),
            Some(ty) => Some(ty),
        }
    }

    /// `singleton: None` takes the instance method when both variants exist.
    fn method_ref(&self, class: &str, method: &str, singleton: Option<bool>) -> String {
        let variants: &[bool] = match singleton {
            Some(false) => &[false],
            Some(true) => &[true],
            None => &[false, true],
        };
        for &variant in variants {
            if let Some((owner, owner_singleton)) = self
                .registry
                .first_method_call_owner_cached(class, method, variant)
            {
                return method_id(&owner, method, owner_singleton);
            }
        }
        method_id(class, method, singleton.unwrap_or(false))
    }

    fn write_record<W: Write>(
        &mut self,
        id: String,
        record: Value,
        writer: &mut W,
    ) -> io::Result<()> {
        if let Some(deps) = record.get("deps").and_then(Value::as_array) {
            self.referenced
                .extend(deps.iter().filter_map(Value::as_str).map(str::to_string));
        }
        write_line(writer, &id, record)?;
        self.emitted.insert(id);
        Ok(())
    }

    fn write_constant<W: Write>(
        &mut self,
        owner: &str,
        constant: &crate::types::ConstantSig,
        writer: &mut W,
    ) -> io::Result<()> {
        let id = constant_id(owner, &constant.name);
        let record = json!({
            "kind": "constant",
            "id": id,
            "type": crate::rbs::render::constant_decl_line(constant),
            "file": constant.file_path,
            "line": constant.loc.map(|loc| loc.line),
        });
        self.write_record(id, record, writer)
    }

    fn method_file(&self, data: &ClassData, name: &str, singleton: bool) -> Option<String> {
        data.method_file_paths
            .get(&(Sym::new(name), singleton))
            .or(data.file_path.as_ref())
            .map(|path| path.to_string())
    }

    fn write_method<W: Write>(
        &mut self,
        owner: &str,
        data: &ClassData,
        method: &MethodSig,
        writer: &mut W,
    ) -> io::Result<()> {
        let id = method_id(owner, &method.name, method.is_singleton);
        let owner_sym = Sym::new(owner);
        let name_sym = Sym::new(&method.name);
        let key = (owner_sym, name_sym, method.is_singleton);
        let mut deps = Vec::new();
        let body = self.store.bodies.get(&key);
        if let Some(body) = body {
            self.collect_refs(owner, method.is_singleton, body, &mut deps);
            // Resolving the body again on the final registry names the methods it reads
            // through ancestors, call-site parameters, and attr readers.
            for (read_owner, read_method) in
                self.registry.return_type_reads(owner_sym, name_sym, body)
            {
                deps.push(self.method_ref(read_owner.as_str(), read_method.as_str(), None));
            }
        }
        if let Some(def) = self
            .registry
            .lookup_method_def(owner, &method.name, method.is_singleton)
            && let Some(ivar) = &def.attr_ivar
        {
            deps.push(ivar_id(owner, ivar, method.is_singleton));
        }
        if let Some(reads) = self.store.body_reads.get(&id) {
            for (read_owner, read_method) in reads {
                deps.push(self.method_ref(read_owner.as_str(), read_method.as_str(), None));
            }
        }
        for types in self.call_types_by_owner.get(&key).into_iter().flatten() {
            for (caller, ty) in types.iter() {
                let (caller_owner, caller_singleton) = caller
                    .as_deref()
                    .and_then(split_method_id)
                    .unwrap_or((owner, method.is_singleton));
                self.collect_refs(caller_owner, caller_singleton, ty, &mut deps);
            }
        }
        let callers = self.callers(owner, &method.name, method.is_singleton);
        // A caller passing its own parameter passes the type its callers give it.
        deps.extend(
            callers
                .iter()
                .filter(|caller| caller.text.contains("<param "))
                .filter_map(|caller| caller.method.clone()),
        );
        let deps = sorted_dedup(deps, &id);
        let record = json!({
            "kind": "method",
            "id": id,
            "type": crate::rbs::render::method_sig_line(method),
            "origin": method.origin.as_str(),
            "file": self.method_file(data, &method.name, method.is_singleton),
            "line": method.loc.map(|loc| loc.line),
            "body": body.map(symbolic),
            "deps": deps,
            "callers": evidence_json(&callers),
        });
        self.write_record(id, record, writer)
    }

    /// Call sites whose receiver dispatches `method` to `owner`.
    fn callers(&self, owner: &str, method: &str, singleton: bool) -> FxHashSet<Evidence> {
        self.call_sites_by_owner
            .get(&(Sym::new(owner), Sym::new(method), singleton))
            .into_iter()
            .flatten()
            .flat_map(|evidence| evidence.iter().cloned())
            .collect()
    }

    fn write_ivars<W: Write>(
        &mut self,
        owner: &str,
        data: &ClassData,
        writer: &mut W,
    ) -> io::Result<()> {
        let mut names: Vec<(Sym, bool)> = data.ivars.keys().map(|name| (*name, false)).collect();
        names.extend(
            self.store
                .ivar_writes
                .keys()
                .filter(|(ivar_owner, _, singleton)| ivar_owner.as_str() == owner && *singleton)
                .map(|(_, ivar, _)| (*ivar, true)),
        );
        names.sort();
        names.dedup();
        for (ivar, singleton) in names {
            let id = ivar_id(owner, ivar.as_str(), singleton);
            let ty = self.ivar_type(owner, ivar.as_str(), singleton);
            let writes = self
                .store
                .ivar_writes
                .get(&(Sym::new(owner), ivar, singleton))
                .cloned()
                .unwrap_or_default();
            let mut deps = Vec::new();
            if let Some(types) = data.ivars.get(&ivar).filter(|_| !singleton) {
                for ty in types {
                    self.collect_refs(owner, singleton, ty, &mut deps);
                }
            }
            // A write of a parameter takes its type from that method's callers.
            deps.extend(
                writes
                    .iter()
                    .filter(|write| write.text.contains("<param "))
                    .filter_map(|write| write.method.clone()),
            );
            let deps = sorted_dedup(deps, &id);
            let record = json!({
                "kind": "ivar",
                "id": id,
                "type": ty.as_ref().map(symbolic),
                "deps": deps,
                "writes": evidence_json(&writes),
            });
            self.write_record(id, record, writer)?;
        }
        Ok(())
    }

    /// Dependencies that are not rendered (external RBS, RBI, generated DSL, unresolved).
    fn write_referenced<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        let mut pending = std::mem::take(&mut self.referenced);
        pending.sort();
        pending.dedup();
        for id in pending {
            if self.emitted.contains(&id) {
                continue;
            }
            let record = self.referenced_record(&id);
            write_line(writer, &id, record)?;
            self.emitted.insert(id);
        }
        Ok(())
    }

    fn referenced_record(&self, id: &str) -> Value {
        let method = id
            .rsplit_once('#')
            .map(|(owner, name)| (owner, name, false))
            .or_else(|| {
                id.rsplit_once('.')
                    .filter(|(_, name)| !name.starts_with('@'))
                    .map(|(owner, name)| (owner, name, true))
            });
        if let Some((owner, name, singleton)) = method
            && let Some(def) = self.registry.lookup_method_def(owner, name, singleton)
        {
            let file = self
                .registry
                .class_data_for(owner)
                .and_then(|data| self.method_file(data, name, singleton));
            return json!({
                "kind": "method",
                "id": id,
                "type": symbolic(&def.raw_return_type),
                "origin": def.origin.as_str(),
                "file": file,
                "line": def.loc.map(|loc| loc.line),
                "rendered": false,
            });
        }
        json!({ "kind": "unresolved", "id": id })
    }
}
