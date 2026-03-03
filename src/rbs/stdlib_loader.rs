use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::project::RubyVersion;
use crate::rbs::bounded_file_cache::{BoundedFileCache, DEFAULT_FILE_CACHE_CAP};
use crate::rbs::import::load_rbs_string_with_dependency_aliases;
use crate::registry::{RegistryDeepBytes, TypeRegistry};

const MAX_ALIAS_DEPENDENCY_DEPTH: usize = 8;

type StdlibFileRegistrySlot = Arc<OnceLock<Arc<TypeRegistry>>>;
type SharedShapeSlot = Arc<OnceLock<Option<Arc<TypeRegistry>>>>;

fn stdlib_file_cache_cap() -> usize {
    std::env::var("TYDA_STDLIB_RBS_FILE_CACHE_CAP")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n: &usize| n > 0)
        .unwrap_or(DEFAULT_FILE_CACHE_CAP)
}

pub struct LazyRbsLoader {
    class_to_files: FxHashMap<String, Vec<PathBuf>>,
    file_cache: Mutex<BoundedFileCache<PathBuf, StdlibFileRegistrySlot>>,
    shared_shapes: Mutex<FxHashMap<String, SharedShapeSlot>>,
}

impl LazyRbsLoader {
    pub fn new(core_dir: PathBuf) -> Self {
        Self::from_rbs_roots(vec![core_dir])
    }

    pub fn for_ruby_version(vendor_rbs_root: PathBuf, ruby_version: RubyVersion) -> Self {
        Self::from_rbs_roots(resolve_versioned_rbs_roots(&vendor_rbs_root, ruby_version))
    }

    pub fn from_rbs_roots(roots: Vec<PathBuf>) -> Self {
        let mut class_to_files = FxHashMap::default();

        for root in roots {
            if !root.exists() {
                continue;
            }
            let is_stdlib_root = root
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "stdlib")
                || root
                    .parent()
                    .and_then(|p| p.file_name())
                    .is_some_and(|n| n == "stdlib");
            let rbs_files = collect_rbs_files(&[root]);
            for path in rbs_files {
                if let Some(stem) = path.file_stem().and_then(|segment| segment.to_str()) {
                    let class_name = stem_to_class_name(stem);
                    insert_class_file(&mut class_to_files, class_name, path.clone());
                }
                // Path suffix matching uses `/` so it remains stable on Windows.
                let normalized_path = normalize_path_for_matching(&path);
                if let Some(path_str) = normalized_path.as_deref()
                    && let Some(fq) = classify_rbs_path_to_class_name(path_str)
                {
                    insert_class_file(&mut class_to_files, fq, path.clone());
                }
                // Nested mixin modules aren't indexed by the stem convention, so link them explicitly (e.g. SecureRandom).
                if let Some(path_str) = normalized_path.as_deref()
                    && let Some(class_name) = classify_nested_mixin_rbs_path(path_str)
                {
                    insert_class_file(&mut class_to_files, class_name.to_string(), path.clone());
                }
                // stdlib often has stem != class name, so index by the leading declaration name (core skips this scan via the stem convention).
                if is_stdlib_root {
                    for declared in scan_top_level_declarations(&path) {
                        insert_class_file(&mut class_to_files, declared, path.clone());
                    }
                } else {
                    // Core aliases (`Mutex = Thread::Mutex`) can't use the stem convention -> lightly scan just the alias lines.
                    for declared in scan_top_level_class_aliases(&path) {
                        insert_class_file(&mut class_to_files, declared, path.clone());
                    }
                    // Qualified top-level nested classes (e.g. `File::Stat`) can't use the stem convention -> load via chained qualified scan.
                    for declared in scan_top_level_qualified_declarations(&path) {
                        insert_class_file(&mut class_to_files, declared, path.clone());
                    }
                }
            }
        }

        // errors.rbs only has the stem "Errors", so each Exception subclass isn't indexed; add them explicitly.
        let error_classes: &[&str] = &[
            "StandardError",
            "RuntimeError",
            "ArgumentError",
            "IndexError",
            "KeyError",
            "NameError",
            "NoMethodError",
            "TypeError",
            "ZeroDivisionError",
            "RangeError",
            "FloatDomainError",
            "StopIteration",
            "ClosedQueueError",
            "EncodingError",
            "EOFError",
            "FrozenError",
            "IOError",
            "Interrupt",
            "LoadError",
            "LocalJumpError",
            "NoMatchingPatternError",
            "NoMatchingPatternKeyError",
            "NoMemoryError",
            "NotImplementedError",
            "RegexpError",
            "ScriptError",
            "SecurityError",
            "SignalException",
            "SyntaxError",
            "SystemCallError",
            "SystemExit",
            "SystemStackError",
            "ThreadError",
            "UncaughtThrowError",
        ];
        if let Some(errors_path) = first_class_file(&class_to_files, "Errors").cloned() {
            for class_name in error_classes {
                insert_class_file(
                    &mut class_to_files,
                    class_name.to_string(),
                    errors_path.clone(),
                );
            }
        }

        Self {
            class_to_files,
            file_cache: Mutex::new(BoundedFileCache::with_cap(stdlib_file_cache_cap())),
            shared_shapes: Mutex::new(FxHashMap::default()),
        }
    }

    fn file_slot(&self, path: &Path) -> StdlibFileRegistrySlot {
        let mut cache = self.file_cache.lock().expect("stdlib file cache poisoned");
        if let Some(existing) = cache.get(&path.to_path_buf()) {
            return Arc::clone(existing);
        }
        let fresh: StdlibFileRegistrySlot = Arc::new(OnceLock::new());
        Arc::clone(cache.insert(path.to_path_buf(), fresh))
    }

    fn load_file_registry(&self, path: &Path) -> Arc<TypeRegistry> {
        let slot = self.file_slot(path);
        Arc::clone(slot.get_or_init(|| {
            let mut registry = TypeRegistry::new();
            if let Ok(content) = std::fs::read_to_string(path) {
                let mut visited = HashSet::from([path.to_path_buf()]);
                let mut dependency_contents = Vec::new();
                self.collect_alias_dependency_contents(
                    &content,
                    &mut visited,
                    0,
                    &mut dependency_contents,
                );
                load_rbs_string_with_dependency_aliases(
                    &content,
                    &dependency_contents,
                    &mut registry,
                );
            }
            // Shrinks the per-file OnceLock slot to remove excess capacity (content is unchanged).
            registry.shrink_to_fit_after_compact();
            Arc::new(registry)
        }))
    }

    fn collect_alias_dependency_contents(
        &self,
        content: &str,
        visited: &mut HashSet<PathBuf>,
        depth: usize,
        out: &mut Vec<String>,
    ) {
        if depth >= MAX_ALIAS_DEPENDENCY_DEPTH {
            return;
        }

        for path in self.alias_dependency_paths(content) {
            if !visited.insert(path.clone()) {
                continue;
            }
            let Ok(dependency_content) = std::fs::read_to_string(&path) else {
                continue;
            };
            self.collect_alias_dependency_contents(&dependency_content, visited, depth + 1, out);
            out.push(dependency_content);
        }
    }

    fn alias_dependency_paths(&self, content: &str) -> Vec<PathBuf> {
        let Ok(sig) = rbs_sys::parse_signature(content) else {
            return Vec::new();
        };
        let mut names = HashSet::new();
        collect_signature_type_names(&sig, &mut names);
        let mut paths: Vec<PathBuf> = names
            .into_iter()
            .filter_map(|name| self.dependency_path_for_type_name(&name))
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    fn dependency_path_for_type_name(&self, type_name: &str) -> Option<PathBuf> {
        let bare = type_name.trim_start_matches("::");
        if !bare.contains("::") {
            return None;
        }

        let mut cursor = bare;
        while let Some((owner, _)) = cursor.rsplit_once("::") {
            if let Some(path) = first_class_file(&self.class_to_files, owner) {
                return Some(path.clone());
            }
            cursor = owner;
        }
        None
    }

    pub fn merge_class_into(&self, class_name: &str, target: &mut TypeRegistry) -> bool {
        #[cfg(test)]
        merge_counter::record(class_name);
        let Some(shape) = self.shared_class_shape(class_name) else {
            return false;
        };
        target.merge_stdlib_rbs_registry(&shape);
        // Cross-file method aliases (`Kernel#send` → `BasicObject#__send__`) still
        // finalize against the engine once ancestors are present.
        crate::rbs::import::finalize_pending_method_aliases(target);
        true
    }

    fn shared_class_shape(&self, class_name: &str) -> Option<Arc<TypeRegistry>> {
        if !self.class_to_files.contains_key(class_name) {
            return None;
        }
        let slot: SharedShapeSlot = {
            let mut shapes = self
                .shared_shapes
                .lock()
                .expect("stdlib shared shapes poisoned");
            if let Some(existing) = shapes.get(class_name) {
                Arc::clone(existing)
            } else {
                let fresh: SharedShapeSlot = Arc::new(OnceLock::new());
                shapes.insert(class_name.to_string(), Arc::clone(&fresh));
                fresh
            }
        };
        slot.get_or_init(|| self.build_class_shape(class_name))
            .clone()
    }

    /// Merge every declaring `.rbs` file into an empty registry once per FQN.
    /// Alias expansion is done here so the cached shape does not depend on the
    /// caller's already-merged ancestors (byte-stable across per-file engines).
    fn build_class_shape(&self, class_name: &str) -> Option<Arc<TypeRegistry>> {
        #[cfg(test)]
        merge_counter::record_build(class_name);
        let paths = self.class_to_files.get(class_name)?;
        let mut shape = TypeRegistry::new();
        let mut merged = false;
        for path in paths {
            let registry = self.load_file_registry(path);
            if registry.class_data_for(class_name).is_some()
                || registry.type_aliases().contains_key(class_name)
            {
                shape.merge_stdlib_rbs_registry(&registry);
                merged = true;
            }
        }
        if !merged {
            return None;
        }
        crate::rbs::import::finalize_pending_method_aliases(&mut shape);
        shape.shrink_to_fit_after_compact();
        Some(Arc::new(shape))
    }

    /// stdlib namespaces safe for lazy loading (excludes `CORE_OWNED_CLASSES`, since re-merging them would reopen `Object` as a side effect).
    pub fn is_loadable_namespace_head(&self, head: &str) -> bool {
        !CORE_OWNED_CLASSES.contains(&head) && self.class_to_files.contains_key(head)
    }

    pub fn lookup_method(&self, class_name: &str, method_name: &str) -> Option<crate::types::Type> {
        self.class_to_files
            .get(class_name)?
            .iter()
            .find_map(|path| {
                self.load_file_registry(path)
                    .lookup_method_return_type(class_name, method_name)
            })
    }

    /// Iterates over parsed cache registries, for memory-breakdown attribution.
    /// Bench-only (runs the caller's closure while still holding the lock).
    #[cfg(test)]
    pub fn for_each_cached_registry(&self, mut f: impl FnMut(&TypeRegistry)) {
        let cache = self.file_cache.lock().expect("stdlib file cache poisoned");
        for slot in cache.values() {
            if let Some(registry) = slot.get() {
                f(registry);
            }
        }
    }

    /// Takes out the Arcs of parsed registries and empties the cache, for
    /// bisecting the memory-breakdown bench's stdlib cache attribution only.
    #[cfg(test)]
    pub fn debug_take_cached_registries(&self) -> Vec<Arc<TypeRegistry>> {
        let mut cache = self.file_cache.lock().expect("stdlib file cache poisoned");
        let taken: Vec<Arc<TypeRegistry>> = cache
            .values()
            .filter_map(|slot| slot.get().map(Arc::clone))
            .collect();
        cache.clear();
        taken
    }

    /// Drops `file_cache` once scanning is done (parse memos already merged into the workspace are dead weight).
    /// Canonical shapes stay: later `merge_class_into` copies the shared Arc, it does not re-parse.
    pub fn drop_file_cache(&self) {
        let mut cache = self.file_cache.lock().expect("stdlib file cache poisoned");
        cache.clear();
    }

    /// Deep byte attribution of both lazy caches plus the class index, for
    /// `TYDA_MEMORY_BREAKDOWN`. `seen` is shared with the caller's other walks so
    /// chunks reachable from several registries are charged exactly once.
    pub fn deep_breakdown(&self, seen: &mut FxHashSet<usize>) -> LazyRbsLoaderDeepBytes {
        let mut out = LazyRbsLoaderDeepBytes::default();
        {
            let shapes = self
                .shared_shapes
                .lock()
                .expect("stdlib shared shapes poisoned");
            out.shape_slot_count = shapes.len();
            for (class_name, slot) in shapes.iter() {
                out.index_bytes += class_name.capacity()
                    + std::mem::size_of::<String>()
                    + std::mem::size_of::<SharedShapeSlot>();
                if let Some(Some(registry)) = slot.get() {
                    out.shape_count += 1;
                    out.shapes.accumulate(&registry.deep_breakdown(seen));
                }
            }
        }
        {
            let cache = self.file_cache.lock().expect("stdlib file cache poisoned");
            for slot in cache.values() {
                if let Some(registry) = slot.get() {
                    out.file_cache_count += 1;
                    out.file_cache.accumulate(&registry.deep_breakdown(seen));
                }
            }
        }
        for (class_name, paths) in &self.class_to_files {
            out.index_bytes += class_name.capacity()
                + std::mem::size_of::<String>()
                + std::mem::size_of::<Vec<PathBuf>>()
                + paths.capacity() * std::mem::size_of::<PathBuf>()
                + paths
                    .iter()
                    .map(|path| path.as_os_str().len())
                    .sum::<usize>();
        }
        out
    }

    /// file_cache breakdown: counts only the initialized OnceLock slots.
    pub fn cache_breakdown(&self) -> StdlibCacheBreakdown {
        let cache = self.file_cache.lock().expect("stdlib file cache poisoned");
        let mut breakdown = StdlibCacheBreakdown::default();
        for slot in cache.values() {
            breakdown.slot_count += 1;
            let Some(registry) = slot.get() else {
                continue;
            };
            breakdown.parsed_file_count += 1;
            let totals = registry.breakdown_totals();
            breakdown.class_count += totals.class_count;
            breakdown.method_count += totals.method_count;
        }
        breakdown
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LazyRbsLoaderDeepBytes {
    pub shape_slot_count: usize,
    pub shape_count: usize,
    pub shapes: RegistryDeepBytes,
    pub file_cache_count: usize,
    pub file_cache: RegistryDeepBytes,
    pub index_bytes: usize,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StdlibCacheBreakdown {
    pub slot_count: usize,
    pub parsed_file_count: usize,
    pub class_count: usize,
    pub method_count: usize,
}

fn insert_class_file(
    class_to_files: &mut FxHashMap<String, Vec<PathBuf>>,
    class_name: String,
    path: PathBuf,
) {
    let files = class_to_files.entry(class_name).or_default();
    if !files.contains(&path) {
        files.push(path);
    }
}

fn first_class_file<'a>(
    class_to_files: &'a FxHashMap<String, Vec<PathBuf>>,
    class_name: &str,
) -> Option<&'a PathBuf> {
    class_to_files
        .get(class_name)
        .and_then(|files| files.first())
}

fn collect_signature_type_names(sig: &rbs_sys::Signature, names: &mut HashSet<String>) {
    for decl in &sig.declarations {
        match decl {
            rbs_sys::Declaration::Class {
                methods,
                variables,
                superclass_args,
                type_param_bounds,
                type_param_defaults,
                mixins,
                ..
            } => {
                for arg in superclass_args {
                    collect_rbs_type_names(arg, names);
                }
                for mixin in mixins {
                    names.insert(mixin.name.clone());
                    for arg in &mixin.args {
                        collect_rbs_type_names(arg, names);
                    }
                }
                collect_rbs_type_param_entry_names(type_param_bounds, names);
                collect_rbs_type_param_entry_names(type_param_defaults, names);
                collect_method_type_names(methods, names);
                for variable in variables {
                    collect_rbs_type_names(&variable.type_, names);
                }
            }
            rbs_sys::Declaration::Module {
                methods,
                variables,
                type_param_bounds,
                type_param_defaults,
                self_types,
                mixins,
                ..
            } => {
                for self_type in self_types {
                    names.insert(self_type.name.clone());
                    for arg in &self_type.args {
                        collect_rbs_type_names(arg, names);
                    }
                }
                for mixin in mixins {
                    names.insert(mixin.name.clone());
                    for arg in &mixin.args {
                        collect_rbs_type_names(arg, names);
                    }
                }
                collect_rbs_type_param_entry_names(type_param_bounds, names);
                collect_rbs_type_param_entry_names(type_param_defaults, names);
                collect_method_type_names(methods, names);
                for variable in variables {
                    collect_rbs_type_names(&variable.type_, names);
                }
            }
            rbs_sys::Declaration::Interface {
                methods,
                type_param_bounds,
                type_param_defaults,
                mixins,
                ..
            } => {
                for mixin in mixins {
                    names.insert(mixin.name.clone());
                    for arg in &mixin.args {
                        collect_rbs_type_names(arg, names);
                    }
                }
                collect_rbs_type_param_entry_names(type_param_bounds, names);
                collect_rbs_type_param_entry_names(type_param_defaults, names);
                collect_method_type_names(methods, names);
            }
            rbs_sys::Declaration::Constant { type_, .. }
            | rbs_sys::Declaration::Global { type_, .. } => {
                collect_rbs_type_names(type_, names);
            }
            rbs_sys::Declaration::TypeAlias {
                type_,
                type_param_bounds,
                type_param_defaults,
                ..
            } => {
                collect_rbs_type_param_entry_names(type_param_bounds, names);
                collect_rbs_type_param_entry_names(type_param_defaults, names);
                collect_rbs_type_names(type_, names);
            }
            rbs_sys::Declaration::ClassAlias { .. } | rbs_sys::Declaration::ModuleAlias { .. } => {}
        }
    }
}

fn collect_method_type_names(methods: &[rbs_sys::MethodDecl], names: &mut HashSet<String>) {
    for method in methods {
        for method_type in &method.method_types {
            collect_rbs_method_type_names(method_type, names);
        }
    }
}

fn collect_rbs_type_param_entry_names(
    entries: &[(String, rbs_sys::RbsType)],
    names: &mut HashSet<String>,
) {
    for (_, type_) in entries {
        collect_rbs_type_names(type_, names);
    }
}

fn collect_rbs_method_type_names(method_type: &rbs_sys::MethodType, names: &mut HashSet<String>) {
    collect_function_type_names(&method_type.function_type, names);
    if let Some(self_type) = &method_type.self_type {
        collect_rbs_type_names(self_type, names);
    }
    for (_, bound) in &method_type.type_param_bounds {
        collect_rbs_type_names(bound, names);
    }
    for (_, lower_bound) in &method_type.type_param_lower_bounds {
        collect_rbs_type_names(lower_bound, names);
    }
    if let Some(block) = &method_type.block {
        collect_function_type_names(&block.function_type, names);
        if let Some(self_type) = &block.self_type {
            collect_rbs_type_names(self_type, names);
        }
    }
}

fn collect_function_type_names(function_type: &rbs_sys::FunctionType, names: &mut HashSet<String>) {
    for param in function_type
        .required_positionals
        .iter()
        .chain(function_type.optional_positionals.iter())
        .chain(function_type.trailing_positionals.iter())
    {
        collect_rbs_type_names(&param.type_, names);
    }
    if let Some(param) = &function_type.rest_positionals {
        collect_rbs_type_names(&param.type_, names);
    }
    for (_, param) in function_type
        .required_keywords
        .iter()
        .chain(function_type.optional_keywords.iter())
    {
        collect_rbs_type_names(&param.type_, names);
    }
    if let Some(param) = &function_type.rest_keywords {
        collect_rbs_type_names(&param.type_, names);
    }
    collect_rbs_type_names(&function_type.return_type, names);
}

fn collect_rbs_type_names(rbs_type: &rbs_sys::RbsType, names: &mut HashSet<String>) {
    match rbs_type {
        rbs_sys::RbsType::Alias(name, args) | rbs_sys::RbsType::Class(name, args) => {
            names.insert(name.clone());
            for arg in args {
                collect_rbs_type_names(arg, names);
            }
        }
        rbs_sys::RbsType::Singleton(name) => {
            names.insert(name.clone());
        }
        rbs_sys::RbsType::Union(types)
        | rbs_sys::RbsType::Intersection(types)
        | rbs_sys::RbsType::Tuple(types) => {
            for ty in types {
                collect_rbs_type_names(ty, names);
            }
        }
        rbs_sys::RbsType::Optional(inner) => {
            collect_rbs_type_names(inner, names);
        }
        rbs_sys::RbsType::Record(fields) => {
            for field in fields {
                collect_rbs_type_names(&field.type_, names);
            }
        }
        rbs_sys::RbsType::Proc(method_type) => {
            collect_rbs_method_type_names(method_type, names);
        }
        rbs_sys::RbsType::Variable(_)
        | rbs_sys::RbsType::Literal(_)
        | rbs_sys::RbsType::Integer
        | rbs_sys::RbsType::Float
        | rbs_sys::RbsType::String
        | rbs_sys::RbsType::Symbol
        | rbs_sys::RbsType::Bool
        | rbs_sys::RbsType::Nil
        | rbs_sys::RbsType::Void
        | rbs_sys::RbsType::Untyped
        | rbs_sys::RbsType::Top
        | rbs_sys::RbsType::Bottom
        | rbs_sys::RbsType::SelfType
        | rbs_sys::RbsType::ClassType
        | rbs_sys::RbsType::InstanceType => {}
    }
}

fn resolve_versioned_rbs_roots(vendor_rbs_root: &Path, ruby_version: RubyVersion) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut push = |path: PathBuf| {
        if path.exists() && !candidates.contains(&path) {
            candidates.push(path);
        }
    };
    let major_minor = ruby_version.major_minor_string();
    let full = ruby_version.full_string();

    for dir_name in ["core", "stdlib"] {
        push(vendor_rbs_root.join(&major_minor).join(dir_name));
        push(vendor_rbs_root.join(&full).join(dir_name));
        push(vendor_rbs_root.join(dir_name).join(&major_minor));
        push(vendor_rbs_root.join(dir_name).join(&full));
    }
    push(vendor_rbs_root.join("core"));
    push(vendor_rbs_root.join("stdlib"));

    candidates
}

// Excludes stdlib reopens of core built-ins from the index (would reduce accuracy in an environment without the require).
const CORE_OWNED_CLASSES: &[&str] = &[
    "Object",
    "BasicObject",
    "Kernel",
    "Module",
    "Class",
    "Integer",
    "Float",
    "Numeric",
    "Rational",
    "Complex",
    "String",
    "Symbol",
    "Array",
    "Hash",
    "Range",
    "Regexp",
    "MatchData",
    "Comparable",
    "Enumerable",
    "Enumerator",
    "NilClass",
    "TrueClass",
    "FalseClass",
    "Proc",
    "Method",
    "UnboundMethod",
    "Exception",
    "StandardError",
    "Struct",
    "Data",
    "IO",
    "File",
    "Dir",
    "Encoding",
    "Thread",
    "Mutex",
    "Time",
];

/// Lightweight scan for top-level class/module names (defers parsing, excludes core reopens).
fn scan_top_level_declarations(path: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for line in content.lines() {
        let Some(rest) = line
            .strip_prefix("class ")
            .or_else(|| line.strip_prefix("module "))
        else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == ':' || *c == '_')
            .collect();
        if name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_uppercase())
            && !CORE_OWNED_CLASSES.contains(&name.as_str())
            && !names.contains(&name)
        {
            names.push(name);
        }
    }
    names
}

/// Scans only for top-level aliases (avoids stem shadowing, reduces core startup cost).
fn scan_top_level_class_aliases(path: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for line in content.lines() {
        let Some(rest) = line
            .strip_prefix("class ")
            .or_else(|| line.strip_prefix("module "))
        else {
            continue;
        };
        // Only the alias form `Name = Other` (not `class Foo < Bar` / `class Foo[T]`).
        let Some((lhs, _)) = rest.split_once('=') else {
            continue;
        };
        let name = lhs.trim();
        if name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_uppercase())
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == ':' || c == '_')
            && !names.iter().any(|existing: &String| existing == name)
        {
            names.push(name.to_string());
        }
    }
    names
}

/// Scans for qualified top-level nested classes (can't use the stem convention; resolves chains like `IO#stat` -> `File::Stat`).
fn scan_top_level_qualified_declarations(path: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for line in content.lines() {
        let Some(rest) = line
            .strip_prefix("class ")
            .or_else(|| line.strip_prefix("module "))
        else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == ':' || *c == '_')
            .collect();
        // Qualified names only (e.g. `File::Stat`). A single segment (`File`) is already
        // indexed by the stem convention, so it's skipped here; alias forms (`Name = Other`) are excluded too.
        if name.contains("::")
            && name
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_uppercase())
            && !names.contains(&name)
        {
            names.push(name);
        }
    }
    names
}

/// Recognize well-known stdlib RBS files whose declared class name doesn't
/// follow the "stem -> CamelCase" convention.
fn classify_rbs_path_to_class_name(path_str: &str) -> Option<String> {
    if path_str.ends_with("/rbs/unnamed/env_class.rbs") {
        return Some("RBS::Unnamed::ENVClass".to_string());
    }
    if path_str.ends_with("/rbs/unnamed/argf_class.rbs") {
        return Some("RBS::Unnamed::ARGFClass".to_string());
    }
    if path_str.ends_with("/rbs/unnamed/random_class.rbs") {
        return Some("RBS::Unnamed::Random_class".to_string());
    }
    if path_str.ends_with("/stdlib/cgi/0/core.rbs")
        || path_str.ends_with("/stdlib/cgi-escape/0/escape.rbs")
    {
        return Some("CGI".to_string());
    }
    if path_str.ends_with("/stdlib/tmpdir/0/tmpdir.rbs") {
        return Some("Dir".to_string());
    }
    if path_str.ends_with("/stdlib/date/0/time.rbs")
        || path_str.ends_with("/stdlib/time/0/time.rbs")
    {
        return Some("Time".to_string());
    }
    if path_str.ends_with("/stdlib/date/0/date.rbs") {
        return Some("Date".to_string());
    }
    if path_str.ends_with("/stdlib/date/0/date_time.rbs") {
        return Some("DateTime".to_string());
    }
    if let Some(class_name) = classify_zlib_rbs_path(path_str) {
        return Some(class_name);
    }
    if let Some(class_name) = classify_uri_rbs_path(path_str) {
        return Some(class_name);
    }
    None
}

fn normalize_path_for_matching(path: &Path) -> Option<Cow<'_, str>> {
    let path = path.to_str()?;
    if path.contains('\\') {
        Some(Cow::Owned(path.replace('\\', "/")))
    } else {
        Some(Cow::Borrowed(path))
    }
}

fn classify_nested_mixin_rbs_path(path_str: &str) -> Option<&'static str> {
    if path_str.ends_with("/unnamed/random.rbs") || path_str.ends_with("/random-formatter.rbs") {
        // Random formatter is split across 2 core/stdlib files, so index both.
        Some("RBS::Unnamed::Random_Formatter")
    } else if path_str.ends_with("/random.rbs") {
        // `module Random::Formatter` includes `RBS::Unnamed::Random_Formatter`.
        Some("Random::Formatter")
    } else {
        None
    }
}

fn classify_zlib_rbs_path(path_str: &str) -> Option<String> {
    let rel = path_str
        .split("/stdlib/zlib/0/")
        .nth(1)?
        .strip_suffix(".rbs")?;
    if rel == "zlib" {
        return Some("Zlib".to_string());
    }
    Some(format!(
        "Zlib::{}",
        rel.split('/')
            .map(stem_to_class_name)
            .collect::<Vec<_>>()
            .join("::")
    ))
}

fn classify_uri_rbs_path(path_str: &str) -> Option<String> {
    let rel = path_str
        .split("/stdlib/uri/0/")
        .nth(1)?
        .strip_suffix(".rbs")?;
    let class_name = match rel {
        "common" => "URI",
        "file" => "URI::File",
        "ftp" => "URI::FTP",
        "generic" => "URI::Generic",
        "http" => "URI::HTTP",
        "https" => "URI::HTTPS",
        "ldap" => "URI::LDAP",
        "ldaps" => "URI::LDAPS",
        "mailto" => "URI::MailTo",
        "rfc2396_parser" => "URI::RFC2396_Parser",
        "rfc3986_parser" => "URI::RFC3986_Parser",
        "ws" => "URI::WS",
        "wss" => "URI::WSS",
        _ => return None,
    };
    Some(class_name.to_string())
}

fn stem_to_class_name(stem: &str) -> String {
    static KNOWN_MAPPINGS: &[(&str, &str)] = &[
        ("string", "String"),
        ("integer", "Integer"),
        ("float", "Float"),
        ("array", "Array"),
        ("hash", "Hash"),
        ("symbol", "Symbol"),
        ("nil_class", "NilClass"),
        ("true_class", "TrueClass"),
        ("false_class", "FalseClass"),
        ("object", "Object"),
        ("basic_object", "BasicObject"),
        ("kernel", "Kernel"),
        ("comparable", "Comparable"),
        ("enumerable", "Enumerable"),
        ("numeric", "Numeric"),
        ("range", "Range"),
        ("regexp", "Regexp"),
        ("io", "IO"),
        ("file", "File"),
        ("dir", "Dir"),
        ("proc", "Proc"),
        ("method", "Method"),
        ("exception", "Exception"),
        ("encoding", "Encoding"),
        ("time", "Time"),
        ("struct", "Struct"),
        ("class", "Class"),
        ("module", "Module"),
        // Stdlib modules whose declared class name doesn't follow the
        // default "split-on-underscore" rule.
        ("fileutils", "FileUtils"),
        ("json", "JSON"),
        ("openssl", "OpenSSL"),
        ("zlib", "Zlib"),
        ("stringio", "StringIO"),
        ("yaml", "YAML"),
        ("csv", "CSV"),
        ("erb", "ERB"),
        ("cgi", "CGI"),
        ("uri", "URI"),
        ("ipaddr", "IPAddr"),
        ("rbconfig", "RbConfig"),
        ("pstore", "PStore"),
        ("drb", "DRb"),
        ("ostruct", "OpenStruct"),
    ];

    for &(file_stem, class_name) in KNOWN_MAPPINGS {
        if stem == file_stem {
            return class_name.to_string();
        }
    }

    stem.split('_')
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    format!("{upper}{}", chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect()
}

fn collect_rbs_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "rbs") {
            result.push(path.clone());
        } else if path.is_dir() {
            collect_rbs_files_recursive(path, &mut result);
        }
    }
    // Sorts to make index construction deterministic (APFS/ext4 readdir order differs; also gives `core/errors.rbs` priority).
    result.sort();
    result
}

fn collect_rbs_files_recursive(dir: &Path, result: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "rbs") {
                result.push(path);
            } else if path.is_dir() && !should_skip_dir(&path) {
                collect_rbs_files_recursive(&path, result);
            }
        }
    }
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(should_skip_dir_name)
}

fn should_skip_dir_name(name: &str) -> bool {
    matches!(
        name,
        "vendor" | "target" | "node_modules" | ".git" | ".bundle" | "tmp" | "log"
    )
}

#[cfg(test)]
mod merge_counter {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    static COUNTS: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
    static BUILDS: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();

    fn counts() -> &'static Mutex<HashMap<String, usize>> {
        COUNTS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn builds() -> &'static Mutex<HashMap<String, usize>> {
        BUILDS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub(super) fn record(class_name: &str) {
        *counts()
            .lock()
            .expect("merge counter poisoned")
            .entry(class_name.to_string())
            .or_insert(0) += 1;
    }

    pub(super) fn record_build(class_name: &str) {
        *builds()
            .lock()
            .expect("build counter poisoned")
            .entry(class_name.to_string())
            .or_insert(0) += 1;
    }

    pub(super) fn reset() {
        counts().lock().expect("merge counter poisoned").clear();
        builds().lock().expect("build counter poisoned").clear();
    }

    pub(super) fn get(class_name: &str) -> usize {
        counts()
            .lock()
            .expect("merge counter poisoned")
            .get(class_name)
            .copied()
            .unwrap_or(0)
    }

    pub(super) fn get_builds(class_name: &str) -> usize {
        builds()
            .lock()
            .expect("build counter poisoned")
            .get(class_name)
            .copied()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Barrier, Mutex};
    use tempfile::tempdir;

    struct DuplicateProneLazyRbsLoader {
        class_to_file: HashMap<String, PathBuf>,
        loaded: Mutex<HashMap<String, bool>>,
        registry: Mutex<TypeRegistry>,
    }

    impl DuplicateProneLazyRbsLoader {
        fn new(core_dir: PathBuf) -> Self {
            let mut class_to_file = HashMap::new();
            for path in collect_rbs_files(&[core_dir]) {
                if let Some(stem) = path.file_stem().and_then(|segment| segment.to_str()) {
                    class_to_file.insert(stem_to_class_name(stem), path);
                }
            }
            Self {
                class_to_file,
                loaded: Mutex::new(HashMap::new()),
                registry: Mutex::new(TypeRegistry::new()),
            }
        }

        fn merge_class_into(&self, class_name: &str, target: &mut TypeRegistry) -> bool {
            {
                let loaded = self.loaded.lock().expect("loaded lock poisoned");
                if loaded.contains_key(class_name) {
                    let registry = self.registry.lock().expect("registry lock poisoned");
                    return target.merge_rbs_class_from(&registry, class_name);
                }
            }

            if let Some(path) = self.class_to_file.get(class_name)
                && let Ok(content) = std::fs::read_to_string(path)
            {
                let mut registry = self.registry.lock().expect("registry lock poisoned");
                crate::rbs::import::load_rbs_string(&content, &mut registry);
            }

            self.loaded
                .lock()
                .expect("loaded lock poisoned")
                .insert(class_name.to_string(), true);
            let registry = self.registry.lock().expect("registry lock poisoned");
            target.merge_rbs_class_from(&registry, class_name)
        }
    }

    fn write_heavy_stdlib_file(path: &Path) {
        let mut source = String::from("class String\n");
        for idx in 0..4000 {
            source.push_str(&format!("  def method_{idx}: () -> String\n"));
        }
        source.push_str("end\n");
        std::fs::write(path, source).expect("write stdlib file");
    }

    fn bench_parallel_stdlib_merge(
        merge_class_into: impl Fn() + Send + Sync + 'static,
        threads: usize,
    ) -> std::time::Duration {
        let start = std::time::Instant::now();
        let barrier = Arc::new(Barrier::new(threads));
        let merge_class_into = Arc::new(merge_class_into);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                let barrier = Arc::clone(&barrier);
                let merge = Arc::clone(&merge_class_into);
                scope.spawn(move || {
                    barrier.wait();
                    merge();
                });
            }
        });
        start.elapsed()
    }

    #[test]
    fn ruby_version_loader_prefers_versioned_core_dir() {
        let dir = tempdir().expect("tempdir");
        let vendor = dir.path().join("vendor").join("rbs");
        let versioned_core = vendor.join("3.1").join("core");
        let fallback_core = vendor.join("core");
        std::fs::create_dir_all(&versioned_core).expect("mkdir versioned core");
        std::fs::create_dir_all(&fallback_core).expect("mkdir fallback core");
        std::fs::write(
            versioned_core.join("string.rbs"),
            "class String\n  def size: () -> Integer\nend\n",
        )
        .expect("write versioned rbs");
        std::fs::write(
            fallback_core.join("string.rbs"),
            "class String\n  def size: () -> String\nend\n",
        )
        .expect("write fallback rbs");

        let loader = LazyRbsLoader::for_ruby_version(vendor, RubyVersion::new(3, 1, 4));
        let mut registry = TypeRegistry::new();
        assert!(loader.merge_class_into("String", &mut registry));
        assert_eq!(
            registry.lookup_method_return_type("String", "size"),
            Some(crate::types::Type::Integer)
        );
    }

    #[test]
    fn ruby_version_loader_falls_back_to_unversioned_core() {
        let dir = tempdir().expect("tempdir");
        let vendor = dir.path().join("vendor").join("rbs");
        let fallback_core = vendor.join("core");
        std::fs::create_dir_all(&fallback_core).expect("mkdir fallback core");
        std::fs::write(
            fallback_core.join("integer.rbs"),
            "class Integer\n  def succ: () -> Integer\nend\n",
        )
        .expect("write fallback rbs");

        let loader = LazyRbsLoader::for_ruby_version(vendor, RubyVersion::new(3, 4, 0));
        let mut registry = TypeRegistry::new();
        assert!(loader.merge_class_into("Integer", &mut registry));
        assert_eq!(
            registry.lookup_method_return_type("Integer", "succ"),
            Some(crate::types::Type::Integer)
        );
    }

    #[test]
    fn merge_class_into_keeps_same_file_extensions() {
        let dir = tempdir().expect("tempdir");
        let core_dir = dir.path().join("vendor").join("rbs").join("core");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        std::fs::write(
            core_dir.join("set.rbs"),
            "class Set[Elem]\nend\nmodule Enumerable[Elem]\n  def to_set: () -> Set[Elem]\nend\n",
        )
        .expect("write set rbs");

        let loader = LazyRbsLoader::new(core_dir);
        let mut registry = TypeRegistry::new();
        assert!(loader.merge_class_into("Set", &mut registry));
        assert!(
            !registry
                .lookup_rbs_method_types("Enumerable", "to_set")
                .is_empty()
        );
    }

    #[test]
    fn merge_class_into_keeps_cross_file_extensions() {
        let dir = tempdir().expect("tempdir");
        let vendor = dir.path().join("vendor").join("rbs");
        let core_dir = vendor.join("core");
        let time_dir = vendor.join("stdlib").join("time").join("0");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        std::fs::create_dir_all(&time_dir).expect("mkdir time");
        std::fs::write(
            core_dir.join("time.rbs"),
            "class Time\n  def self.now: () -> Time\nend\n",
        )
        .expect("write core time");
        std::fs::write(
            time_dir.join("time.rbs"),
            "class Time\n  def httpdate: () -> String\nend\n",
        )
        .expect("write stdlib time");

        let loader = LazyRbsLoader::for_ruby_version(vendor, RubyVersion::new(3, 4, 0));
        let mut registry = TypeRegistry::new();
        assert!(loader.merge_class_into("Time", &mut registry));
        assert_eq!(
            registry.lookup_method_return_type("Time", "httpdate"),
            Some(crate::types::Type::String)
        );
    }

    #[test]
    fn merge_class_into_resolves_cross_file_method_alias() {
        // A cross-file method alias (Kernel#send) resolves via finalize after the ancestors are merged.
        let dir = tempdir().expect("tempdir");
        let vendor = dir.path().join("vendor").join("rbs");
        let core_dir = vendor.join("core");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        std::fs::write(
            core_dir.join("basic_object.rbs"),
            "class BasicObject\n  def __send__: (Symbol | String, *untyped) -> untyped\nend\n",
        )
        .expect("write basic_object");
        std::fs::write(
            core_dir.join("kernel.rbs"),
            "module Kernel\n  alias send __send__\nend\n",
        )
        .expect("write kernel");
        std::fs::write(
            core_dir.join("object.rbs"),
            "class Object < BasicObject\n  include Kernel\nend\n",
        )
        .expect("write object");

        let loader = LazyRbsLoader::for_ruby_version(vendor, RubyVersion::new(3, 4, 0));
        let mut registry = TypeRegistry::new();
        // Merge Kernel first (at this point, send's target __send__ is not yet merged).
        loader.merge_class_into("Kernel", &mut registry);
        loader.merge_class_into("BasicObject", &mut registry);
        loader.merge_class_into("Object", &mut registry);
        assert!(
            !registry
                .resolve_method_call_owners("Object", "send", false)
                .is_empty(),
            "Kernel#send alias to BasicObject#__send__ should resolve after ancestry merge"
        );
    }

    #[test]
    fn lazy_loader_indexes_securerandom_formatter_chain() {
        // Verifies the nested mixin module needed to resolve `SecureRandom.uuid` is
        // loadable via the explicit index (using the actual vendored RBS).
        let vendor = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs");
        let loader = LazyRbsLoader::for_ruby_version(vendor, RubyVersion::new(3, 4, 0));
        let mut reg = TypeRegistry::new();
        assert!(
            loader.merge_class_into("RBS::Unnamed::Random_Formatter", &mut reg),
            "RBS::Unnamed::Random_Formatter should be indexed and loadable"
        );
        // uuid is defined in stdlib's random-formatter.rbs, as a nested declaration.
        assert!(
            reg.lookup_method_return_type("RBS::Unnamed::Random_Formatter", "uuid")
                .is_some(),
            "uuid should be present after merging the formatter module"
        );
    }

    #[test]
    fn lazy_loader_indexes_qualified_nested_core_classes() {
        // Resolves `File::Stat#ino` via the qualified nested core class index (prevents a regression to untyped through the chain).
        let vendor = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs");
        let loader = LazyRbsLoader::for_ruby_version(vendor, RubyVersion::new(3, 4, 0));
        let mut reg = TypeRegistry::new();
        assert!(
            loader.merge_class_into("File::Stat", &mut reg),
            "File::Stat should be indexed by its qualified top-level declaration"
        );
        assert_eq!(
            reg.lookup_method_return_type("File::Stat", "ino"),
            Some(crate::types::Type::Integer)
        );
    }

    #[test]
    fn scan_top_level_qualified_declarations_matches_only_qualified() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("sample.rbs");
        std::fs::write(
            &path,
            "class File < IO\n\
             end\n\
             class File::Stat < Object\n\
             end\n\
             module Process::Status\n\
             end\n\
             class Plain\n\
             end\n",
        )
        .expect("write rbs");
        let mut names = scan_top_level_qualified_declarations(&path);
        names.sort();
        assert_eq!(
            names,
            vec!["File::Stat".to_string(), "Process::Status".to_string()]
        );
    }

    #[test]
    fn lazy_loader_indexes_core_class_aliases() {
        // Verifies core top-level aliases like `class Mutex = Thread::Mutex` are loadable
        // via the index and resolve as constants (using the actual vendored RBS).
        let vendor = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs");
        let loader = LazyRbsLoader::for_ruby_version(vendor, RubyVersion::new(3, 4, 0));
        for alias in ["Mutex", "Queue", "ConditionVariable", "SizedQueue"] {
            let mut reg = TypeRegistry::new();
            assert!(
                loader.merge_class_into(alias, &mut reg),
                "{alias} should be indexed and loadable as a class alias"
            );
            let constant = reg
                .class_data_for("Object")
                .and_then(|object| object.constants.get(alias))
                .unwrap_or_else(|| panic!("{alias} constant should be merged onto Object"));
            // The runtime alias constant is lookup-able but must not leak into top-level output.
            assert!(
                constant.external_source,
                "{alias} alias constant must be flagged external_source"
            );
        }
    }

    #[test]
    fn scan_top_level_class_aliases_matches_only_alias_form() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("sample.rbs");
        std::fs::write(
            &path,
            "class Mutex = Thread::Mutex\n\
             module YAML = Psych\n\
             class Regular < Object\n\
             end\n\
             module Plain\n\
             end\n",
        )
        .expect("write rbs");
        let mut names = scan_top_level_class_aliases(&path);
        names.sort();
        assert_eq!(names, vec!["Mutex".to_string(), "YAML".to_string()]);
    }

    #[test]
    fn stdlib_path_classification_accepts_windows_separators() {
        let zlib_path = Path::new(r"C:\workspace\vendor\rbs\stdlib\zlib\0\deflate.rbs");
        let zlib_path = normalize_path_for_matching(zlib_path).expect("valid UTF-8 path");
        assert_eq!(
            classify_rbs_path_to_class_name(&zlib_path),
            Some("Zlib::Deflate".to_string())
        );

        let random_path = Path::new(r"C:\workspace\vendor\rbs\core\rbs\unnamed\random.rbs");
        let random_path = normalize_path_for_matching(random_path).expect("valid UTF-8 path");
        assert_eq!(
            classify_nested_mixin_rbs_path(&random_path),
            Some("RBS::Unnamed::Random_Formatter")
        );
    }

    #[test]
    fn merge_class_into_maps_stdlib_nested_paths() {
        let dir = tempdir().expect("tempdir");
        let vendor = dir.path().join("vendor").join("rbs");
        let zlib_dir = vendor.join("stdlib").join("zlib").join("0");
        std::fs::create_dir_all(&zlib_dir).expect("mkdir zlib");
        std::fs::write(
            zlib_dir.join("deflate.rbs"),
            "module Zlib\n  class Deflate\n    def self.deflate: (String string) -> String\n  end\nend\n",
        )
        .expect("write deflate");

        let loader = LazyRbsLoader::for_ruby_version(vendor, RubyVersion::new(3, 4, 0));
        let mut registry = TypeRegistry::new();
        assert!(loader.merge_class_into("Zlib::Deflate", &mut registry));
        assert_eq!(
            registry.lookup_method_return_type_with_hint("Zlib::Deflate", "deflate", true),
            Some(crate::types::Type::String)
        );
    }

    #[test]
    fn merge_class_into_preserves_user_rbs_methods() {
        let dir = tempdir().expect("tempdir");
        let core_dir = dir.path().join("vendor").join("rbs").join("core");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        std::fs::write(
            core_dir.join("array.rbs"),
            "class Array[Elem]\n  def zip: [U] (Array[U] other) -> Array[[Elem, U?]]\nend\n",
        )
        .expect("write array rbs");

        let loader = LazyRbsLoader::new(core_dir);
        let mut user = TypeRegistry::new();
        crate::rbs::import::load_rbs_string(
            "class Array\n  def zip: (Array[untyped] other) -> Symbol\nend\n",
            &mut user,
        );
        let mut registry = TypeRegistry::new();
        registry.merge_user_rbs_registry(&user);
        assert!(loader.merge_class_into("Array", &mut registry));
        assert_eq!(
            registry.lookup_method_return_type("Array", "zip"),
            Some(crate::types::Type::Symbol)
        );
    }

    #[test]
    fn merge_class_into_resolves_method_type_aliases_from_dependency_files() {
        let dir = tempdir().expect("tempdir");
        let core_dir = dir.path().join("vendor").join("rbs").join("core");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        std::fs::write(
            core_dir.join("types.rbs"),
            "module Types\n  type label = Integer\nend\n",
        )
        .expect("write types rbs");
        std::fs::write(
            core_dir.join("worker.rbs"),
            concat!(
                "class Worker\n",
                "  def cast: (String value) -> String\n",
                "          | (Types::label value) -> Integer\n",
                "end\n",
            ),
        )
        .expect("write worker rbs");

        let loader = LazyRbsLoader::new(core_dir);
        let mut registry = TypeRegistry::new();
        assert!(loader.merge_class_into("Worker", &mut registry));
        let overloads = registry.lookup_rbs_method_types("Worker", "cast");

        assert_eq!(overloads.len(), 2);
        assert_eq!(
            overloads[1].function_type.required_positionals[0].type_,
            crate::rbs::ir::RbsType::Class(crate::types::Sym::new("Integer"), Box::default())
        );
    }

    #[test]
    fn merge_class_into_preserves_generic_alias_args_from_dependency_files() {
        let dir = tempdir().expect("tempdir");
        let core_dir = dir.path().join("vendor").join("rbs").join("core");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        std::fs::write(
            core_dir.join("types.rbs"),
            "module Types\n  type list[T] = Array[T]\nend\n",
        )
        .expect("write types rbs");
        std::fs::write(
            core_dir.join("worker.rbs"),
            concat!(
                "class Worker\n",
                "  def names: () -> Types::list[String]\n",
                "  def pick: (Types::list[String] values) -> String\n",
                "end\n",
            ),
        )
        .expect("write worker rbs");

        let loader = LazyRbsLoader::new(core_dir);
        let mut registry = TypeRegistry::new();
        assert!(loader.merge_class_into("Worker", &mut registry));

        assert_eq!(
            registry.lookup_method_return_type("Worker", "names"),
            Some(crate::types::Type::Array(Some(Box::new(
                crate::types::Type::String
            ))))
        );

        let overloads = registry.lookup_rbs_method_types("Worker", "pick");
        assert_eq!(
            overloads[0].function_type.required_positionals[0].type_,
            crate::rbs::ir::RbsType::Class(
                crate::types::Sym::new("Array"),
                Box::new([crate::rbs::ir::RbsType::Class(
                    crate::types::Sym::new("String"),
                    Box::default(),
                )]),
            )
        );
    }

    #[test]
    fn merge_class_into_resolves_class_type_param_bounds_from_dependency_files() {
        let dir = tempdir().expect("tempdir");
        let core_dir = dir.path().join("vendor").join("rbs").join("core");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        std::fs::write(
            core_dir.join("types.rbs"),
            "module Types\n  type label = String\nend\n",
        )
        .expect("write types rbs");
        std::fs::write(
            core_dir.join("worker.rbs"),
            "class Worker[T < Types::label]\n  def value: -> T\nend\n",
        )
        .expect("write worker rbs");

        let loader = LazyRbsLoader::new(core_dir);
        let mut registry = TypeRegistry::new();
        assert!(loader.merge_class_into("Worker", &mut registry));

        assert_eq!(
            registry.get_class_type_param_bounds("Worker"),
            &[(
                "T".to_string(),
                crate::rbs::ir::RbsType::Class(crate::types::Sym::new("String"), Box::default())
            )]
        );
    }

    #[test]
    fn merge_class_into_resolves_type_alias_param_metadata_from_dependency_files() {
        let dir = tempdir().expect("tempdir");
        let core_dir = dir.path().join("vendor").join("rbs").join("core");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        std::fs::write(
            core_dir.join("types.rbs"),
            "module Types\n  type label = String\nend\n",
        )
        .expect("write types rbs");
        std::fs::write(
            core_dir.join("worker.rbs"),
            concat!(
                "type default_list[T = Types::label] = Array[T]\n",
                "type bounded_list[T < Types::label] = Array[T]\n",
                "class Worker\n",
                "  def defaults: -> default_list\n",
                "  def bounds: -> bounded_list\n",
                "end\n",
            ),
        )
        .expect("write worker rbs");

        let loader = LazyRbsLoader::new(core_dir);
        let mut registry = TypeRegistry::new();
        assert!(loader.merge_class_into("Worker", &mut registry));

        let expected = Some(crate::types::Type::Array(Some(Box::new(
            crate::types::Type::String,
        ))));
        assert_eq!(
            registry.lookup_method_return_type("Worker", "defaults"),
            expected
        );
        assert_eq!(
            registry.lookup_method_return_type("Worker", "bounds"),
            expected
        );
    }

    static MERGE_COUNTER_GUARD: Mutex<()> = Mutex::new(());

    fn class_shape_fingerprint(registry: &TypeRegistry, class_name: &str) -> String {
        let data = registry
            .class_data_for(class_name)
            .expect("merged class present");
        let mut methods: Vec<String> = data
            .methods
            .iter()
            .map(|m| format!("{}#{}", m.is_singleton as u8, m.name))
            .collect();
        methods.sort();
        let mut mixins: Vec<String> = data
            .mixins
            .iter()
            .map(|m| format!("{:?}:{}", m.kind, m.module_name.as_ref()))
            .collect();
        mixins.sort();
        format!(
            "module={} super={:?} mixins=[{}] methods=[{}]",
            data.is_module,
            data.superclass.as_ref().map(|s| s.as_ref()),
            mixins.join(","),
            methods.join(","),
        )
    }

    #[test]
    fn merge_class_into_is_repeated_once_per_referencing_file() {
        let _guard = MERGE_COUNTER_GUARD.lock().expect("merge counter guard");
        merge_counter::reset();

        let dir = tempdir().expect("tempdir");
        let core_dir = dir.path().join("vendor").join("rbs").join("core");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        std::fs::write(
            core_dir.join("shared.rbs"),
            "class Shared\n  def name: () -> String\nend\n",
        )
        .expect("write rbs");

        let loader = LazyRbsLoader::new(core_dir);
        const FILE_COUNT: usize = 50;
        for _ in 0..FILE_COUNT {
            let mut per_file_registry = TypeRegistry::new();
            assert!(loader.merge_class_into("Shared", &mut per_file_registry));
        }

        assert_eq!(
            merge_counter::get("Shared"),
            FILE_COUNT,
            "same external class is re-merged once per referencing file",
        );
        assert_eq!(
            merge_counter::get_builds("Shared"),
            1,
            "canonical shape is built once workspace-wide, not per file",
        );

        loader.drop_file_cache();
        let mut after_drop = TypeRegistry::new();
        assert!(loader.merge_class_into("Shared", &mut after_drop));
        assert_eq!(
            merge_counter::get_builds("Shared"),
            1,
            "drop_file_cache must not rebuild canonical shapes",
        );
        assert_eq!(
            after_drop.lookup_method_return_type("Shared", "name"),
            Some(crate::types::Type::String)
        );
    }

    #[test]
    fn merge_class_into_shape_is_independent_of_merge_context() {
        let dir = tempdir().expect("tempdir");
        let core_dir = dir.path().join("vendor").join("rbs").join("core");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        std::fs::write(
            core_dir.join("integer.rbs"),
            "class Integer < Numeric\n  def succ: () -> Integer\nend\n",
        )
        .expect("write integer");
        std::fs::write(
            core_dir.join("string.rbs"),
            "class String\n  def length: () -> Integer\nend\n",
        )
        .expect("write string");
        std::fs::write(
            core_dir.join("numeric.rbs"),
            "class Numeric\n  def abs: () -> Numeric\nend\n",
        )
        .expect("write numeric");

        let loader = LazyRbsLoader::new(core_dir);

        let mut registry_a = TypeRegistry::new();
        assert!(loader.merge_class_into("Integer", &mut registry_a));

        let mut registry_b = TypeRegistry::new();
        assert!(loader.merge_class_into("String", &mut registry_b));
        assert!(loader.merge_class_into("Numeric", &mut registry_b));
        assert!(loader.merge_class_into("Integer", &mut registry_b));

        assert_eq!(
            class_shape_fingerprint(&registry_a, "Integer"),
            class_shape_fingerprint(&registry_b, "Integer"),
            "merged shape for a fixed FQN must not depend on merge context",
        );
    }

    #[test]
    #[ignore = "benchmark"]
    fn bench_parallel_stdlib_load_deduplicates_first_parse() {
        let dir = tempdir().expect("tempdir");
        let core_dir = dir.path().join("vendor").join("rbs").join("core");
        std::fs::create_dir_all(&core_dir).expect("mkdir core");
        write_heavy_stdlib_file(&core_dir.join("string.rbs"));

        let threads = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4)
            .max(4);

        let baseline_loader = Arc::new(DuplicateProneLazyRbsLoader::new(core_dir.clone()));
        let baseline_ms = bench_parallel_stdlib_merge(
            {
                let baseline_loader = Arc::clone(&baseline_loader);
                move || {
                    let mut registry = TypeRegistry::new();
                    assert!(baseline_loader.merge_class_into("String", &mut registry));
                }
            },
            threads,
        )
        .as_secs_f64()
            * 1000.0;

        let loader = Arc::new(LazyRbsLoader::new(core_dir));
        let optimized_ms = bench_parallel_stdlib_merge(
            {
                let loader = Arc::clone(&loader);
                move || {
                    let mut registry = TypeRegistry::new();
                    assert!(loader.merge_class_into("String", &mut registry));
                }
            },
            threads,
        )
        .as_secs_f64()
            * 1000.0;

        eprintln!(
            "[bench] stdlib parallel first-load baseline_ms={baseline_ms:.0} optimized_ms={optimized_ms:.0} speedup={:.2}x threads={threads}",
            baseline_ms / optimized_ms.max(0.1),
        );
        assert!(
            optimized_ms < baseline_ms,
            "optimized stdlib loader should beat duplicate-prone baseline"
        );
    }
}
