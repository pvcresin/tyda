use crate::rbs::ir as rbs_ir;
use crate::sym::NamePath;
use crate::types::Sym;
use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

thread_local! {
    static RESOLVE_DEPTH: Cell<usize> = const { Cell::new(0) };
    // attr-reader resolution recurses mutually; a depth guard alone can't bound the branching and causes CPU hangs, so cut it off via `visiting`.
    static ATTR_READER_VISITING: std::cell::RefCell<HashSet<(String, String, bool)>> =
        std::cell::RefCell::new(HashSet::new());
}

thread_local! {
    // worklist: the only dependency for slot re-evaluation is the (owner class, method name) return types read during this round.
    static RETURN_TYPE_READS: std::cell::RefCell<Option<FxHashSet<(Sym, Sym)>>> =
        const { std::cell::RefCell::new(None) };
}

/// `owner_class` must be the class whose `class_data` entry was physically read, not a logical/inherited owner.
fn note_return_type_read(owner_class: impl Into<Sym>, method_name: impl Into<Sym>) {
    RETURN_TYPE_READS.with(|cell| {
        if let Some(set) = cell.borrow_mut().as_mut() {
            set.insert((owner_class.into(), method_name.into()));
        }
    });
}

const MAX_RESOLVE_DEPTH: usize = 64;
const MAX_EXACT_ANCESTOR_CHAIN_LENGTH: usize = 64;

struct ResolveDepthGuard;

impl ResolveDepthGuard {
    fn enter() -> Option<Self> {
        RESOLVE_DEPTH.with(|d| {
            let current = d.get();
            if current >= MAX_RESOLVE_DEPTH {
                None
            } else {
                d.set(current + 1);
                Some(Self)
            }
        })
    }
}

impl Drop for ResolveDepthGuard {
    fn drop(&mut self) {
        RESOLVE_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

struct AttrReaderVisitGuard {
    key: (String, String, bool),
}

impl AttrReaderVisitGuard {
    fn enter(class_name: &str, ivar_name: &str, is_singleton: bool) -> Option<Self> {
        let key = (class_name.to_string(), ivar_name.to_string(), is_singleton);
        let inserted = ATTR_READER_VISITING.with(|cell| cell.borrow_mut().insert(key.clone()));
        if inserted { Some(Self { key }) } else { None }
    }
}

impl Drop for AttrReaderVisitGuard {
    fn drop(&mut self) {
        ATTR_READER_VISITING.with(|cell| {
            cell.borrow_mut().remove(&self.key);
        });
    }
}

thread_local! {
    // ivar type inference from `initialize` also recurses mutually via a separate path, so cut it off via `visiting`.
    static ATTR_INIT_VISITING: std::cell::RefCell<HashSet<(String, String)>> =
        std::cell::RefCell::new(HashSet::new());
}

struct AttrInitVisitGuard {
    key: (String, String),
}

impl AttrInitVisitGuard {
    fn enter(class_name: &str, ivar_name: &str) -> Option<Self> {
        let key = (class_name.to_string(), ivar_name.to_string());
        let inserted = ATTR_INIT_VISITING.with(|cell| cell.borrow_mut().insert(key.clone()));
        if inserted { Some(Self { key }) } else { None }
    }
}

impl Drop for AttrInitVisitGuard {
    fn drop(&mut self) {
        ATTR_INIT_VISITING.with(|cell| {
            cell.borrow_mut().remove(&self.key);
        });
    }
}

thread_local! {
    // prewarm solver: the param table only reads the cache and records dependencies (nested resolve would cause missed wake-ups and nondeterminism).
    static PARAM_TABLE_MODE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PARAM_TABLE_READS: std::cell::RefCell<Option<FxHashSet<(SharedName, SharedName, bool)>>> =
        const { std::cell::RefCell::new(None) };
    // deferred hop memo: shares recomputation for the same (class, singleton, type) (avoids branching explosion when evaluating hubs).
    static DEFERRED_HOP_MEMO: std::cell::RefCell<Option<DeferredHopMemo>> =
        const { std::cell::RefCell::new(None) };
}

type DeferredHopMemo = FxHashMap<Arc<DeferredKey>, Type>;
type DeferredMemo = FxHashMap<Arc<DeferredKey>, Type>;
type DeferredVisiting = FxHashSet<Arc<DeferredKey>>;

/// Key for the deferred-ref memo and its `visiting` set. Both maps are probed with the
/// same key, and the `Type` component is a whole subtree, so the hash is computed once
/// at construction and each map op only walks the type when hashes collide. `Arc` keeps
/// the shared key out of a second deep clone for the `visiting` insert.
#[derive(Debug)]
struct DeferredKey {
    hash: u64,
    class_name: SharedName,
    singleton_context: bool,
    ty: Type,
}

impl DeferredKey {
    fn new(class_name: SharedName, singleton_context: bool, ty: Type) -> Self {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        class_name.hash(&mut hasher);
        singleton_context.hash(&mut hasher);
        ty.hash(&mut hasher);
        Self {
            hash: hasher.finish(),
            class_name,
            singleton_context,
            ty,
        }
    }
}

impl PartialEq for DeferredKey {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
            && self.singleton_context == other.singleton_context
            && self.class_name == other.class_name
            && self.ty == other.ty
    }
}

impl Eq for DeferredKey {}

impl std::hash::Hash for DeferredKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

fn note_param_table_read(key: (SharedName, SharedName, bool)) {
    PARAM_TABLE_READS.with(|cell| {
        if let Some(set) = cell.borrow_mut().as_mut() {
            set.insert(key);
        }
    });
}

struct ParamTableScope;

impl ParamTableScope {
    fn enter() -> Self {
        PARAM_TABLE_MODE.with(|cell| cell.set(true));
        PARAM_TABLE_READS.with(|cell| {
            *cell.borrow_mut() = Some(FxHashSet::default());
        });
        DEFERRED_HOP_MEMO.with(|cell| {
            *cell.borrow_mut() = Some(FxHashMap::default());
        });
        Self
    }

    fn take_reads() -> Vec<(SharedName, SharedName, bool)> {
        PARAM_TABLE_READS.with(|cell| {
            cell.borrow_mut()
                .take()
                .map(|set| set.into_iter().collect())
                .unwrap_or_default()
        })
    }
}

impl Drop for ParamTableScope {
    fn drop(&mut self) {
        PARAM_TABLE_MODE.with(|cell| cell.set(false));
        PARAM_TABLE_READS.with(|cell| {
            *cell.borrow_mut() = None;
        });
        DEFERRED_HOP_MEMO.with(|cell| {
            *cell.borrow_mut() = None;
        });
    }
}

struct DeferredHopMemoScope {
    installed: bool,
}

impl DeferredHopMemoScope {
    fn enter() -> Self {
        let installed = DEFERRED_HOP_MEMO.with(|cell| {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                *slot = Some(FxHashMap::default());
                true
            } else {
                false
            }
        });
        Self { installed }
    }
}

impl Drop for DeferredHopMemoScope {
    fn drop(&mut self) {
        if self.installed {
            DEFERRED_HOP_MEMO.with(|cell| {
                *cell.borrow_mut() = None;
            });
        }
    }
}

type CallerCtxMemoKey = (SharedName, SharedName, bool, Type);

thread_local! {
    // caller-context memo: avoids re-resolving the same (caller, arg type) combo in dense graphs.
    static CALLER_CTX_MEMO: std::cell::RefCell<Option<FxHashMap<CallerCtxMemoKey, Type>>> =
        const { std::cell::RefCell::new(None) };
}

struct CallerCtxMemoScope {
    installed: bool,
}

impl CallerCtxMemoScope {
    fn enter() -> Self {
        let installed = CALLER_CTX_MEMO.with(|cell| {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                *slot = Some(FxHashMap::default());
                true
            } else {
                false
            }
        });
        Self { installed }
    }
}

impl Drop for CallerCtxMemoScope {
    fn drop(&mut self) {
        if self.installed {
            CALLER_CTX_MEMO.with(|cell| {
                *cell.borrow_mut() = None;
            });
        }
    }
}

mod external_types;
mod file_local;
mod includer_dsl;
mod keyword_args;
pub mod knowledge_precedence;
mod output;
mod stdlib_returns;

pub use keyword_args::KeywordArgTypes;

pub use file_local::MethodBodySummary;

#[derive(Debug, Default, Clone, Copy)]
pub struct RegistryBreakdownTotals {
    pub class_count: usize,
    pub method_count: usize,
    pub method_index_count: usize,
    pub constant_count: usize,
    pub ivar_count: usize,
    pub singleton_ivar_count: usize,
    pub class_variable_count: usize,
    pub call_site_count: usize,
    pub mixin_count: usize,
    pub undefined_method_count: usize,
    pub annotated_param_count: usize,
    pub param_count: usize,
    pub rbs_overload_count: usize,
    pub method_block_meta_count: usize,
    pub name_pool_count: usize,
    pub type_alias_count: usize,
    pub global_variable_count: usize,
}

use crate::types::{
    ClassInfo, ConstantSig, HoverBlockSig, MethodAliasSig, MethodSig, OverloadSig, Param,
    ParamKind, RecordField, RecordKey, SharedName, SharedPath, SourceLocation, Type,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamInfo {
    pub name: String,
    pub kind: ParamKind,
    pub default_type: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallSiteCallerContext {
    pub class_name: SharedName,
    pub method_name: SharedName,
    pub method_is_singleton: bool,
}

fn call_site_fingerprint(call_site: &CallSite) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    call_site.method_name.hash(&mut hasher);
    call_site.method_is_singleton.hash(&mut hasher);
    call_site.arg_types.hash(&mut hasher);
    let mut keyword_acc: u64 = 0;
    for (name, ty) in call_site.keyword_arg_types.iter() {
        let mut kw_hasher = rustc_hash::FxHasher::default();
        name.hash(&mut kw_hasher);
        ty.hash(&mut kw_hasher);
        keyword_acc ^= kw_hasher.finish();
    }
    keyword_acc.hash(&mut hasher);
    call_site.block.hash(&mut hasher);
    if let Some(context) = call_site.caller_context.as_deref() {
        context.class_name.hash(&mut hasher);
        context.method_name.hash(&mut hasher);
        context.method_is_singleton.hash(&mut hasher);
    }
    hasher.finish()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub method_name: SharedName,
    pub method_is_singleton: bool,
    pub arg_types: Vec<Type>,
    pub keyword_arg_types: KeywordArgTypes,
    // `block` only applies to a few sites, so box it to keep `CallSite` small.
    pub block: Option<Box<HoverBlockSig>>,
    // same for `caller_context` (generic param forwarding is rare).
    pub caller_context: Option<Box<CallSiteCallerContext>>,
}

type CallSiteCallerContextKey = (SharedName, SharedName, bool);
type CallSiteSummaryKey = (SharedName, bool, Option<CallSiteCallerContextKey>);
type GroupedCallSites = HashMap<CallSiteSummaryKey, CallSiteSummaryAccumulator>;

// type slot dedups immediately via a set (avoids the O(n^2) sort+dedup cost and transient bloat for popular methods).
#[derive(Default)]
struct TypeSlotAccumulator {
    parts: rustc_hash::FxHashSet<Type>,
    // exceeding the union cap sticks to untyped regardless of merge order.
    saturated: bool,
}

impl TypeSlotAccumulator {
    fn from_type(ty: Type) -> Self {
        let mut acc = Self::default();
        acc.add(ty);
        acc
    }

    fn add(&mut self, ty: Type) {
        if self.saturated {
            return;
        }
        // append_union_parts flattens nested unions and drops Untyped
        // (but keeps the empty signal meaning "call site told us nothing").
        let mut flattened = Vec::new();
        Type::append_union_parts(&mut flattened, ty);
        for part in flattened {
            self.parts.insert(part);
        }
        if self.parts.len() > Type::UNION_CARDINALITY_LIMIT {
            self.parts = rustc_hash::FxHashSet::default();
            self.saturated = true;
        }
    }

    fn finish(self) -> Type {
        if self.saturated {
            return Type::Untyped;
        }
        Type::from_type_vec(self.parts.into_iter().collect())
    }
}

struct CallSiteSummaryAccumulator {
    template: CallSite,
    arg_slots: Vec<TypeSlotAccumulator>,
    keyword_slots: HashMap<SharedName, TypeSlotAccumulator>,
    block_return: Option<TypeSlotAccumulator>,
    block_required: bool,
}

impl CallSiteSummaryAccumulator {
    fn new(mut site: CallSite) -> Self {
        let arg_slots = site
            .arg_types
            .drain(..)
            .map(TypeSlotAccumulator::from_type)
            .collect();
        let keyword_slots = site
            .keyword_arg_types
            .drain()
            .map(|(name, ty)| (name, TypeSlotAccumulator::from_type(ty)))
            .collect();
        let (block_return, block_required) = match &mut site.block {
            Some(block) => (
                Some(TypeSlotAccumulator::from_type(std::mem::replace(
                    &mut block.return_type,
                    Type::Untyped,
                ))),
                block.required,
            ),
            None => (None, false),
        };
        Self {
            template: site,
            arg_slots,
            keyword_slots,
            block_return,
            block_required,
        }
    }

    fn fold(&mut self, other: CallSite) {
        if self.arg_slots.len() < other.arg_types.len() {
            // fill missing positional-argument observations as Untyped (i.e. an empty slot).
            self.arg_slots
                .resize_with(other.arg_types.len(), TypeSlotAccumulator::default);
        }
        for (idx, ty) in other.arg_types.into_iter().enumerate() {
            self.arg_slots[idx].add(ty);
        }
        for (name, ty) in other.keyword_arg_types {
            match self.keyword_slots.entry(name) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().add(ty);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(TypeSlotAccumulator::from_type(ty));
                }
            }
        }
        if let Some(mut other_block) = other.block {
            self.block_required |= other_block.required;
            let other_return = std::mem::replace(&mut other_block.return_type, Type::Untyped);
            match &mut self.block_return {
                Some(acc) => acc.add(other_return),
                None => {
                    // use the params from the call site where `block` was first observed as the template.
                    self.template.block = Some(other_block);
                    self.block_return = Some(TypeSlotAccumulator::from_type(other_return));
                }
            }
        }
    }

    fn finish(self) -> CallSite {
        let mut site = self.template;
        site.arg_types = self
            .arg_slots
            .into_iter()
            .map(TypeSlotAccumulator::finish)
            .collect();
        site.keyword_arg_types = self
            .keyword_slots
            .into_iter()
            .map(|(name, acc)| (name, acc.finish()))
            .collect();
        if let (Some(block), Some(acc)) = (&mut site.block, self.block_return) {
            block.return_type = acc.finish();
            block.required = self.block_required;
        }
        site
    }
}

impl std::hash::Hash for CallSite {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.method_name.hash(state);
        self.method_is_singleton.hash(state);
        self.arg_types.hash(state);
        for (key, value) in self.keyword_arg_types.sorted_pairs() {
            key.hash(state);
            value.hash(state);
        }
        self.block.hash(state);
        self.caller_context.hash(state);
    }
}

/// keeps shared chunks plus an owned tail as a segment list (invariant: same order/content as the old single Vec).
#[derive(Debug, Default, Clone)]
pub struct CallSiteStore {
    head: Option<Box<CallSiteHead>>,

    tail: Vec<CallSite>,
}

#[derive(Debug, Default, Clone)]
struct CallSiteHead {
    segments: Vec<CallSiteSegment>,
    ends: Vec<u32>,
}

#[derive(Debug, Clone)]
enum CallSiteSegment {
    Shared(Arc<[CallSite]>),
    Owned(Vec<CallSite>),
}

impl CallSiteSegment {
    #[inline]
    fn as_slice(&self) -> &[CallSite] {
        match self {
            CallSiteSegment::Shared(chunk) => chunk,
            CallSiteSegment::Owned(sites) => sites,
        }
    }
}

impl CallSiteHead {
    #[inline]
    fn len(&self) -> usize {
        self.ends.last().copied().unwrap_or(0) as usize
    }

    fn push_segment(&mut self, segment: CallSiteSegment) {
        let end = (self.len() + segment.as_slice().len()) as u32;
        self.segments.push(segment);
        self.ends.push(end);
    }
}

impl CallSiteStore {
    #[inline]
    fn head_len(&self) -> usize {
        match &self.head {
            Some(head) => head.len(),
            None => 0,
        }
    }

    #[inline]
    fn head_segments(&self) -> &[CallSiteSegment] {
        match &self.head {
            Some(head) => &head.segments,
            None => &[],
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.head_len() + self.tail.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.is_none() && self.tail.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> CallSiteIter<'_> {
        CallSiteIter {
            segments: self.head_segments().iter(),
            current: [].iter(),
            tail: self.tail.iter(),
        }
    }

    #[inline]
    pub fn push(&mut self, site: CallSite) {
        self.tail.push(site);
    }

    pub fn extend(&mut self, sites: impl IntoIterator<Item = CallSite>) {
        self.tail.extend(sites);
    }
    pub fn push_chunk(&mut self, chunk: Arc<[CallSite]>) {
        if chunk.is_empty() {
            return;
        }
        let head = self.head.get_or_insert_with(Default::default);
        if !self.tail.is_empty() {
            let mut run = std::mem::take(&mut self.tail);
            run.shrink_to_fit();
            head.push_segment(CallSiteSegment::Owned(run));
        }
        head.push_segment(CallSiteSegment::Shared(chunk));
    }
    #[inline]
    pub fn get(&self, flat: usize) -> &CallSite {
        let head_len = self.head_len();
        if flat >= head_len {
            return &self.tail[flat - head_len];
        }
        let head = self.head.as_deref().expect("head_len > 0 implies head");
        let idx = head.ends.partition_point(|&end| (end as usize) <= flat);
        let start = if idx == 0 {
            0
        } else {
            head.ends[idx - 1] as usize
        };
        &head.segments[idx].as_slice()[flat - start]
    }
    pub fn take_all(&mut self) -> Vec<CallSite> {
        let Some(head) = self.head.take() else {
            return std::mem::take(&mut self.tail);
        };
        let mut out = Vec::with_capacity(self.len());
        for segment in head.segments {
            match segment {
                CallSiteSegment::Shared(chunk) => out.extend(chunk.iter().cloned()),
                CallSiteSegment::Owned(mut sites) => out.append(&mut sites),
            }
        }
        out.append(&mut self.tail);
        out
    }
    pub fn replace_with(&mut self, sites: Vec<CallSite>) {
        self.head = None;
        self.tail = sites;
    }

    pub fn contains(&self, site: &CallSite) -> bool {
        self.iter().any(|s| s == site)
    }

    pub fn to_vec(&self) -> Vec<CallSite> {
        self.iter().cloned().collect()
    }

    pub(crate) fn segment_count(&self) -> usize {
        self.head_segments().len()
    }

    pub(crate) fn owned_sites_mut(&mut self) -> impl Iterator<Item = &mut CallSite> {
        let head_owned = self
            .head
            .as_deref_mut()
            .into_iter()
            .flat_map(|head| head.segments.iter_mut())
            .filter_map(|segment| match segment {
                CallSiteSegment::Shared(_) => None,
                CallSiteSegment::Owned(sites) => Some(sites.iter_mut()),
            })
            .flatten();
        head_owned.chain(self.tail.iter_mut())
    }

    pub(crate) fn for_each_attribution(&self, mut f: impl FnMut(CallSiteAttribution<'_>)) {
        for segment in self.head_segments() {
            match segment {
                CallSiteSegment::Shared(chunk) => f(CallSiteAttribution::SharedChunk(
                    chunk.as_ptr() as usize,
                    chunk,
                )),
                CallSiteSegment::Owned(sites) => {
                    for site in sites {
                        f(CallSiteAttribution::OwnedSite(site));
                    }
                }
            }
        }
        for site in &self.tail {
            f(CallSiteAttribution::OwnedSite(site));
        }
    }

    pub fn shrink_to_fit(&mut self) {
        if let Some(head) = self.head.as_deref_mut() {
            head.segments.shrink_to_fit();
            head.ends.shrink_to_fit();
        }
        self.tail.shrink_to_fit();
    }
}

impl<'a> IntoIterator for &'a CallSiteStore {
    type Item = &'a CallSite;
    type IntoIter = CallSiteIter<'a>;

    fn into_iter(self) -> CallSiteIter<'a> {
        self.iter()
    }
}

pub(crate) enum CallSiteAttribution<'a> {
    SharedChunk(usize, &'a [CallSite]),

    OwnedSite(&'a CallSite),
}

pub struct CallSiteIter<'a> {
    segments: std::slice::Iter<'a, CallSiteSegment>,
    current: std::slice::Iter<'a, CallSite>,
    tail: std::slice::Iter<'a, CallSite>,
}

impl<'a> Iterator for CallSiteIter<'a> {
    type Item = &'a CallSite;

    #[inline]
    fn next(&mut self) -> Option<&'a CallSite> {
        loop {
            if let Some(site) = self.current.next() {
                return Some(site);
            }
            match self.segments.next() {
                Some(segment) => self.current = segment.as_slice().iter(),
                None => return self.tail.next(),
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MethodBlockMeta {
    pub yield_param_types: Vec<Type>,
    pub return_type: Option<Type>,
    pub forwarded_block: Option<(SharedName, bool)>,
    pub yields: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ClassMethodBlockMeta {
    pub instance: HashMap<SharedName, MethodBlockMeta>,
    pub singleton: HashMap<SharedName, MethodBlockMeta>,
}

impl ClassMethodBlockMeta {
    pub(crate) fn deep_bytes(&self) -> usize {
        let side = |map: &HashMap<SharedName, MethodBlockMeta>| {
            let mut bytes = if map.is_empty() {
                0
            } else {
                (map.len() * 8 / 7 + 1).next_power_of_two()
                    * (std::mem::size_of::<(SharedName, MethodBlockMeta)>() + 1)
                    + 48
            };
            for meta in map.values() {
                bytes += meta.yield_param_types.len() * std::mem::size_of::<Type>();
                bytes += meta
                    .yield_param_types
                    .iter()
                    .map(Type::deep_extra_bytes)
                    .sum::<usize>();
                if let Some(ty) = &meta.return_type {
                    bytes += ty.deep_extra_bytes();
                }
            }
            bytes
        };
        side(&self.instance) + side(&self.singleton)
    }

    fn map(&self, is_singleton: bool) -> &HashMap<SharedName, MethodBlockMeta> {
        if is_singleton {
            &self.singleton
        } else {
            &self.instance
        }
    }

    fn map_mut(&mut self, is_singleton: bool) -> &mut HashMap<SharedName, MethodBlockMeta> {
        if is_singleton {
            &mut self.singleton
        } else {
            &mut self.instance
        }
    }

    fn get(&self, method_name: &str, is_singleton: bool) -> Option<&MethodBlockMeta> {
        self.map(is_singleton).get(method_name)
    }

    fn insert(
        &mut self,
        method_name: SharedName,
        is_singleton: bool,
        meta: MethodBlockMeta,
    ) -> Option<MethodBlockMeta> {
        self.map_mut(is_singleton).insert(method_name, meta)
    }

    fn get_or_insert(
        &mut self,
        method_name: SharedName,
        is_singleton: bool,
        meta: MethodBlockMeta,
    ) {
        self.map_mut(is_singleton)
            .entry(method_name)
            .or_insert(meta);
    }
}

#[derive(Debug, Clone)]
pub struct OverloadDef {
    pub param_types: Vec<(Type, ParamKind)>,
    pub return_type: Type,
}

#[derive(Debug, Clone)]
pub struct MethodDef {
    pub name: Sym,
    pub param_infos: Vec<ParamInfo>,
    pub raw_return_type: Type,
    pub sorbet_modifier_comments: Vec<String>,
    pub rbs_annotated: bool,
    pub rbs_inline_annotated: bool,
    pub sig_annotated: bool,
    pub attr_ivar: Option<String>,
    pub is_singleton: bool,
    pub rbs_file_source: bool,
    pub synthetic_dsl_source: bool,
    pub rbs_method_types: Arc<Vec<rbs_ir::MethodType>>,
    pub extra_overloads: Vec<OverloadDef>,
    pub loc: Option<SourceLocation>,
}

/// matches and synthesizes the 15 AR dirty-family methods per column without materializing them (reduces `user_rbs` RSS).
#[derive(Debug, Clone, Default)]
pub struct DirtyPattern {
    columns: Vec<(Sym, Type)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirtyKind {
    Bool,
    Change,
    History,
    MaybeChange,
    Void,
}

/// dirty-family method name -> (column, kind). A cheap suffix/prefix filter for the hot path.
fn split_dirty_method_name(name: &str) -> Option<(&str, DirtyKind)> {
    // non-dirty-family `?`/`!` predicates are rejected immediately by the last byte, avoiding a chain of strips on the hot path.
    match name.as_bytes().last().copied() {
        // dirty `?` forms are only the `_changed?` family or `saved_change_to_x?` /
        // `will_save_change_to_x?`. Other predicates (e.g. `nil?`) are rejected here.
        Some(b'?') => {
            if !name.ends_with("_changed?")
                && !name.starts_with("saved_change_to_")
                && !name.starts_with("will_save_change_to_")
            {
                return None;
            }
        }
        // dirty `!` forms are only `_will_change!` / `restore_x!`.
        Some(b'!') => {
            if !name.ends_with("_will_change!") && !name.starts_with("restore_") {
                return None;
            }
        }
        // remaining dirty forms have a specific suffix, or are `saved_change_to_<col>` (MaybeChange).
        _ => {
            if !name.ends_with("_change")
                && !name.ends_with("_was")
                && !name.ends_with("_database")
                && !name.ends_with("_save")
                && !name.ends_with("_saved")
                && !name.starts_with("saved_change_to_")
            {
                return None;
            }
        }
    }
    // check prefix rules first (`saved_change_to_` / `will_save_change_to_` / `restore_` / `clear_`).
    if let Some(rest) = name.strip_prefix("will_save_change_to_") {
        return rest.strip_suffix('?').map(|col| (col, DirtyKind::Bool));
    }
    if let Some(rest) = name.strip_prefix("saved_change_to_") {
        return match rest.strip_suffix('?') {
            Some(col) => Some((col, DirtyKind::Bool)),
            None => Some((rest, DirtyKind::MaybeChange)),
        };
    }
    if let Some(rest) = name.strip_prefix("restore_") {
        return rest.strip_suffix('!').map(|col| (col, DirtyKind::Void));
    }
    if let Some(rest) = name.strip_prefix("clear_") {
        return rest
            .strip_suffix("_change")
            .map(|col| (col, DirtyKind::Void));
    }
    // suffix rules. Match longer suffixes first so they aren't swallowed by shorter ones.
    if let Some(col) = name.strip_suffix("_previously_changed?") {
        return Some((col, DirtyKind::Bool));
    }
    if let Some(col) = name.strip_suffix("_changed?") {
        return Some((col, DirtyKind::Bool));
    }
    if let Some(col) = name.strip_suffix("_previously_was") {
        return Some((col, DirtyKind::History));
    }
    if let Some(col) = name.strip_suffix("_before_last_save") {
        return Some((col, DirtyKind::History));
    }
    if let Some(col) = name.strip_suffix("_in_database") {
        return Some((col, DirtyKind::History));
    }
    if let Some(col) = name.strip_suffix("_was") {
        return Some((col, DirtyKind::History));
    }
    if let Some(col) = name.strip_suffix("_previous_change") {
        return Some((col, DirtyKind::MaybeChange));
    }
    if let Some(col) = name.strip_suffix("_change_to_be_saved") {
        return Some((col, DirtyKind::MaybeChange));
    }
    if let Some(col) = name.strip_suffix("_will_change!") {
        return Some((col, DirtyKind::Void));
    }
    if let Some(col) = name.strip_suffix("_change") {
        return Some((col, DirtyKind::Change));
    }
    None
}

impl DirtyPattern {
    pub fn from_columns(columns: Vec<(Sym, Type)>) -> Self {
        Self { columns }
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }
    fn merge_from(&mut self, other: &DirtyPattern) {
        for (name, ty) in &other.columns {
            if !self.columns.iter().any(|(existing, _)| existing == name) {
                self.columns.push((*name, ty.clone()));
            }
        }
    }

    fn column_type(&self, col: &str) -> Option<&Type> {
        self.columns
            .iter()
            .find(|(name, _)| name.as_str() == col)
            .map(|(_, ty)| ty)
    }
    pub(crate) fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|(col, _)| col.as_str() == name)
    }

    fn return_type_for(kind: DirtyKind, base_type: &Type) -> Type {
        // equivalent to `nullable_type(base, true)` (Untyped doesn't get `nil` added).
        let history_type = if matches!(base_type, Type::Untyped) {
            Type::Untyped
        } else {
            Type::Union(vec![base_type.clone(), Type::Nil])
        };
        match kind {
            DirtyKind::Bool => Type::Bool,
            DirtyKind::Change => Type::Tuple(vec![history_type.clone(), history_type]),
            DirtyKind::History => history_type,
            DirtyKind::MaybeChange => {
                let array = Type::Array(Some(Box::new(history_type)));
                Type::Union(vec![array, Type::Nil])
            }
            DirtyKind::Void => Type::Void,
        }
    }
    pub fn synthesize(&self, method_name: &str) -> Option<MethodDef> {
        let (col, kind) = split_dirty_method_name(method_name)?;
        let base_type = self.column_type(col)?;
        Some(Self::build_method_def(
            method_name,
            Self::return_type_for(kind, base_type),
        ))
    }

    fn build_method_def(method_name: &str, return_type: Type) -> MethodDef {
        MethodDef {
            name: Sym::new(method_name),
            param_infos: Vec::new(),
            raw_return_type: return_type,
            sorbet_modifier_comments: Vec::new(),
            rbs_annotated: true,
            rbs_inline_annotated: false,
            sig_annotated: false,
            attr_ivar: None,
            is_singleton: false,
            rbs_file_source: true,
            synthetic_dsl_source: true,
            rbs_method_types: Default::default(),
            extra_overloads: Vec::new(),
            loc: None,
        }
    }
    pub fn enumerate_methods_by_column(
        &self,
        already_present: &dyn Fn(&str) -> bool,
    ) -> Vec<(Sym, Vec<MethodDef>)> {
        let mut out = Vec::with_capacity(self.columns.len());
        for (col, base_type) in &self.columns {
            let col_str = col.as_str();
            let mut methods = Vec::with_capacity(DIRTY_METHOD_ORDER.len());
            for &(name_rule, kind) in DIRTY_METHOD_ORDER {
                let name = name_rule.method_name(col_str);
                if already_present(&name) {
                    continue;
                }
                methods.push(Self::build_method_def(
                    &name,
                    Self::return_type_for(kind, base_type),
                ));
            }
            out.push((*col, methods));
        }
        out
    }
}
#[derive(Debug, Clone, Copy)]
enum DirtyNameRule {
    Suffix(&'static str),
    Prefix(&'static str),

    PrefixQuestion(&'static str),

    RestorePrefix,

    ClearWrap,
}

impl DirtyNameRule {
    fn method_name(self, col: &str) -> String {
        match self {
            DirtyNameRule::Suffix(s) => format!("{col}{s}"),
            DirtyNameRule::Prefix(p) => format!("{p}{col}"),
            DirtyNameRule::PrefixQuestion(p) => format!("{p}{col}?"),
            DirtyNameRule::RestorePrefix => format!("restore_{col}!"),
            DirtyNameRule::ClearWrap => format!("clear_{col}_change"),
        }
    }
}
const DIRTY_METHOD_ORDER: &[(DirtyNameRule, DirtyKind)] = &[
    (DirtyNameRule::Suffix("_changed?"), DirtyKind::Bool),
    (
        DirtyNameRule::Suffix("_previously_changed?"),
        DirtyKind::Bool,
    ),
    (
        DirtyNameRule::PrefixQuestion("saved_change_to_"),
        DirtyKind::Bool,
    ),
    (
        DirtyNameRule::PrefixQuestion("will_save_change_to_"),
        DirtyKind::Bool,
    ),
    (DirtyNameRule::Suffix("_change"), DirtyKind::Change),
    (DirtyNameRule::Suffix("_was"), DirtyKind::History),
    (DirtyNameRule::Suffix("_previously_was"), DirtyKind::History),
    (
        DirtyNameRule::Suffix("_before_last_save"),
        DirtyKind::History,
    ),
    (DirtyNameRule::Suffix("_in_database"), DirtyKind::History),
    (
        DirtyNameRule::Suffix("_previous_change"),
        DirtyKind::MaybeChange,
    ),
    (
        DirtyNameRule::Suffix("_change_to_be_saved"),
        DirtyKind::MaybeChange,
    ),
    (
        DirtyNameRule::Prefix("saved_change_to_"),
        DirtyKind::MaybeChange,
    ),
    (DirtyNameRule::Suffix("_will_change!"), DirtyKind::Void),
    (DirtyNameRule::RestorePrefix, DirtyKind::Void),
    (DirtyNameRule::ClearWrap, DirtyKind::Void),
];

#[derive(Debug, Clone)]
pub struct MethodCompletionCandidate {
    pub name: String,
    pub owner_class: String,
    pub is_singleton: bool,
    pub sig: MethodSig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstantCompletionKind {
    Class,
    Module,
    Constant,
}

#[derive(Debug, Clone)]
pub struct ConstantCompletionCandidate {
    pub name: String,
    pub full_name: String,
    pub kind: ConstantCompletionKind,
    pub const_type: Option<Type>,
}

impl MethodDef {
    pub fn has_annotation(&self) -> bool {
        self.rbs_annotated || self.sig_annotated
    }

    pub fn is_external_rbs_source(&self) -> bool {
        self.rbs_file_source && !self.synthetic_dsl_source
    }

    pub fn resolve_param_ref(&self, idx: usize) -> Option<Type> {
        let positional: Vec<&ParamInfo> = self
            .param_infos
            .iter()
            .filter(|param_info| {
                matches!(
                    param_info.kind,
                    ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                )
            })
            .collect();
        positional
            .get(idx)
            .and_then(|param_info| param_info.default_type.clone())
            .or(Some(Type::Untyped))
    }

    pub fn resolve_keyword_param_ref(&self, name: &str) -> Option<Type> {
        self.param_infos
            .iter()
            .find(|param_info| {
                param_info.name == name
                    && matches!(
                        param_info.kind,
                        ParamKind::KeywordRequired | ParamKind::KeywordOptional
                    )
            })
            .and_then(|param_info| param_info.default_type.clone())
            .or(Some(Type::Untyped))
    }

    pub fn param_name_at(&self, idx: usize) -> Option<String> {
        self.param_infos
            .get(idx)
            .map(|param_info| param_info.name.clone())
    }

    pub fn effective_param_names(&self) -> Vec<String> {
        self.param_infos
            .iter()
            .map(|param_info| param_info.name.clone())
            .collect()
    }

    pub fn shrink_to_fit_after_collect(&mut self) {
        if self.param_infos.capacity() > self.param_infos.len() {
            self.param_infos.shrink_to_fit();
        }
        for info in &mut self.param_infos {
            info.name.shrink_to_fit();
        }
        if self.sorbet_modifier_comments.capacity() > self.sorbet_modifier_comments.len() {
            self.sorbet_modifier_comments.shrink_to_fit();
        }
        for comment in &mut self.sorbet_modifier_comments {
            comment.shrink_to_fit();
        }
        if self.extra_overloads.capacity() > self.extra_overloads.len() {
            self.extra_overloads.shrink_to_fit();
        }
        if let Some(ivar) = self.attr_ivar.as_mut() {
            ivar.shrink_to_fit();
        }
        if self.rbs_method_types.is_empty() {
            self.rbs_method_types = empty_rbs_method_types();
        }
    }
    pub fn needs_shrink(&self) -> bool {
        self.param_infos.capacity() > self.param_infos.len()
            || self
                .param_infos
                .iter()
                .any(|i| i.name.capacity() > i.name.len())
            || self.sorbet_modifier_comments.capacity() > self.sorbet_modifier_comments.len()
            || self
                .sorbet_modifier_comments
                .iter()
                .any(|c| c.capacity() > c.len())
            || self.extra_overloads.capacity() > self.extra_overloads.len()
            || self
                .attr_ivar
                .as_ref()
                .is_some_and(|ivar| ivar.capacity() > ivar.len())
            || (self.rbs_method_types.is_empty()
                && !Arc::ptr_eq(&self.rbs_method_types, &empty_rbs_method_types()))
    }

    pub fn deep_bytes(&self) -> usize {
        let mut bytes = std::mem::size_of::<MethodDef>() + 16;
        bytes += self.param_infos.len() * std::mem::size_of::<ParamInfo>();
        for info in &self.param_infos {
            bytes += info.name.capacity();
            if let Some(default) = &info.default_type {
                bytes += default.deep_extra_bytes();
            }
        }
        bytes += self.raw_return_type.deep_extra_bytes();
        bytes += string_vec_bytes(&self.sorbet_modifier_comments);
        bytes += self.attr_ivar.as_ref().map(String::capacity).unwrap_or(0);
        for overload in &self.extra_overloads {
            bytes += std::mem::size_of::<OverloadDef>();
            bytes += overload.param_types.len() * std::mem::size_of::<(Type, ParamKind)>();
            bytes += overload
                .param_types
                .iter()
                .map(|(ty, _)| ty.deep_extra_bytes())
                .sum::<usize>();
            bytes += overload.return_type.deep_extra_bytes();
        }
        if !self.rbs_method_types.is_empty() {
            bytes += 16 + self.rbs_method_types.len() * std::mem::size_of::<rbs_ir::MethodType>();
            bytes += self
                .rbs_method_types
                .iter()
                .map(rbs_ir::method_type_extra_bytes)
                .sum::<usize>();
        }
        bytes
    }
}

impl CallSite {
    pub fn deep_bytes(&self) -> usize {
        let mut bytes = std::mem::size_of::<CallSite>();
        bytes += self.arg_types.len() * std::mem::size_of::<Type>();
        bytes += self
            .arg_types
            .iter()
            .map(Type::deep_extra_bytes)
            .sum::<usize>();
        if !self.keyword_arg_types.is_empty() {
            bytes += self.keyword_arg_types.shell_bytes();
            bytes += self
                .keyword_arg_types
                .values()
                .map(Type::deep_extra_bytes)
                .sum::<usize>();
        }
        if let Some(block) = &self.block {
            bytes += std::mem::size_of::<HoverBlockSig>();
            bytes += param_vec_bytes(&block.params) + block.return_type.deep_extra_bytes();
        }
        if self.caller_context.is_some() {
            bytes += std::mem::size_of::<CallSiteCallerContext>();
        }
        bytes
    }
}

// share an empty RBS overload vec process-wide to avoid allocating an `Arc` for tens of thousands of methods.
fn empty_rbs_method_types() -> Arc<Vec<rbs_ir::MethodType>> {
    use std::sync::OnceLock;
    static EMPTY: OnceLock<Arc<Vec<rbs_ir::MethodType>>> = OnceLock::new();
    Arc::clone(EMPTY.get_or_init(|| Arc::new(Vec::new())))
}

#[derive(Debug, Clone, PartialEq)]
pub enum MixinKind {
    Include,
    Extend,
    Prepend,
}

impl MixinKind {
    fn hook_method_name(&self) -> &'static str {
        match self {
            Self::Include => "included",
            Self::Extend => "extended",
            Self::Prepend => "prepended",
        }
    }
}

/// RBS has no `protected`, so `Protected` degrades to `private`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Protected,
}

#[derive(Debug, Clone)]
pub struct Mixin {
    pub module_name: SharedName,
    pub type_args: Vec<rbs_ir::RbsType>,
    pub kind: MixinKind,
    pub external_source: bool,
}

#[derive(Debug, Clone)]
pub struct ConstantDef {
    pub name: Sym,
    pub const_type: Type,
    pub loc: Option<SourceLocation>,
    pub file_path: Option<SharedPath>,
    // lazy stdlib constants are lookup-only and are never emitted in RBS output.
    pub external_source: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MethodSlots {
    pub instance: Option<usize>,
    pub singleton: Option<usize>,
}

impl MethodSlots {
    fn get(self, is_singleton: bool) -> Option<usize> {
        if is_singleton {
            self.singleton
        } else {
            self.instance
        }
    }

    fn get_mut(&mut self, is_singleton: bool) -> &mut Option<usize> {
        if is_singleton {
            &mut self.singleton
        } else {
            &mut self.instance
        }
    }

    fn has(self, is_singleton: bool) -> bool {
        self.get(is_singleton).is_some()
    }
}

/// `u32::MAX` marks an unfilled slot, so a packed entry never needs an `Option`.
const FROZEN_SLOT_NONE: u32 = u32::MAX;

/// One `method_index` entry packed to 16B (`MethodSlots` alone is 32B).
#[derive(Debug, Clone, Copy)]
pub struct FrozenMethodSlots {
    name: Sym,
    /// First 8 name bytes, zero-padded, big-endian. Ordering by it agrees with
    /// `str` ordering (no identifier holds a NUL), so the probe resolves without
    /// dereferencing the interned name in the common case.
    prefix: u64,
    instance: u32,
    singleton: u32,
}

/// Ties (equal prefixes) fall through to the full name, so the total order is
/// exactly the `str` order the frozen array is built in.
#[inline]
fn name_prefix(name: &str) -> u64 {
    let mut acc = 0u64;
    let mut shift = 56u32;
    for byte in name.as_bytes().iter().take(8) {
        acc |= u64::from(*byte) << shift;
        shift = shift.wrapping_sub(8);
    }
    acc
}

impl FrozenMethodSlots {
    fn pack(name: Sym, slots: MethodSlots) -> Option<Self> {
        let pack = |slot: Option<usize>| match slot {
            None => Some(FROZEN_SLOT_NONE),
            Some(idx) => u32::try_from(idx).ok().filter(|i| *i != FROZEN_SLOT_NONE),
        };
        Some(Self {
            prefix: name_prefix(name.as_str()),
            name,
            instance: pack(slots.instance)?,
            singleton: pack(slots.singleton)?,
        })
    }

    #[inline]
    fn cmp_name(&self, prefix: u64, name: &str) -> std::cmp::Ordering {
        self.prefix
            .cmp(&prefix)
            .then_with(|| self.name.as_str().cmp(name))
    }

    fn slots(self) -> MethodSlots {
        let unpack = |slot: u32| (slot != FROZEN_SLOT_NONE).then_some(slot as usize);
        MethodSlots {
            instance: unpack(self.instance),
            singleton: unpack(self.singleton),
        }
    }

    fn slot_mut(&mut self, is_singleton: bool) -> &mut u32 {
        if is_singleton {
            &mut self.singleton
        } else {
            &mut self.instance
        }
    }
}

/// Method name -> slot indexes into `ClassData::methods`.
///
/// A frozen registry no longer gains method names, so `freeze()` packs the map into a
/// sorted array: 24B per entry against the ~2x-overprovisioned 41B of the map shell.
/// Slots of an existing name stay writable in place, and only a new name rematerializes
/// the map, so the encoding is behavior-identical.
#[derive(Debug, Default, Clone)]
pub enum MethodIndex {
    #[default]
    Empty,
    Frozen(Box<[FrozenMethodSlots]>),
    Live(Box<FxHashMap<Sym, MethodSlots>>),
}

pub enum MethodIndexIter<'a> {
    Empty,
    Frozen(std::slice::Iter<'a, FrozenMethodSlots>),
    Live(std::collections::hash_map::Iter<'a, Sym, MethodSlots>),
}

impl Iterator for MethodIndexIter<'_> {
    type Item = (Sym, MethodSlots);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Frozen(iter) => iter.next().map(|entry| (entry.name, entry.slots())),
            Self::Live(iter) => iter.next().map(|(name, slots)| (*name, *slots)),
        }
    }
}

impl MethodIndex {
    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Frozen(entries) => entries.len(),
            Self::Live(map) => map.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, name: &str) -> Option<MethodSlots> {
        match self {
            Self::Empty => None,
            Self::Frozen(entries) => {
                let prefix = name_prefix(name);
                entries
                    .binary_search_by(|probe| probe.cmp_name(prefix, name))
                    .ok()
                    .and_then(|idx| entries.get(idx))
                    .map(|entry| entry.slots())
            }
            Self::Live(map) => map.get(name).copied(),
        }
    }

    pub fn contains_key(&self, name: &str) -> bool {
        match self {
            Self::Empty => false,
            Self::Frozen(entries) => {
                let prefix = name_prefix(name);
                entries
                    .binary_search_by(|probe| probe.cmp_name(prefix, name))
                    .is_ok()
            }
            Self::Live(map) => map.contains_key(name),
        }
    }

    pub fn iter(&self) -> MethodIndexIter<'_> {
        match self {
            Self::Empty => MethodIndexIter::Empty,
            Self::Frozen(entries) => MethodIndexIter::Frozen(entries.iter()),
            Self::Live(map) => MethodIndexIter::Live(map.iter()),
        }
    }

    pub fn clear(&mut self) {
        *self = Self::Empty;
    }

    /// Write `idx` into the variant's slot, replacing whatever was there.
    pub fn set_slot(&mut self, name: Sym, is_singleton: bool, idx: usize) {
        if self.set_slot_in_place(name, is_singleton, idx, true) {
            return;
        }
        let mut map = self.take_map();
        *map.entry(name).or_default().get_mut(is_singleton) = Some(idx);
        *self = Self::Live(map);
    }

    /// Write `idx` only when the variant's slot is still empty.
    pub fn set_slot_if_absent(&mut self, name: Sym, is_singleton: bool, idx: usize) {
        if self.set_slot_in_place(name, is_singleton, idx, false) {
            return;
        }
        let mut map = self.take_map();
        let slot = map.entry(name).or_default().get_mut(is_singleton);
        if slot.is_none() {
            *slot = Some(idx);
        }
        *self = Self::Live(map);
    }

    /// Returns false when the write needs the map (unknown name, or an index too wide to pack).
    fn set_slot_in_place(
        &mut self,
        name: Sym,
        is_singleton: bool,
        idx: usize,
        overwrite: bool,
    ) -> bool {
        match self {
            Self::Empty => false,
            Self::Frozen(entries) => {
                let Ok(packed) = u32::try_from(idx) else {
                    return false;
                };
                if packed == FROZEN_SLOT_NONE {
                    return false;
                }
                let prefix = name_prefix(name.as_str());
                let Ok(pos) = entries.binary_search_by(|probe| probe.cmp_name(prefix, &name))
                else {
                    return false;
                };
                let Some(entry) = entries.get_mut(pos) else {
                    return false;
                };
                let slot = entry.slot_mut(is_singleton);
                if overwrite || *slot == FROZEN_SLOT_NONE {
                    *slot = packed;
                }
                true
            }
            Self::Live(map) => {
                let Some(slots) = map.get_mut(&name) else {
                    return false;
                };
                let slot = slots.get_mut(is_singleton);
                if overwrite || slot.is_none() {
                    *slot = Some(idx);
                }
                true
            }
        }
    }

    pub fn shrink_to_fit(&mut self) {
        if let Self::Live(map) = self {
            if map.is_empty() {
                *self = Self::Empty;
            } else {
                map.shrink_to_fit();
            }
        }
    }

    /// Pack the map into the sorted array read-only form.
    pub fn freeze(&mut self) {
        let Self::Live(map) = self else {
            return;
        };
        if map.is_empty() {
            *self = Self::Empty;
            return;
        }
        let mut entries: Vec<FrozenMethodSlots> = Vec::with_capacity(map.len());
        for (name, slots) in map.iter() {
            // an index too wide to pack keeps the whole class on the map.
            let Some(entry) = FrozenMethodSlots::pack(*name, *slots) else {
                return;
            };
            entries.push(entry);
        }
        entries.sort_unstable_by(|a, b| a.cmp_name(b.prefix, b.name.as_str()));
        *self = Self::Frozen(entries.into_boxed_slice());
    }

    fn take_map(&mut self) -> Box<FxHashMap<Sym, MethodSlots>> {
        match std::mem::take(self) {
            Self::Empty => Box::default(),
            Self::Frozen(entries) => Box::new(
                entries
                    .iter()
                    .map(|entry| (entry.name, entry.slots()))
                    .collect(),
            ),
            Self::Live(map) => map,
        }
    }

    pub(crate) fn container_bytes(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Frozen(entries) => entries.len() * std::mem::size_of::<FrozenMethodSlots>(),
            Self::Live(map) => {
                std::mem::size_of::<FxHashMap<Sym, MethodSlots>>()
                    + map_shell_bytes(map.len(), std::mem::size_of::<(Sym, MethodSlots)>())
            }
        }
    }
}

pub type MethodKey = (Sym, bool);

#[derive(Debug, Clone)]
pub struct UniformMethodFilePaths {
    path: SharedPath,
    // sorted, for binary search.
    keys: Box<[MethodKey]>,
}

/// Definition file path per method variant.
///
/// A class whose methods all come from one file (every per-file snapshot, and most
/// classes in the merged registry) stores that path once plus a packed key list; only
/// mixed-origin classes pay for the map. Both variants are boxed so the inline handle
/// stays 16B in the `ClassData` shell.
#[derive(Debug, Default, Clone)]
pub enum MethodFilePaths {
    #[default]
    Empty,
    Uniform(Box<UniformMethodFilePaths>),
    PerMethod(Box<FxHashMap<MethodKey, SharedPath>>),
}

pub enum MethodFilePathsIter<'a> {
    Empty,
    Uniform {
        path: &'a SharedPath,
        keys: std::slice::Iter<'a, MethodKey>,
    },
    PerMethod(std::collections::hash_map::Iter<'a, MethodKey, SharedPath>),
}

impl<'a> Iterator for MethodFilePathsIter<'a> {
    type Item = (MethodKey, &'a SharedPath);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Uniform { path, keys } => keys.next().map(|key| (*key, &**path)),
            Self::PerMethod(iter) => iter.next().map(|(key, path)| (*key, path)),
        }
    }
}

fn method_key_cmp(a: &MethodKey, b: &MethodKey) -> std::cmp::Ordering {
    a.0.cmp(&b.0).then(a.1.cmp(&b.1))
}

impl MethodFilePaths {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Uniform(uniform) => uniform.keys.is_empty(),
            Self::PerMethod(map) => map.is_empty(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Uniform(uniform) => uniform.keys.len(),
            Self::PerMethod(map) => map.len(),
        }
    }

    pub fn get(&self, key: &MethodKey) -> Option<&SharedPath> {
        match self {
            Self::Empty => None,
            Self::Uniform(uniform) => uniform
                .keys
                .binary_search_by(|probe| method_key_cmp(probe, key))
                .ok()
                .map(|_| &uniform.path),
            Self::PerMethod(map) => map.get(key),
        }
    }

    pub fn iter(&self) -> MethodFilePathsIter<'_> {
        match self {
            Self::Empty => MethodFilePathsIter::Empty,
            Self::Uniform(uniform) => MethodFilePathsIter::Uniform {
                path: &uniform.path,
                keys: uniform.keys.iter(),
            },
            Self::PerMethod(map) => MethodFilePathsIter::PerMethod(map.iter()),
        }
    }

    pub fn values(&self) -> impl Iterator<Item = &SharedPath> + '_ {
        self.iter().map(|(_, path)| path)
    }

    pub fn insert(&mut self, key: MethodKey, path: SharedPath) {
        let mut map = self.take_map();
        map.insert(key, path);
        *self = Self::PerMethod(map);
    }

    pub fn insert_if_absent(&mut self, key: MethodKey, path: SharedPath) {
        if self.get(&key).is_some() {
            return;
        }
        self.insert(key, path);
    }

    pub fn remove(&mut self, key: &MethodKey) {
        if self.get(key).is_none() {
            return;
        }
        let mut map = self.take_map();
        map.remove(key);
        *self = if map.is_empty() {
            Self::Empty
        } else {
            Self::PerMethod(map)
        };
    }

    /// Keep only the entries whose path satisfies `keep` (the key is never inspected).
    pub fn retain_paths(&mut self, keep: impl Fn(&SharedPath) -> bool) {
        match self {
            Self::Empty => {}
            Self::Uniform(uniform) => {
                if !keep(&uniform.path) {
                    *self = Self::Empty;
                }
            }
            Self::PerMethod(map) => {
                map.retain(|_, path| keep(path));
                if map.is_empty() {
                    *self = Self::Empty;
                }
            }
        }
    }

    pub fn shrink_to_fit(&mut self) {
        if let Self::PerMethod(map) = self {
            if map.is_empty() {
                *self = Self::Empty;
            } else {
                map.shrink_to_fit();
            }
        }
    }

    /// Collapse a map whose entries all share one path into the packed representation.
    pub fn freeze(&mut self) {
        let Self::PerMethod(map) = self else {
            return;
        };
        let mut values = map.values();
        let Some(first) = values.next().cloned() else {
            *self = Self::Empty;
            return;
        };
        if !values.all(|path| Arc::ptr_eq(path, &first) || *path == first) {
            return;
        }
        let mut keys: Vec<MethodKey> = map.keys().copied().collect();
        keys.sort_unstable_by(method_key_cmp);
        *self = Self::Uniform(Box::new(UniformMethodFilePaths {
            path: first,
            keys: keys.into_boxed_slice(),
        }));
    }

    fn take_map(&mut self) -> Box<FxHashMap<MethodKey, SharedPath>> {
        match std::mem::take(self) {
            Self::Empty => Box::default(),
            Self::Uniform(uniform) => Box::new(
                uniform
                    .keys
                    .iter()
                    .map(|key| (*key, uniform.path.clone()))
                    .collect(),
            ),
            Self::PerMethod(map) => map,
        }
    }

    pub(crate) fn container_bytes(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Uniform(uniform) => {
                std::mem::size_of::<UniformMethodFilePaths>()
                    + uniform.keys.len() * std::mem::size_of::<MethodKey>()
            }
            Self::PerMethod(map) => {
                std::mem::size_of::<FxHashMap<MethodKey, SharedPath>>()
                    + map_shell_bytes(map.len(), std::mem::size_of::<(MethodKey, SharedPath)>())
            }
        }
    }
}

/// box out cold fields that are usually empty (reduces LSP footprint across tens of thousands of `ClassData`).
#[derive(Debug, Default, Clone)]
pub struct ClassDataCold {
    pub undefined_methods: Vec<(SharedName, bool)>,
    // method aliases: call sites are consolidated to the canonical name, output uses an RBS alias.
    pub method_aliases: Vec<MethodAlias>,
    // visibility override (a cold map, so we don't touch the 130+ `MethodDef` construction sites).
    pub method_visibility: FxHashMap<(Sym, bool), Visibility>,
    pub singleton_ivars: FxHashMap<Sym, Vec<Type>>,
    pub class_variables: FxHashMap<Sym, Vec<Type>>,
    pub initialize_param_passthroughs: FxHashMap<SharedName, Vec<SharedName>>,
    // annotated params are keyed by (name, singleton) (prevents instance/singleton name collisions).
    pub annotated_params: FxHashMap<(Sym, bool), FxHashMap<usize, Type>>,

    pub superclass_type_args: Vec<rbs_ir::RbsType>,

    pub required_ancestors: Vec<SharedName>,

    pub required_ancestor_type_args: Vec<(SharedName, Vec<rbs_ir::RbsType>)>,

    pub sorbet_modifier_comments: Vec<String>,

    pub class_type_params: Vec<String>,
    pub class_type_param_bounds: Vec<(String, rbs_ir::RbsType)>,
    pub class_type_param_defaults: Vec<(String, Type)>,
    // dirty-family skeleton pattern (`ClassDataCold`).
    pub dirty_method_pattern: Option<DirtyPattern>,
    // set of bare ivar readers (used only for self-fact narrowing).
    pub bare_ivar_readers: FxHashSet<(Sym, bool)>,
    /// DSL recorded in a concern `included do` that must run against each includer
    /// (resource name / serializer owner), not the concern module itself.
    pub includer_bound_dsl: Vec<IncluderBoundDsl>,
}

/// Owner-dependent DSL collected on a concern and applied at each includer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncluderBoundDsl {
    Devise {
        loc: SourceLocation,
    },
    AmsBelongsTo {
        name: String,
        loc: SourceLocation,
    },
    AmsHasMany {
        name: String,
        loc: SourceLocation,
    },
    AmsModelAttributes {
        names: Vec<String>,
        loc: SourceLocation,
    },
}

fn empty_class_data_cold() -> &'static ClassDataCold {
    use std::sync::OnceLock;
    static EMPTY: OnceLock<ClassDataCold> = OnceLock::new();
    EMPTY.get_or_init(ClassDataCold::default)
}
/// Mixins contributed by a module's `included` / `extended` / `prepended` hook block.
#[derive(Debug, Default, Clone)]
pub struct HookMixins {
    pub included: Vec<Mixin>,
    pub extended: Vec<Mixin>,
    pub prepended: Vec<Mixin>,
}

impl HookMixins {
    pub fn by_kind(&self, kind: &MixinKind) -> &Vec<Mixin> {
        match kind {
            MixinKind::Include => &self.included,
            MixinKind::Extend => &self.extended,
            MixinKind::Prepend => &self.prepended,
        }
    }

    pub fn by_kind_mut(&mut self, kind: &MixinKind) -> &mut Vec<Mixin> {
        match kind {
            MixinKind::Include => &mut self.included,
            MixinKind::Extend => &mut self.extended,
            MixinKind::Prepend => &mut self.prepended,
        }
    }

    fn iter_mut_all(&mut self) -> impl Iterator<Item = &mut Mixin> {
        self.included
            .iter_mut()
            .chain(&mut self.extended)
            .chain(&mut self.prepended)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ClassData {
    // `MethodDef` is shared via `Arc`, only `make_mut` when needed.
    pub methods: Vec<Arc<MethodDef>>,
    pub method_index: MethodIndex,
    pub method_file_paths: MethodFilePaths,
    pub constants: FxHashMap<Sym, ConstantDef>,
    pub ivars: FxHashMap<Sym, Vec<Type>>,
    pub call_sites: CallSiteStore,
    pub(crate) call_sites_revision: u32,
    // boxed: only live during collection, and the 48B shell would otherwise sit in every
    // frozen `ClassData`.
    #[allow(clippy::box_collection)]
    pub(crate) call_site_fingerprints: Option<Box<std::collections::HashSet<u64>>>,
    pub has_pending_call_site_summary: bool,
    pub superclass: Option<SharedName>,
    pub mixins: Vec<Mixin>,
    // boxed: `included do` / `extended do` / `prepended do` are rare, and three inline
    // vectors would cost 72B in every `ClassData`.
    hook_mixins: Option<Box<HookMixins>>,
    pub is_module: bool,
    pub user_defined: bool,
    pub loc: Option<SourceLocation>,
    pub file_path: Option<SharedPath>,

    cold: Option<Box<ClassDataCold>>,
}

impl ClassData {
    #[inline]
    pub fn cold(&self) -> &ClassDataCold {
        match &self.cold {
            Some(cold) => cold,
            None => empty_class_data_cold(),
        }
    }

    #[inline]
    pub fn cold_mut(&mut self) -> &mut ClassDataCold {
        self.cold.get_or_insert_with(Default::default)
    }

    #[inline]
    pub fn cold_opt(&self) -> Option<&ClassDataCold> {
        self.cold.as_deref()
    }

    // an empty stub for an undefined constant receiver won't shadow the lazy loader's real definition.
    pub fn has_type_substance(&self) -> bool {
        self.user_defined
            || self.loc.is_some()
            || self.file_path.is_some()
            || !self.methods.is_empty()
            || self.superclass.is_some()
            || !self.mixins.is_empty()
    }

    pub fn merge_dirty_pattern(&mut self, src: &DirtyPattern) {
        if src.is_empty() {
            return;
        }
        self.cold_mut()
            .dirty_method_pattern
            .get_or_insert_with(Default::default)
            .merge_from(src);
    }

    pub fn shrink_to_fit_after_compact(&mut self) {
        for method in &mut self.methods {
            // `make_mut` on a shared `Arc` deep-clones even for a no-op shrink, so check `needs_shrink` first.
            if method.needs_shrink() {
                Arc::make_mut(method).shrink_to_fit_after_collect();
            }
        }
        self.shrink_containers_after_freeze();
    }
    pub fn shrink_containers_after_freeze(&mut self) {
        self.methods.shrink_to_fit();
        self.method_index.freeze();
        self.method_index.shrink_to_fit();
        self.method_file_paths.freeze();
        self.method_file_paths.shrink_to_fit();
        self.constants.shrink_to_fit();
        self.ivars.shrink_to_fit();
        for types in self.ivars.values_mut() {
            types.shrink_to_fit();
        }
        self.call_sites.shrink_to_fit();
        for call_site in self.call_sites.owned_sites_mut() {
            call_site.arg_types.shrink_to_fit();
            call_site.keyword_arg_types.shrink_to_fit();
        }
        self.mixins.shrink_to_fit();
        if let Some(hooks) = self.hook_mixins.as_deref_mut() {
            hooks.included.shrink_to_fit();
            hooks.extended.shrink_to_fit();
            hooks.prepended.shrink_to_fit();
        }
        let Some(cold) = self.cold.as_deref_mut() else {
            return;
        };
        cold.method_visibility.shrink_to_fit();
        cold.bare_ivar_readers.shrink_to_fit();
        cold.undefined_methods.shrink_to_fit();
        cold.singleton_ivars.shrink_to_fit();
        for types in cold.singleton_ivars.values_mut() {
            types.shrink_to_fit();
        }
        cold.class_variables.shrink_to_fit();
        for types in cold.class_variables.values_mut() {
            types.shrink_to_fit();
        }
        cold.initialize_param_passthroughs.shrink_to_fit();
        cold.annotated_params.shrink_to_fit();
        cold.superclass_type_args.shrink_to_fit();
        cold.required_ancestors.shrink_to_fit();
        cold.required_ancestor_type_args.shrink_to_fit();
        cold.sorbet_modifier_comments.shrink_to_fit();
        for comment in &mut cold.sorbet_modifier_comments {
            comment.shrink_to_fit();
        }
        cold.class_type_params.shrink_to_fit();
        cold.class_type_param_bounds.shrink_to_fit();
        cold.class_type_param_defaults.shrink_to_fit();
    }

    pub fn hook_mixins(&self) -> Option<&HookMixins> {
        self.hook_mixins.as_deref()
    }

    pub fn hook_mixins_mut(&mut self) -> &mut HookMixins {
        self.hook_mixins.get_or_insert_default()
    }

    pub fn hook_mixins_by_kind(&self, kind: &MixinKind) -> &[Mixin] {
        self.hook_mixins
            .as_deref()
            .map_or(&[][..], |hooks| hooks.by_kind(kind))
    }

    pub(crate) fn container_bytes(&self) -> usize {
        let mut bytes = self.methods.len() * std::mem::size_of::<Arc<MethodDef>>();
        bytes += self.method_index.container_bytes();
        bytes += self.method_file_paths.container_bytes();
        bytes += self
            .call_site_fingerprints
            .as_ref()
            .map_or(0, |fingerprints| {
                map_shell_bytes(fingerprints.len(), 8)
                    + std::mem::size_of::<std::collections::HashSet<u64>>()
            });
        bytes += self.call_sites.segment_count()
            * (std::mem::size_of::<CallSiteSegment>() + std::mem::size_of::<u32>());
        let mixin_bytes = |mixins: &[Mixin]| {
            std::mem::size_of_val(mixins)
                + mixins
                    .iter()
                    .map(|m| m.type_args.len() * std::mem::size_of::<rbs_ir::RbsType>())
                    .sum::<usize>()
        };
        bytes += mixin_bytes(&self.mixins);
        if let Some(hooks) = self.hook_mixins.as_deref() {
            bytes += std::mem::size_of::<HookMixins>();
            bytes += mixin_bytes(&hooks.included);
            bytes += mixin_bytes(&hooks.extended);
            bytes += mixin_bytes(&hooks.prepended);
        }
        let Some(cold) = self.cold_opt() else {
            return bytes;
        };
        // once cold is boxed, the shell itself also lives on the heap.
        bytes += std::mem::size_of::<ClassDataCold>();
        bytes += cold.undefined_methods.len() * std::mem::size_of::<(SharedName, bool)>();
        bytes += cold.method_aliases.len() * std::mem::size_of::<MethodAlias>();
        bytes += cold
            .method_aliases
            .iter()
            .map(|a| a.new_name.capacity() + a.old_name.capacity())
            .sum::<usize>();
        bytes += map_shell_bytes(
            cold.method_visibility.len(),
            std::mem::size_of::<((Sym, bool), Visibility)>(),
        );
        bytes += map_shell_bytes(
            cold.bare_ivar_readers.len(),
            std::mem::size_of::<(Sym, bool)>(),
        );
        bytes += map_shell_bytes(
            cold.initialize_param_passthroughs.len(),
            std::mem::size_of::<(SharedName, Vec<SharedName>)>(),
        );
        bytes += map_shell_bytes(
            cold.annotated_params.len(),
            std::mem::size_of::<(SharedName, FxHashMap<usize, Type>)>(),
        );
        for params in cold.annotated_params.values() {
            bytes += map_shell_bytes(params.len(), std::mem::size_of::<(usize, Type)>());
            bytes += params.values().map(Type::deep_extra_bytes).sum::<usize>();
        }
        bytes += cold.required_ancestors.len() * std::mem::size_of::<SharedName>();
        bytes += cold.required_ancestor_type_args.len() * std::mem::size_of::<rbs_ir::RbsType>();
        bytes += string_vec_bytes(&cold.sorbet_modifier_comments);
        bytes += string_vec_bytes(&cold.class_type_params);
        bytes +=
            cold.class_type_param_bounds.len() * std::mem::size_of::<(String, rbs_ir::RbsType)>();
        bytes += cold.class_type_param_defaults.len() * std::mem::size_of::<(String, Type)>();
        bytes
    }

    pub(crate) fn constant_ivar_bytes(&self) -> usize {
        let mut bytes = map_shell_bytes(
            self.constants.len(),
            std::mem::size_of::<(Sym, ConstantDef)>(),
        );
        for constant in self.constants.values() {
            bytes += constant.const_type.deep_extra_bytes();
        }
        let ivar_map_bytes = |map: &FxHashMap<Sym, Vec<Type>>| {
            let mut bytes = map_shell_bytes(map.len(), std::mem::size_of::<(Sym, Vec<Type>)>());
            for types in map.values() {
                bytes += types.len() * std::mem::size_of::<Type>();
                bytes += types.iter().map(Type::deep_extra_bytes).sum::<usize>();
            }
            bytes
        };
        bytes += ivar_map_bytes(&self.ivars);
        if let Some(cold) = self.cold_opt() {
            bytes += ivar_map_bytes(&cold.singleton_ivars);
            bytes += ivar_map_bytes(&cold.class_variables);
        }
        bytes
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RegistryDeepBytes {
    pub methods_walked: usize,

    pub methods_new: usize,

    pub methods_shared_prior: usize,
    pub method_body_bytes: usize,
    pub call_site_count: usize,
    pub call_site_bytes: usize,

    pub container_bytes: usize,
    pub constant_ivar_bytes: usize,
    pub total_bytes: usize,
}

impl RegistryDeepBytes {
    pub fn accumulate(&mut self, other: &RegistryDeepBytes) {
        self.methods_walked += other.methods_walked;
        self.methods_new += other.methods_new;
        self.methods_shared_prior += other.methods_shared_prior;
        self.method_body_bytes += other.method_body_bytes;
        self.call_site_count += other.call_site_count;
        self.call_site_bytes += other.call_site_bytes;
        self.container_bytes += other.container_bytes;
        self.constant_ivar_bytes += other.constant_ivar_bytes;
        self.total_bytes += other.total_bytes;
    }
}

fn map_shell_bytes(len: usize, kv_size: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let capacity = (len * 8 / 7 + 1).next_power_of_two();
    capacity * (kv_size + 1) + 48
}

fn string_vec_bytes(strings: &[String]) -> usize {
    std::mem::size_of_val(strings) + strings.iter().map(String::capacity).sum::<usize>()
}

fn param_vec_bytes(params: &[Param]) -> usize {
    std::mem::size_of_val(params)
        + params
            .iter()
            .map(|p| p.name.capacity() + p.param_type.deep_extra_bytes())
            .sum::<usize>()
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RegistryMemoryBreakdown {
    pub classes: usize,
    pub methods_total: usize,
    pub methods_shared: usize,
    pub call_sites_total: usize,
    pub constants_total: usize,
    pub ivars_total: usize,
    pub param_cache_entries: usize,
    pub name_pool_entries: usize,
}

type ParamCacheKey = (SharedName, SharedName, bool);

// the param cache is sharded with lazy init (avoids mutex transport during prewarm and dead weight in per-file snapshots).
type ParamCacheShard = Mutex<FxHashMap<ParamCacheKey, Arc<Vec<Param>>>>;

#[derive(Debug, Default)]
struct ParamCache {
    shards: std::sync::OnceLock<Vec<ParamCacheShard>>,
}

impl ParamCache {
    fn len(&self) -> usize {
        self.shards
            .get()
            .map(|shards| {
                shards
                    .iter()
                    .map(|shard| shard.lock().map(|s| s.len()).unwrap_or(0))
                    .sum()
            })
            .unwrap_or(0)
    }

    /// Estimated resident bytes for `TYDA_MEMORY_BREAKDOWN` reporting (off the hot path):
    /// key bytes (two interned names + the bool discriminant) plus each cached `Vec<Param>`
    /// via the same `deep_extra_bytes` convention as `param_vec_bytes`.
    fn deep_bytes(&self) -> usize {
        self.shards
            .get()
            .map(|shards| {
                shards
                    .iter()
                    .map(|shard| {
                        let Ok(guard) = shard.lock() else {
                            return 0;
                        };
                        let mut bytes = map_shell_bytes(
                            guard.len(),
                            std::mem::size_of::<(ParamCacheKey, Arc<Vec<Param>>)>(),
                        );
                        for (key, params) in guard.iter() {
                            bytes += key.0.len() + key.1.len();
                            bytes += param_vec_bytes(params);
                        }
                        bytes
                    })
                    .sum()
            })
            .unwrap_or(0)
    }

    fn shard_index(key: &ParamCacheKey) -> usize {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % 16
    }

    fn shard(&self, key: &ParamCacheKey) -> Option<&ParamCacheShard> {
        self.shards
            .get()
            .map(|shards| &shards[Self::shard_index(key)])
    }

    fn shard_or_init(&self, key: &ParamCacheKey) -> &ParamCacheShard {
        let shards = self
            .shards
            .get_or_init(|| (0..16).map(|_| Mutex::new(FxHashMap::default())).collect());
        &shards[Self::shard_index(key)]
    }

    fn get(&self, key: &ParamCacheKey) -> Option<Vec<Param>> {
        // deep-cloning inside the lock serializes prewarm workers via the shard mutex, so `Arc` clone happens inside the lock and Vec materialization happens outside.
        let shared = self.shard(key)?.lock().ok()?.get(key).cloned()?;
        Some((*shared).clone())
    }

    // when a shard exceeds its limit, clear it per-shard (bounds long-lived LSP sessions).
    fn insert_capped(&self, key: ParamCacheKey, params: Vec<Param>, shard_cap: usize) {
        let params = Arc::new(params);
        if let Ok(mut shard) = self.shard_or_init(&key).lock() {
            if shard.len() >= shard_cap {
                shard.clear();
                shard.shrink_to_fit();
            }
            shard.insert(key, params);
        }
    }

    fn insert_uncapped(&self, key: ParamCacheKey, params: Vec<Param>) {
        let params = Arc::new(params);
        if let Ok(mut shard) = self.shard_or_init(&key).lock() {
            shard.insert(key, params);
        }
    }

    fn clear(&self) {
        if let Some(shards) = self.shards.get() {
            for shard in shards {
                if let Ok(mut guard) = shard.lock() {
                    guard.clear();
                    guard.shrink_to_fit();
                }
            }
        }
    }

    fn deallocate(&mut self) {
        self.shards.take();
    }
}

type CallSiteIndexEntry = std::sync::Arc<(u32, FxHashMap<(SharedName, bool), Vec<u32>>)>;

// call site index: avoids linear scans for popular classes (revision prevents staleness).
#[derive(Debug, Default)]
struct CallSiteIndexCache {
    shards: std::sync::OnceLock<Vec<std::sync::RwLock<FxHashMap<SharedName, CallSiteIndexEntry>>>>,
}

impl CallSiteIndexCache {
    const LINEAR_SCAN_THRESHOLD: usize = 256;

    fn shard(&self, key: &str) -> &std::sync::RwLock<FxHashMap<SharedName, CallSiteIndexEntry>> {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        key.hash(&mut hasher);
        let shards = self.shards.get_or_init(|| {
            (0..16)
                .map(|_| std::sync::RwLock::new(FxHashMap::default()))
                .collect()
        });
        &shards[(hasher.finish() as usize) % shards.len()]
    }
}

type FirstOwnerEntry = Option<(SharedName, bool)>;

/// first-owner memo: without it, re-running ancestor DFS dominates prewarm cost (only while `owner_lookup_cache_enabled`).
type FirstOwnerShard =
    std::sync::RwLock<FxHashMap<(SharedName, SharedName, bool), FirstOwnerEntry>>;

#[derive(Debug, Default)]
struct FirstOwnerCache {
    shards: std::sync::OnceLock<Vec<FirstOwnerShard>>,
}

impl FirstOwnerCache {
    fn shard(&self, key: &(SharedName, SharedName, bool)) -> &FirstOwnerShard {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        key.hash(&mut hasher);
        let shards = self.shards.get_or_init(|| {
            (0..16)
                .map(|_| std::sync::RwLock::new(FxHashMap::default()))
                .collect()
        });
        &shards[(hasher.finish() as usize) % shards.len()]
    }
}

/// a context-free `resolve_attr_reader_return_type` outcome plus the return-type
/// reads it performed; a cache hit replays the reads so slot wake-sets stay identical.
#[derive(Debug)]
struct AttrReaderReturn {
    ty: Option<Type>,
    reads: Vec<(Sym, Sym)>,
}

type AttrReaderReturnKey = (Sym, Sym, bool);
type AttrReaderReturnShard =
    std::sync::RwLock<FxHashMap<AttrReaderReturnKey, Arc<AttrReaderReturn>>>;

/// attr-reader return memo (sharded): recomputing it dominates the method-return-ref
/// fixpoint, where one round asks the same handful of keys thousands of times.
#[derive(Debug, Default)]
struct AttrReaderReturnCache {
    shards: std::sync::OnceLock<Vec<AttrReaderReturnShard>>,
}

impl AttrReaderReturnCache {
    fn shards(&self) -> &Vec<AttrReaderReturnShard> {
        self.shards.get_or_init(|| {
            (0..16)
                .map(|_| std::sync::RwLock::new(FxHashMap::default()))
                .collect()
        })
    }

    fn shard_index(key: &AttrReaderReturnKey) -> usize {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % 16
    }

    fn get(&self, key: &AttrReaderReturnKey) -> Option<Arc<AttrReaderReturn>> {
        let shards = self.shards.get()?;
        shards[Self::shard_index(key)]
            .read()
            .ok()?
            .get(key)
            .cloned()
    }

    fn insert(&self, key: AttrReaderReturnKey, entry: Arc<AttrReaderReturn>) {
        if let Ok(mut shard) = self.shards()[Self::shard_index(&key)].write() {
            shard.insert(key, entry);
        }
    }

    fn clear(&self) {
        if let Some(shards) = self.shards.get() {
            for shard in shards {
                if let Ok(mut guard) = shard.write() {
                    guard.clear();
                    guard.shrink_to_fit();
                }
            }
        }
    }
}

/// how owner cache hits are returned: a single owner is `Direct`, subclass/includer fallback is `Union`.
#[derive(Debug, Clone, Copy)]
enum OwnerListKind {
    Direct,
    Union,
}

type OwnerListEntry = Option<(OwnerListKind, Arc<[(Sym, bool)]>)>;
type OwnerLookupKey = (SharedName, SharedName, bool);

/// method-owner cache (sharded): memoizes ownership after structure freeze (including negative results). Avoids lock contention during parallel prewarm.
#[derive(Debug, Default)]
struct OwnerLookupCache {
    shards: std::sync::OnceLock<Vec<std::sync::RwLock<FxHashMap<OwnerLookupKey, OwnerListEntry>>>>,
}

impl OwnerLookupCache {
    fn shard_index(key: &OwnerLookupKey) -> usize {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % 16
    }

    fn get(&self, key: &OwnerLookupKey) -> Option<OwnerListEntry> {
        let shards = self.shards.get()?;
        shards[Self::shard_index(key)]
            .read()
            .ok()?
            .get(key)
            .cloned()
    }

    fn insert(&self, key: OwnerLookupKey, entry: OwnerListEntry) {
        let shards = self.shards.get_or_init(|| {
            (0..16)
                .map(|_| std::sync::RwLock::new(FxHashMap::default()))
                .collect()
        });
        if let Ok(mut shard) = shards[Self::shard_index(&key)].write() {
            shard.insert(key, entry);
        }
    }

    fn clear(&self) {
        if let Some(shards) = self.shards.get() {
            for shard in shards {
                if let Ok(mut guard) = shard.write() {
                    guard.clear();
                    guard.shrink_to_fit();
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct TypeRegistry {
    // boxed: a 392-byte `ClassData` inline makes the map shell dominate per-file snapshots.
    class_data: FxHashMap<Sym, Box<ClassData>>,
    /// boxed: none of these are populated in a per-file snapshot, and their empty map
    /// headers would otherwise add 336B to every one of them.
    cold_tail: Option<Box<RegistryColdTail>>,
    // Classes this engine defined or extended during the current analysis pass
    // (not copied by workspace preload). Used to scope Full resolution to the
    // file being analyzed; `ClassData.file_path` is first-writer-wins so Object
    // keeps the stdlib path.
    file_contribution_names: HashSet<String>,
    file_contribution_method_names: HashSet<Sym>,
    name_pool_enabled: bool,
    has_mixin_hook_mixins: bool,
    has_mixin_hook_methods: bool,
    mixin_hook_mixins_applied: bool,
    has_includer_bound_dsl: bool,
    includer_bound_dsl_applied: bool,
    /// skip the dirty-synthesis path entirely for registries with no dirty patterns (avoids per-miss matching cost for non-Rails).
    has_dirty_patterns: bool,
    /// resolved-param cache: valid because the registry is immutable during resolution. Call `invalidate_resolve_cache` after mutation.
    resolve_params_cache: ParamCache,
    /// frozen param cache: write-locked to make parallel render deterministic. Inserting under the degraded guard would leak scheduling-dependent signatures.
    resolve_params_cache_frozen: std::sync::atomic::AtomicBool,
    /// owner lookup cache: only consulted after structural mutation completes (`owner_lookup_cache_enabled`).
    owner_lookup_cache: OwnerLookupCache,
    owner_lookup_cache_enabled: std::sync::atomic::AtomicBool,
    /// attr-reader return cache: only consulted while `attr_reader_return_cache_enabled`,
    /// i.e. inside one method-return-ref fixpoint round, where the registry is frozen.
    attr_reader_return_cache: AttrReaderReturnCache,
    attr_reader_return_cache_enabled: std::sync::atomic::AtomicBool,
    first_owner_cache: FirstOwnerCache,
    call_site_index: CallSiteIndexCache,
    /// method aliases unresolved during merge are re-resolved by `finalize_pending_method_aliases` after full load.
    pending_method_aliases: Vec<PendingMethodAlias>,
    /// forward references inside scoped types are resolved in lexical order by `finalize_pending_scoped_type_refs` after merge.
    pending_scoped_type_refs: Vec<PendingScopedTypeRef>,
}

/// Registry state a per-file snapshot never populates, kept behind one box.
#[derive(Debug, Default, Clone)]
struct RegistryColdTail {
    type_aliases: HashMap<String, Type>,
    global_variables: HashMap<String, Type>,
    method_block_meta: FxHashMap<SharedName, ClassMethodBlockMeta>,
    body_fact_class_names: HashSet<String>,
    name_pool: HashSet<SharedName>,
    /// Reverse index: superclass name -> list of direct subclass names.
    /// Built lazily by `build_subclass_index` and invalidated on class mutations.
    subclass_index: Option<FxHashMap<SharedName, Vec<SharedName>>>,
    /// reverse lookup of module includers (built together with `build_subclass_index`): turns the including-classes fallback from an O(N) scan into O(1).
    module_includer_index: Option<FxHashMap<SharedName, Vec<SharedName>>>,
    /// known gem/Rails namespaces: suppress `unresolved_constant` even without bundled types.
    known_constant_namespaces: HashSet<String>,
}

fn empty_registry_cold_tail() -> &'static RegistryColdTail {
    use std::sync::OnceLock;
    static EMPTY: OnceLock<RegistryColdTail> = OnceLock::new();
    EMPTY.get_or_init(RegistryColdTail::default)
}

/// Bounds one method-return slot's deferred-reference expansion without
/// imposing a low fixed depth on normal recursive inference.
struct GlobalResolveBudget {
    remaining_nodes: usize,
    exhausted: bool,
}

impl GlobalResolveBudget {
    const NODE_LIMIT: usize = 100_000;

    fn new() -> Self {
        Self {
            remaining_nodes: Self::NODE_LIMIT,
            exhausted: false,
        }
    }

    fn consume(&mut self) -> bool {
        if self.remaining_nodes == 0 {
            self.exhausted = true;
            return false;
        }
        self.remaining_nodes -= 1;
        true
    }

    fn is_exhausted(&self) -> bool {
        self.exhausted
    }
}

impl Clone for TypeRegistry {
    fn clone(&self) -> Self {
        Self {
            class_data: self.class_data.clone(),
            cold_tail: self.cold_tail.clone(),
            file_contribution_names: self.file_contribution_names.clone(),
            file_contribution_method_names: self.file_contribution_method_names.clone(),
            name_pool_enabled: self.name_pool_enabled,
            has_mixin_hook_mixins: self.has_mixin_hook_mixins,
            has_mixin_hook_methods: self.has_mixin_hook_methods,
            mixin_hook_mixins_applied: self.mixin_hook_mixins_applied,
            has_includer_bound_dsl: self.has_includer_bound_dsl,
            includer_bound_dsl_applied: self.includer_bound_dsl_applied,
            has_dirty_patterns: self.has_dirty_patterns,
            resolve_params_cache: ParamCache::default(),
            resolve_params_cache_frozen: std::sync::atomic::AtomicBool::new(false),
            owner_lookup_cache: OwnerLookupCache::default(),
            owner_lookup_cache_enabled: std::sync::atomic::AtomicBool::new(false),
            attr_reader_return_cache: AttrReaderReturnCache::default(),
            attr_reader_return_cache_enabled: std::sync::atomic::AtomicBool::new(false),
            first_owner_cache: FirstOwnerCache::default(),
            call_site_index: CallSiteIndexCache::default(),
            pending_method_aliases: self.pending_method_aliases.clone(),
            pending_scoped_type_refs: self.pending_scoped_type_refs.clone(),
        }
    }
}

/// a single method alias (`alias hi hello`). `new_name` is an alias for `old_name`.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodAlias {
    pub new_name: String,
    pub old_name: String,
    pub is_singleton: bool,
    pub loc: Option<SourceLocation>,
}

/// pending info for a method alias that couldn't be resolved at merge time. Pushed when the class/
/// module holding the target hasn't been merged yet; re-resolved during finalize after full load.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingMethodAlias {
    pub(crate) class_name: String,
    pub(crate) old_name: String,
    pub(crate) new_name: String,
    pub(crate) is_singleton: bool,
}

/// pending nominal forward reference inside a type: per-file only has the bare name; finalize writes it back in lexical order after merge.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingScopedTypeRef {
    /// the owner class that holds the method (where the reader/writer gets registered).
    pub(crate) owner_class: String,
    /// the method name to write back (`memcache` for the reader, `memcache=` for the writer).
    pub(crate) method_name: String,
    pub(crate) is_singleton: bool,
    pub(crate) declaration_scope: String,
    /// the type keeps the nominal name (`Memcache` / `::Foo::Bar`) exactly as written in source.
    /// a bare nominal is `Class(name)`, a compound type is e.g. `Array[Class(name)]`.
    pub(crate) raw_type: Type,
}

impl TypeRegistry {
    pub(crate) fn push_pending_method_alias(&mut self, pending: PendingMethodAlias) {
        // don't duplicate the same entry (so pending entries don't grow unbounded when
        // the same per-file registry gets merged in multiple times via lazy merge).
        if !self.pending_method_aliases.contains(&pending) {
            self.pending_method_aliases.push(pending);
        }
    }

    pub(crate) fn take_pending_method_aliases(&mut self) -> Vec<PendingMethodAlias> {
        std::mem::take(&mut self.pending_method_aliases)
    }

    pub(crate) fn push_pending_scoped_type_ref(&mut self, pending: PendingScopedTypeRef) {
        // same as method aliases: reject duplicates so pending entries don't grow unbounded
        // even if the same per-file registry gets merged in multiple times via lazy merge.
        if !self.pending_scoped_type_refs.contains(&pending) {
            self.pending_scoped_type_refs.push(pending);
        }
    }

    pub(crate) fn take_pending_scoped_type_refs(&mut self) -> Vec<PendingScopedTypeRef> {
        std::mem::take(&mut self.pending_scoped_type_refs)
    }

    /// record a method alias (`alias new old`). An alias with the same (new, is_singleton)
    /// is overwritten by the later one (last-write-wins, respecting re-aliasing).
    pub(crate) fn record_method_alias(
        &mut self,
        class_name: &str,
        new_name: String,
        old_name: String,
        is_singleton: bool,
        loc: Option<SourceLocation>,
    ) {
        if new_name == old_name {
            return;
        }
        let cold = self.class_data_mut(class_name).cold_mut();
        cold.method_aliases
            .retain(|a| !(a.new_name == new_name && a.is_singleton == is_singleton));
        cold.method_aliases.push(MethodAlias {
            new_name,
            old_name,
            is_singleton,
            loc,
        });
    }

    /// follow aliases within a class to resolve to the canonical (non-alias) method name. Cycles
    /// are cut off by `MAX_RESOLVE_DEPTH`.
    pub(crate) fn canonical_method_name(
        &self,
        class_name: &str,
        name: &str,
        is_singleton: bool,
    ) -> String {
        let Some(data) = self.class_data.get(class_name) else {
            return name.to_string();
        };
        let mut current = name.to_string();
        for _ in 0..MAX_RESOLVE_DEPTH {
            let Some(alias) = data
                .cold()
                .method_aliases
                .iter()
                .find(|a| a.new_name == current && a.is_singleton == is_singleton)
            else {
                return current;
            };
            if alias.old_name == current {
                return current;
            }
            current = alias.old_name.clone();
        }
        current
    }

    /// union call sites within an alias group (Ruby aliases share the same body). Done once, after recording and before param resolution.
    pub fn merge_alias_call_sites(&mut self) {
        let class_names: Vec<String> = self
            .class_data
            .iter()
            .filter(|(_, d)| !d.cold().method_aliases.is_empty())
            .map(|(n, _)| n.to_string())
            .collect();
        for class_name in class_names {
            let Some(data) = self.class_data.get(class_name.as_str()) else {
                continue;
            };
            // collect all names involved in aliasing and group them by canonical name.
            let mut names: Vec<(String, bool)> = Vec::new();
            for a in &data.cold().method_aliases {
                names.push((a.new_name.clone(), a.is_singleton));
                names.push((a.old_name.clone(), a.is_singleton));
            }
            names.sort();
            names.dedup();
            let mut groups: FxHashMap<(String, bool), Vec<String>> = FxHashMap::default();
            for (name, is_singleton) in names {
                let canon = self.canonical_method_name(&class_name, &name, is_singleton);
                groups.entry((canon, is_singleton)).or_default().push(name);
            }

            let mut additions: Vec<CallSite> = Vec::new();
            {
                let data = &self.class_data[class_name.as_str()];
                for ((_canon, is_singleton), members) in &groups {
                    if members.len() < 2 {
                        // skip standalone names (no need to aggregate call sites).
                        continue;
                    }
                    let member_set: std::collections::HashSet<&str> =
                        members.iter().map(String::as_str).collect();
                    let union: Vec<CallSite> = data
                        .call_sites
                        .iter()
                        .filter(|cs| {
                            cs.method_is_singleton == *is_singleton
                                && member_set.contains(cs.method_name.as_ref())
                        })
                        .cloned()
                        .collect();
                    for member in members {
                        for cs in &union {
                            if cs.method_name.as_ref() != member.as_str() {
                                let mut copy = cs.clone();
                                copy.method_name = Arc::from(member.as_str());
                                additions.push(copy);
                            }
                        }
                    }
                }
            }
            if !additions.is_empty() {
                let data = self
                    .class_data
                    .get_mut(class_name.as_str())
                    .expect("class exists");
                data.call_sites.extend(additions);
                data.call_sites_revision = data.call_sites_revision.wrapping_add(1);
            }
        }
    }
}

impl TypeRegistry {
    const DEFERRED_REF_MAX_DEPTH: usize = 16;
    /// exceeding the `resolve_params_cache` limit drops the whole map (coarser than LRU but O(1) cap, avoids `Vec<Param>` bloat in long-lived LSP sessions).
    const RESOLVE_PARAMS_CACHE_CAP: usize = 8192;

    fn intern_name(&mut self, name: &str) -> SharedName {
        if !self.name_pool_enabled {
            return Arc::<str>::from(name);
        }
        if let Some(existing) = self.tail().name_pool.get(name) {
            return existing.clone();
        }
        let shared = Arc::<str>::from(name);
        self.tail_mut().name_pool.insert(shared.clone());
        shared
    }

    fn intern_shared_name(&mut self, name: &SharedName) -> SharedName {
        if !self.name_pool_enabled {
            return name.clone();
        }
        if let Some(existing) = self.tail().name_pool.get(name.as_ref()) {
            return existing.clone();
        }
        self.tail_mut().name_pool.insert(name.clone());
        name.clone()
    }

    fn shared_name(&self, name: &str) -> SharedName {
        if !self.name_pool_enabled {
            return Arc::<str>::from(name);
        }
        self.tail()
            .name_pool
            .get(name)
            .cloned()
            .unwrap_or_else(|| Arc::<str>::from(name))
    }

    // after freezing, only drop the collection-only scaffolding (`fingerprints` / `body_fact` / `name_pool`).
    pub fn drop_transient_collection_state(&mut self) {
        for data in self.class_data.values_mut() {
            data.call_site_fingerprints = None;
        }
        if let Some(tail) = self.tail_opt_mut() {
            tail.body_fact_class_names.clear();
            tail.body_fact_class_names.shrink_to_fit();
            tail.name_pool.clear();
            tail.name_pool.shrink_to_fit();
        }
    }
    pub fn mark_mixins_external(&mut self) {
        for data in self.class_data.values_mut() {
            for mixin in &mut data.mixins {
                mixin.external_source = true;
            }
            if let Some(hooks) = data.hook_mixins.as_deref_mut() {
                for mixin in hooks.iter_mut_all() {
                    mixin.external_source = true;
                }
            }
        }
    }

    pub fn shrink_to_fit_after_compact(&mut self) {
        self.class_data.shrink_to_fit();
        for data in self.class_data.values_mut() {
            data.shrink_to_fit_after_compact();
        }
        let name_pool_enabled = self.name_pool_enabled;
        if let Some(tail) = self.tail_opt_mut() {
            tail.type_aliases.shrink_to_fit();
            tail.global_variables.shrink_to_fit();
            tail.method_block_meta.shrink_to_fit();
            tail.body_fact_class_names.shrink_to_fit();
            // drop unreferenced name pool entries after compaction (don't keep every analyzed name pinned).
            if name_pool_enabled {
                tail.name_pool.shrink_to_fit();
            }
        }
        // bulk-drop the lookup cache: avoids shard/memo dead weight across thousands of per-file snapshots.
        self.resolve_params_cache.deallocate();
        self.owner_lookup_cache = OwnerLookupCache::default();
        self.first_owner_cache = FirstOwnerCache::default();
        self.call_site_index = CallSiteIndexCache::default();
    }

    // compact after batch freeze: keep the param cache to avoid breaking `Arc` sharing via `make_mut`.
    pub fn compact_after_batch_freeze(&mut self) {
        self.drop_transient_collection_state();
        // `call_sites` can't be dropped because render references it for attr-reader/block return types (pruning would change render output).
        self.class_data.shrink_to_fit();
        for data in self.class_data.values_mut() {
            data.shrink_containers_after_freeze();
        }
        if let Some(tail) = self.tail_opt_mut() {
            tail.type_aliases.shrink_to_fit();
            tail.global_variables.shrink_to_fit();
            tail.method_block_meta.shrink_to_fit();
        }
    }

    pub fn deep_breakdown(&self, seen: &mut FxHashSet<usize>) -> RegistryDeepBytes {
        let mut b = RegistryDeepBytes::default();
        b.container_bytes += map_shell_bytes(
            self.class_data.len(),
            std::mem::size_of::<Sym>() + std::mem::size_of::<Box<ClassData>>(),
        );
        b.container_bytes += self.class_data.len() * std::mem::size_of::<ClassData>();
        for data in self.class_data.values() {
            // key name bytes live in the global interner, attributed separately.
            b.container_bytes += data.container_bytes();
            b.constant_ivar_bytes += data.constant_ivar_bytes();
            // shared chunks are only charged to the first walk that reaches them (dedup via `Arc`
            // pointer). Shares the same seen-set with the snapshot-side summary.
            data.call_sites.for_each_attribution(|unit| match unit {
                CallSiteAttribution::SharedChunk(ptr, sites) => {
                    if !seen.insert(ptr) {
                        return;
                    }
                    for call_site in sites {
                        b.call_site_count += 1;
                        b.call_site_bytes += call_site.deep_bytes();
                    }
                }
                CallSiteAttribution::OwnedSite(call_site) => {
                    b.call_site_count += 1;
                    b.call_site_bytes += call_site.deep_bytes();
                }
            });
            for method in &data.methods {
                b.methods_walked += 1;
                if seen.insert(Arc::as_ptr(method) as usize) {
                    b.methods_new += 1;
                    b.method_body_bytes += method.deep_bytes();
                } else {
                    b.methods_shared_prior += 1;
                }
            }
        }
        b.container_bytes += map_shell_bytes(
            self.tail().method_block_meta.len(),
            std::mem::size_of::<(SharedName, ClassMethodBlockMeta)>(),
        );
        for meta in self.tail().method_block_meta.values() {
            b.container_bytes += meta.deep_bytes();
        }
        b.container_bytes += map_shell_bytes(self.tail().name_pool.len(), 16);
        b.container_bytes += self
            .tail()
            .name_pool
            .iter()
            .map(|name| name.len() + 16)
            .sum::<usize>();
        b.container_bytes += map_shell_bytes(
            self.tail().body_fact_class_names.len(),
            std::mem::size_of::<String>(),
        );
        b.total_bytes =
            b.container_bytes + b.constant_ivar_bytes + b.call_site_bytes + b.method_body_bytes;
        b
    }

    /// holder aggregation for `TYDA_MEMORY_BREAKDOWN` (off the hot path).
    pub fn memory_breakdown(&self) -> RegistryMemoryBreakdown {
        let mut b = RegistryMemoryBreakdown {
            classes: self.class_data.len(),
            ..Default::default()
        };
        for data in self.class_data.values() {
            b.methods_total += data.methods.len();
            for method in &data.methods {
                if Arc::strong_count(method) > 1 {
                    b.methods_shared += 1;
                }
            }
            b.call_sites_total += data.call_sites.len();
            b.constants_total += data.constants.len();
            b.ivars_total += data.ivars.len()
                + data.cold().singleton_ivars.len()
                + data.cold().class_variables.len();
        }
        b.param_cache_entries = self.resolve_params_cache.len();
        b.name_pool_entries = self.tail().name_pool.len();
        b
    }

    /// Estimated resident bytes of `resolve_params_cache`, for `TYDA_MEMORY_BREAKDOWN` reporting.
    pub fn resolve_params_cache_deep_bytes(&self) -> usize {
        self.resolve_params_cache.deep_bytes()
    }

    fn intern_method_block_meta_inner(&mut self, meta: &mut MethodBlockMeta) {
        if let Some((name, is_singleton)) = meta.forwarded_block.take() {
            meta.forwarded_block = Some((self.intern_shared_name(&name), is_singleton));
        }
    }

    fn intern_call_site(&mut self, call_site: &mut CallSite) {
        call_site.method_name = self.intern_shared_name(&call_site.method_name);
        let keyword_arg_types = std::mem::take(&mut call_site.keyword_arg_types);
        call_site.keyword_arg_types = keyword_arg_types
            .into_iter()
            .map(|(name, ty)| (self.intern_shared_name(&name), ty))
            .collect();
        if let Some(context) = &mut call_site.caller_context {
            context.class_name = self.intern_shared_name(&context.class_name);
            context.method_name = self.intern_shared_name(&context.method_name);
        }
    }

    fn intern_passthroughs(
        &mut self,
        passthroughs: HashMap<String, Vec<String>>,
    ) -> FxHashMap<SharedName, Vec<SharedName>> {
        passthroughs
            .into_iter()
            .map(|(param_name, targets)| {
                (
                    self.intern_name(&param_name),
                    targets
                        .into_iter()
                        .map(|target| self.intern_name(&target))
                        .collect(),
                )
            })
            .collect()
    }

    fn mark_class_has_method_body_facts(&mut self, class_name: &str) {
        self.tail_mut()
            .body_fact_class_names
            .insert(class_name.to_string());
    }

    fn call_site_summary_key(call_site: &CallSite) -> CallSiteSummaryKey {
        (
            call_site.method_name.clone(),
            call_site.method_is_singleton,
            call_site.caller_context.as_ref().map(|context| {
                (
                    context.class_name.clone(),
                    context.method_name.clone(),
                    context.method_is_singleton,
                )
            }),
        )
    }

    fn push_or_merge_call_site(grouped: &mut GroupedCallSites, call_site: CallSite) {
        let key = Self::call_site_summary_key(&call_site);
        match grouped.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().fold(call_site);
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(CallSiteSummaryAccumulator::new(call_site));
            }
        }
    }

    fn merge_summarized_call_sites(
        data: &mut ClassData,
        new_sites: impl IntoIterator<Item = CallSite>,
    ) {
        let mut grouped: GroupedCallSites = HashMap::new();
        for site in data.call_sites.take_all() {
            Self::push_or_merge_call_site(&mut grouped, site);
        }
        for site in new_sites {
            Self::push_or_merge_call_site(&mut grouped, site);
        }
        data.call_sites.replace_with(
            grouped
                .into_values()
                .map(CallSiteSummaryAccumulator::finish)
                .collect(),
        );
        data.call_sites_revision = data.call_sites_revision.wrapping_add(1);
        data.has_pending_call_site_summary = false;
    }

    fn positional_param_count(param_infos: &[ParamInfo]) -> usize {
        param_infos
            .iter()
            .filter(|pi| {
                matches!(
                    pi.kind,
                    ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                )
            })
            .count()
    }

    fn method_accepts_keywords(param_infos: &[ParamInfo]) -> bool {
        param_infos.iter().any(|pi| {
            matches!(
                pi.kind,
                ParamKind::KeywordRequired | ParamKind::KeywordOptional | ParamKind::DoubleRest
            )
        })
    }

    pub(crate) fn synthesize_keyword_hash_arg(
        call_site: &CallSite,
        param_infos: &[ParamInfo],
    ) -> Vec<Type> {
        let mut arg_types = call_site.arg_types.clone();
        if call_site.keyword_arg_types.is_empty() || Self::method_accepts_keywords(param_infos) {
            return arg_types;
        }

        let positional_count = Self::positional_param_count(param_infos);
        if arg_types.len() >= positional_count {
            return arg_types;
        }

        let mut fields: Vec<RecordField> = call_site
            .keyword_arg_types
            .iter()
            .map(|(name, ty)| RecordField {
                key: RecordKey::Symbol(name.to_string()),
                value: ty.clone(),
                optional: false,
            })
            .collect();
        fields.sort_by(|a, b| a.key.cmp(&b.key));
        arg_types.push(Type::Record(fields));
        arg_types
    }

    pub(crate) fn merge_call_site_positional_types(
        param_types: &mut [Vec<Type>],
        call_site: &CallSite,
        param_infos: &[ParamInfo],
    ) {
        let arg_types = Self::synthesize_keyword_hash_arg(call_site, param_infos);
        Self::merge_resolved_positional_arg_types(param_types, &arg_types, param_infos);
    }

    fn merge_resolved_positional_arg_types(
        param_types: &mut [Vec<Type>],
        arg_types: &[Type],
        param_infos: &[ParamInfo],
    ) {
        let positional_infos: Vec<_> = param_infos
            .iter()
            .filter(|pi| {
                matches!(
                    pi.kind,
                    ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                )
            })
            .collect();
        let Some(rest_index) = positional_infos
            .iter()
            .position(|pi| matches!(pi.kind, ParamKind::Rest))
        else {
            for (i, arg_type) in arg_types.iter().enumerate() {
                if i < param_types.len() {
                    Type::append_union_parts(&mut param_types[i], arg_type.clone());
                }
            }
            return;
        };

        let trailing_count = positional_infos.len().saturating_sub(rest_index + 1);
        for (i, arg_type) in arg_types.iter().take(rest_index).enumerate() {
            if i < param_types.len() {
                Type::append_union_parts(&mut param_types[i], arg_type.clone());
            }
        }

        let trailing_start = arg_types.len().saturating_sub(trailing_count);
        for (offset, arg_type) in arg_types.iter().skip(trailing_start).enumerate() {
            let param_index = rest_index + 1 + offset;
            if param_index < param_types.len() {
                Type::append_union_parts(&mut param_types[param_index], arg_type.clone());
            }
        }

        let middle_end = trailing_start.max(rest_index);
        for arg_type in arg_types
            .iter()
            .skip(rest_index)
            .take(middle_end - rest_index)
        {
            if rest_index < param_types.len() {
                Type::append_union_parts(&mut param_types[rest_index], arg_type.clone());
            }
        }
    }

    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn tail(&self) -> &RegistryColdTail {
        match &self.cold_tail {
            Some(tail) => tail,
            None => empty_registry_cold_tail(),
        }
    }

    #[inline]
    fn tail_mut(&mut self) -> &mut RegistryColdTail {
        self.cold_tail.get_or_insert_with(Default::default)
    }

    /// `&mut` access that stays a no-op (and keeps the box unallocated) when the tail is empty.
    #[inline]
    fn tail_opt_mut(&mut self) -> Option<&mut RegistryColdTail> {
        self.cold_tail.as_deref_mut()
    }

    #[inline]
    fn invalidate_reverse_indexes(&mut self) {
        if let Some(tail) = self.tail_opt_mut() {
            tail.subclass_index = None;
            tail.module_includer_index = None;
        }
    }

    /// framework (see field docs). Merges into any existing set.
    pub fn add_known_constant_namespaces<I: IntoIterator<Item = String>>(&mut self, names: I) {
        self.tail_mut().known_constant_namespaces.extend(names);
    }
    pub fn is_known_constant_namespace(&self, name: &str) -> bool {
        if self.tail().known_constant_namespaces.is_empty() {
            return false;
        }
        let root = name.trim_scope_prefix().split("::").next().unwrap_or(name);
        self.tail().known_constant_namespaces.contains(root)
    }

    pub fn new_pooled() -> Self {
        Self {
            name_pool_enabled: true,
            ..Self::default()
        }
    }
    /// Destructive holder-bisection for the memory-breakdown bench only.
    #[cfg(test)]
    pub fn debug_drop_alias_maps(&mut self) {
        if let Some(tail) = self.tail_opt_mut() {
            tail.type_aliases = HashMap::new();
            tail.global_variables = HashMap::new();
            tail.known_constant_namespaces = HashSet::new();
            tail.body_fact_class_names = HashSet::new();
        }
        self.pending_method_aliases = Vec::new();
        self.pending_scoped_type_refs = Vec::new();
    }
    /// Destructive holder-bisection for the memory-breakdown bench only.
    #[cfg(test)]
    pub fn debug_drop_lookup_caches(&mut self) {
        self.resolve_params_cache = ParamCache::default();
        self.owner_lookup_cache = OwnerLookupCache::default();
        self.first_owner_cache = FirstOwnerCache::default();
        self.call_site_index = CallSiteIndexCache::default();
        self.invalidate_reverse_indexes();
        if let Some(tail) = self.tail_opt_mut() {
            tail.name_pool = HashSet::new();
        }
    }
    /// Destructive holder-bisection for the memory-breakdown bench only.
    #[cfg(test)]
    pub fn debug_drop_annotated_params(&mut self) {
        for data in self.class_data.values_mut() {
            data.cold_mut().annotated_params = FxHashMap::default();
        }
    }
    /// Destructive holder-bisection for the memory-breakdown bench only.
    #[cfg(test)]
    pub fn debug_drop_method_bodies(&mut self) {
        for data in self.class_data.values_mut() {
            data.methods = Vec::new();
            data.method_index = MethodIndex::default();
        }
    }
    /// Destructive holder-bisection for the memory-breakdown bench only.
    #[cfg(test)]
    pub fn debug_drop_constants_ivars(&mut self) {
        for data in self.class_data.values_mut() {
            data.constants = FxHashMap::default();
            data.ivars = FxHashMap::default();
            data.cold_mut().singleton_ivars = FxHashMap::default();
            data.cold_mut().class_variables = FxHashMap::default();
        }
    }

    pub fn iter_class_data(&self) -> impl Iterator<Item = (&Sym, &ClassData)> {
        self.class_data.iter().map(|(name, data)| (name, &**data))
    }

    pub fn get_superclass(&self, class_name: &str) -> Option<&str> {
        self.class_data
            .get(class_name)
            .and_then(|d| d.superclass.as_deref())
    }

    /// Return the statically known runtime ancestor chain in Ruby's lookup order.
    ///
    /// `None` means that an edge in the chain is unknown or the bounded walk was
    /// not able to prove a complete result. Required ancestors are intentionally
    /// excluded: they affect type requirements, not `Module#ancestors`.
    pub fn ordered_ancestor_names(&self, class_name: &str) -> Option<Vec<SharedName>> {
        let mut names = Vec::with_capacity(16);
        let mut seen = FxHashSet::default();
        let mut active = FxHashSet::default();
        self.collect_ordered_ancestor_names(class_name, &mut seen, &mut active, &mut names)
            .then_some(names)
    }

    fn collect_ordered_ancestor_names(
        &self,
        class_name: &str,
        seen: &mut FxHashSet<SharedName>,
        active: &mut FxHashSet<SharedName>,
        names: &mut Vec<SharedName>,
    ) -> bool {
        let class_name = self.resolve_scoped_class_ref_borrow("", class_name);
        let name = self.shared_name(class_name);
        if seen.contains(&name) {
            return !active.contains(&name);
        }
        if names.len() >= MAX_EXACT_ANCESTOR_CHAIN_LENGTH {
            return false;
        }

        let Some(data) = self.class_data.get(class_name) else {
            return false;
        };
        if !data.has_type_substance() {
            return false;
        }

        let is_module = data.is_module;
        let superclass = data.superclass.clone();
        seen.insert(name.clone());
        active.insert(name.clone());

        for mixin in data.mixins.iter().rev() {
            if mixin.kind != MixinKind::Prepend {
                continue;
            }
            let mixin_name =
                self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
            if !self.collect_ordered_ancestor_names(mixin_name, seen, active, names) {
                return false;
            }
        }

        names.push(name.clone());

        for mixin in data.mixins.iter().rev() {
            if mixin.kind != MixinKind::Include {
                continue;
            }
            let mixin_name =
                self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
            if !self.collect_ordered_ancestor_names(mixin_name, seen, active, names) {
                return false;
            }
        }

        let complete = if let Some(superclass) = superclass.as_deref() {
            let superclass_name = self.resolve_scoped_class_ref_borrow(class_name, superclass);
            self.collect_ordered_ancestor_names(superclass_name, seen, active, names)
        } else if is_module {
            true
        } else if class_name == "Object" {
            // Object's implicit BasicObject edge is supplied by the stdlib RBS.
            false
        } else if class_name == "BasicObject" {
            true
        } else {
            self.collect_ordered_ancestor_names("Object", seen, active, names)
        };

        active.remove(&name);
        complete
    }

    pub fn get_required_ancestors(&self, class_name: &str) -> &[SharedName] {
        self.class_data
            .get(class_name)
            .map(|d| d.cold().required_ancestors.as_slice())
            .unwrap_or(&[])
    }

    pub fn mark_user_defined(&mut self, class_name: &str) {
        let data = self.class_data_mut(class_name);
        data.user_defined = true;
    }
    pub fn method_defs_len(&self, class_name: &str) -> usize {
        self.class_data
            .get(class_name)
            .map_or(0, |data| data.methods.len())
    }
    pub fn mark_methods_synthetic_dsl_from(&mut self, class_name: &str, start: usize) {
        let Some(data) = self.class_data.get_mut(class_name) else {
            return;
        };
        for method in data.methods.iter_mut().skip(start) {
            if !method.rbs_file_source || !method.synthetic_dsl_source {
                let inner = Arc::make_mut(method);
                inner.rbs_file_source = true;
                inner.synthetic_dsl_source = true;
            }
        }
    }

    pub fn add_method_def(&mut self, class_name: &str, mut method: MethodDef) {
        method.shrink_to_fit_after_collect();
        if Self::method_needs_mixin_hook_call_site(&method) {
            self.has_mixin_hook_methods = true;
            self.mixin_hook_mixins_applied = false;
        }
        let is_user = !method.rbs_file_source && !method.synthetic_dsl_source;
        if is_user {
            self.file_contribution_names.insert(class_name.to_string());
            self.file_contribution_method_names.insert(method.name);
        }
        let data = self.class_data_mut(class_name);
        let method_file_path = method.loc.and_then(|_| data.file_path.clone());
        if is_user {
            data.user_defined = true;
        }
        if let Some(existing_idx) = data.methods.iter().position(|existing| {
            existing.name == method.name && existing.is_singleton == method.is_singleton
        }) {
            let existing = &data.methods[existing_idx];
            if existing.rbs_file_source && !method.rbs_file_source {
                let next_idx = data.methods.len();
                data.methods.push(Arc::new(method));
                let method_name = data.methods[next_idx].name;
                let is_singleton = data.methods[next_idx].is_singleton;
                Self::set_method_slot(data, method_name, is_singleton, next_idx);
                if let Some(file_path) = method_file_path {
                    data.method_file_paths
                        .insert((method_name, is_singleton), file_path);
                }
                return;
            }
        }

        Self::index_method_if_absent(data, method.name, method.is_singleton, data.methods.len());
        if let Some(file_path) = method_file_path {
            data.method_file_paths
                .insert((method.name, method.is_singleton), file_path);
        }
        data.methods.push(Arc::new(method));
    }

    pub fn add_method_def_if_missing(&mut self, class_name: &str, method: MethodDef) -> bool {
        if self.has_method_variant(class_name, &method.name, method.is_singleton) {
            return false;
        }
        // skip materializing if the dirty skeleton pattern can synthesize a method with the same name (old first-wins behavior = byte-compatible render).
        if !method.is_singleton
            && let Some(pattern) = self
                .class_data
                .get(class_name)
                .and_then(|d| d.cold().dirty_method_pattern.as_ref())
            && pattern.synthesize(method.name.as_str()).is_some()
        {
            return false;
        }
        self.add_method_def(class_name, method);
        true
    }

    /// register dirty pattern: only add unknown columns (avoids duplicates from `table_name` override), sets `has_dirty_patterns`.
    pub(crate) fn mark_has_dirty_patterns(&mut self) {
        self.has_dirty_patterns = true;
    }

    /// static evaluation of `column_names`: dirty pattern column order, and STI inherits via the superclass chain.
    pub fn schema_column_names(&self, class_name: &str) -> Option<Vec<String>> {
        let mut current = Some(class_name.to_string());
        let mut depth = 0;
        while let Some(cls) = current {
            if depth >= MAX_RESOLVE_DEPTH {
                break;
            }
            depth += 1;
            let data = self.class_data.get(cls.as_str())?;
            if let Some(pattern) = data.cold().dirty_method_pattern.as_ref() {
                return Some(
                    pattern
                        .columns
                        .iter()
                        .map(|(name, _)| name.as_str().to_string())
                        .collect(),
                );
            }
            current = data.superclass.as_ref().map(|sc| {
                self.resolve_scoped_class_ref_borrow(&cls, sc.as_ref())
                    .to_string()
            });
        }
        None
    }

    /// Column accessor type from schema/`DirtyPattern`, walking the superclass
    /// chain the same way as `schema_column_names`. Used when `attribute :x`
    /// (and other column-backed DSL) omits a type keyword.
    pub fn schema_column_accessor_type(&self, class_name: &str, column: &str) -> Option<Type> {
        let mut current = Some(class_name.to_string());
        let mut depth = 0;
        while let Some(cls) = current {
            if depth >= MAX_RESOLVE_DEPTH {
                break;
            }
            depth += 1;
            let Some(data) = self.class_data.get(cls.as_str()) else {
                break;
            };
            if let Some(pattern) = data.cold().dirty_method_pattern.as_ref()
                && let Some(base) = pattern.column_type(column)
            {
                if let Some(method) = Self::method_for_lookup_kind(data, column, Some(false))
                    && !matches!(method.raw_return_type, Type::Untyped)
                {
                    return Some(method.raw_return_type.clone());
                }
                return Some(crate::rails::nullable_column_accessor_type(base, true));
            }
            current = data.superclass.as_ref().map(|sc| {
                self.resolve_scoped_class_ref_borrow(&cls, sc.as_ref())
                    .to_string()
            });
        }
        None
    }

    pub fn register_dirty_pattern_columns(&mut self, class_name: &str, columns: Vec<(Sym, Type)>) {
        if columns.is_empty() {
            return;
        }
        self.has_dirty_patterns = true;
        let pattern = self
            .class_data_mut(class_name)
            .cold_mut()
            .dirty_method_pattern
            .get_or_insert_with(Default::default);
        for (name, ty) in columns {
            if !pattern
                .columns
                .iter()
                .any(|(existing, _)| *existing == name)
            {
                pattern.columns.push((name, ty));
            }
        }
    }

    pub fn set_method_block_meta(
        &mut self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
        meta: MethodBlockMeta,
    ) {
        self.mark_class_has_method_body_facts(class_name);
        let class_name = self.intern_name(class_name);
        let method_name = self.intern_name(method_name);
        let mut meta = meta;
        self.intern_method_block_meta_inner(&mut meta);
        self.tail_mut()
            .method_block_meta
            .entry(class_name)
            .or_default()
            .insert(method_name, is_singleton, meta);
    }

    pub fn set_initialize_param_passthroughs(
        &mut self,
        class_name: &str,
        passthroughs: HashMap<String, Vec<String>>,
    ) {
        let passthroughs = self.intern_passthroughs(passthroughs);
        self.class_data_mut(class_name)
            .cold_mut()
            .initialize_param_passthroughs = passthroughs;
    }

    pub fn lookup_method_block_meta(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<&MethodBlockMeta> {
        self.tail()
            .method_block_meta
            .get(class_name)
            .and_then(|meta| meta.get(method_name, is_singleton))
    }

    pub fn set_type_alias(&mut self, alias_name: &str, ty: Type) {
        self.tail_mut()
            .type_aliases
            .insert(alias_name.to_string(), ty);
    }

    pub fn type_aliases(&self) -> &HashMap<String, Type> {
        &self.tail().type_aliases
    }

    pub fn set_global_variable_type(&mut self, name: &str, ty: Type) {
        self.tail_mut()
            .global_variables
            .insert(name.to_string(), ty);
    }

    /// global `$g` is a flow-insensitive union for cross-method reads (intra-method reads prefer the engine side).
    pub fn add_global_variable_type(&mut self, name: &str, ty: Type) {
        match self.tail().global_variables.get(name) {
            Some(existing) if existing == &ty => {}
            Some(existing) => {
                let merged = Type::from_type_vec(vec![existing.clone(), ty]);
                self.tail_mut()
                    .global_variables
                    .insert(name.to_string(), merged);
            }
            None => {
                self.tail_mut()
                    .global_variables
                    .insert(name.to_string(), ty);
            }
        }
    }

    pub fn lookup_global_variable_type(&self, name: &str) -> Option<Type> {
        self.tail().global_variables.get(name).cloned()
    }

    pub fn remove_method_variant(
        &mut self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) {
        let mut removed = false;
        if let Some(data) = self.class_data.get_mut(class_name)
            && let Some(idx) = data.methods.iter().position(|method| {
                method.name == method_name && method.is_singleton == is_singleton
            })
        {
            let removed_name = data.methods[idx].name;
            data.methods.remove(idx);
            data.method_file_paths.remove(&(removed_name, is_singleton));
            data.method_index.clear();
            let names: Vec<Sym> = data.methods.iter().map(|method| method.name).collect();
            for (new_idx, name) in names.into_iter().enumerate() {
                let is_singleton = data.methods[new_idx].is_singleton;
                Self::index_method_if_absent(data, name, is_singleton, new_idx);
            }
            removed = true;
        }
        if removed {
            self.refresh_mixin_hook_method_flag();
            self.mixin_hook_mixins_applied = false;
        }
    }

    pub fn strip_methods_defined_in(&mut self, file_path: &str) {
        let mut removed = false;
        for data in self.class_data.values_mut() {
            let drop_all = matches!(
                &data.method_file_paths,
                MethodFilePaths::Uniform(uniform) if uniform.path.as_ref() == file_path
            );
            if drop_all {
                removed |= !data.methods.is_empty();
                data.methods.clear();
                data.method_index.clear();
                data.method_file_paths = MethodFilePaths::Empty;
                continue;
            }
            let MethodFilePaths::PerMethod(map) = &data.method_file_paths else {
                continue;
            };
            let drop_keys: HashSet<MethodKey> = map
                .iter()
                .filter(|(_, path)| path.as_ref() == file_path)
                .map(|(key, _)| *key)
                .collect();
            if drop_keys.is_empty() {
                continue;
            }
            removed = true;
            data.methods
                .retain(|method| !drop_keys.contains(&(method.name, method.is_singleton)));
            data.method_file_paths
                .retain_paths(|path| path.as_ref() != file_path);
            data.method_index.clear();
            let names: Vec<(Sym, bool)> = data
                .methods
                .iter()
                .map(|method| (method.name, method.is_singleton))
                .collect();
            for (idx, (name, is_singleton)) in names.into_iter().enumerate() {
                Self::index_method_if_absent(data, name, is_singleton, idx);
            }
        }
        if removed {
            self.refresh_mixin_hook_method_flag();
            self.mixin_hook_mixins_applied = false;
        }
    }

    pub fn undef_method_variant(
        &mut self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) {
        self.remove_method_variant(class_name, method_name, is_singleton);
        let method_name = self.intern_name(method_name);
        let cold = self.class_data_mut(class_name).cold_mut();
        if !cold
            .undefined_methods
            .iter()
            .any(|(name, singleton)| name == &method_name && *singleton == is_singleton)
        {
            cold.undefined_methods.push((method_name, is_singleton));
        }
    }

    pub fn add_call_site(&mut self, class_name: &str, call_site: CallSite) {
        let mut call_site = call_site;
        self.intern_call_site(&mut call_site);
        self.mark_class_has_method_body_facts(class_name);
        let data = self.class_data_mut(class_name);
        if !data
            .call_site_fingerprints
            .get_or_insert_default()
            .insert(call_site_fingerprint(&call_site))
        {
            return;
        }
        data.has_pending_call_site_summary = true;
        data.call_sites.push(call_site);
        data.call_sites_revision = data.call_sites_revision.wrapping_add(1);
    }
    pub fn finalize_pending_call_site_summaries(&mut self) {
        let mut had_pending_call_site_summaries = false;
        for data in self.class_data.values_mut() {
            if !data.has_pending_call_site_summary {
                continue;
            }
            had_pending_call_site_summaries = true;
            if data.call_sites.len() <= 1 {
                data.has_pending_call_site_summary = false;
                continue;
            }
            let mut grouped: GroupedCallSites = HashMap::new();
            for site in data.call_sites.take_all() {
                Self::push_or_merge_call_site(&mut grouped, site);
            }
            data.call_sites.replace_with(
                grouped
                    .into_values()
                    .map(CallSiteSummaryAccumulator::finish)
                    .collect(),
            );
            data.call_sites_revision = data.call_sites_revision.wrapping_add(1);
            data.has_pending_call_site_summary = false;
        }
        if had_pending_call_site_summaries {
            self.invalidate_resolve_cache();
        }
    }

    /// Drop parameter signatures cached before the registry's call sites settled.
    pub(crate) fn invalidate_resolve_cache(&mut self) {
        self.resolve_params_cache.clear();
    }

    pub fn set_constant(
        &mut self,
        class_name: &str,
        constant_name: &str,
        const_type: Type,
        loc: Option<SourceLocation>,
        file_path: Option<&str>,
    ) {
        let data = self.class_data_mut(class_name);
        data.user_defined = true;
        let name = Sym::new(constant_name);
        data.constants.insert(
            name,
            ConstantDef {
                name,
                const_type,
                loc,
                file_path: file_path.map(SharedPath::from),
                external_source: false,
            },
        );
    }

    pub fn lookup_constant_type(&self, class_name: &str, constant_name: &str) -> Option<Type> {
        let data = self.class_data.get(class_name)?;
        data.constants
            .get(constant_name)
            .map(|def| def.const_type.clone())
    }

    pub fn lookup_constant_definition_location(
        &self,
        class_name: &str,
        constant_name: &str,
    ) -> Option<(String, SourceLocation)> {
        let data = self.class_data.get(class_name)?;
        let def = data.constants.get(constant_name)?;
        let file_path = def.file_path.as_deref()?;
        let loc = def.loc?;
        Some((file_path.to_string(), loc))
    }

    pub fn lookup_constant_definition_location_through_ancestors(
        &self,
        class_name: &str,
        constant_name: &str,
    ) -> Option<(String, SourceLocation)> {
        let mut seen = Vec::new();
        self.lookup_constant_definition_location_through_ancestors_inner(
            class_name,
            constant_name,
            &mut seen,
        )
    }

    fn lookup_constant_definition_location_through_ancestors_inner<'a>(
        &'a self,
        class_name: &'a str,
        constant_name: &str,
        seen: &mut Vec<&'a str>,
    ) -> Option<(String, SourceLocation)> {
        if seen.contains(&class_name) {
            return None;
        }
        seen.push(class_name);
        if let Some(location) = self.lookup_constant_definition_location(class_name, constant_name)
        {
            return Some(location);
        }
        let data = self.class_data.get(class_name)?;
        for mixin in data.mixins.iter().rev() {
            if matches!(mixin.kind, MixinKind::Include | MixinKind::Prepend)
                && let Some(location) = self
                    .lookup_constant_definition_location_through_ancestors_inner(
                        mixin.module_name.as_ref(),
                        constant_name,
                        seen,
                    )
            {
                return Some(location);
            }
        }
        if let Some(superclass) = &data.superclass
            && let Some(location) = self
                .lookup_constant_definition_location_through_ancestors_inner(
                    superclass.as_ref(),
                    constant_name,
                    seen,
                )
        {
            return Some(location);
        }
        None
    }

    /// constant lookup that also walks the ancestor chain (mixins/superclass) -- Ruby bare constant resolution.
    pub fn lookup_constant_through_ancestors(
        &self,
        class_name: &str,
        constant_name: &str,
    ) -> Option<Type> {
        let mut seen = Vec::new();
        self.lookup_constant_through_ancestors_inner(class_name, constant_name, &mut seen)
    }

    pub fn constant_completion_candidates_for_namespace(
        &self,
        namespace: &str,
        class_context: &str,
    ) -> Vec<ConstantCompletionCandidate> {
        let mut items = BTreeMap::new();
        let mut seen_owners = HashSet::new();
        for owner_path in self.constant_completion_owner_paths(namespace, class_context) {
            if !seen_owners.insert(owner_path.clone()) {
                continue;
            }
            self.collect_constant_completion_candidates_from_owner(&owner_path, &mut items);
        }
        items.into_values().collect()
    }

    pub fn resolve_class_name_for_type_name(
        &self,
        type_name: &str,
        class_context: &str,
    ) -> Option<String> {
        let type_name = type_name.trim();
        if type_name.is_empty() {
            return None;
        }
        let bare = type_name.strip_prefix("::").unwrap_or(type_name);
        if bare.is_empty() {
            return None;
        }

        let candidates = if type_name.starts_with("::") {
            vec![bare.to_string()]
        } else {
            Self::scoped_constant_candidates_for_context(bare, class_context)
        };

        for candidate in candidates {
            if let Some(target) =
                self.resolve_constant_alias_target_for_completion(&candidate, class_context)
            {
                return Some(target);
            }
        }
        None
    }

    fn constant_completion_owner_paths(&self, namespace: &str, class_context: &str) -> Vec<String> {
        let namespace = namespace.trim();
        if namespace.is_empty() || namespace == "::" {
            return vec![String::new()];
        }

        let bare = namespace.strip_prefix("::").unwrap_or(namespace);
        if bare.is_empty() {
            return vec![String::new()];
        }

        let mut owners = Vec::new();
        let candidates = if namespace.starts_with("::") {
            vec![bare.to_string()]
        } else {
            Self::scoped_constant_candidates_for_context(bare, class_context)
        };

        for candidate in candidates {
            self.push_constant_completion_owner_candidate(&candidate, class_context, &mut owners);
        }
        owners
    }

    fn push_constant_completion_owner_candidate(
        &self,
        candidate: &str,
        class_context: &str,
        owners: &mut Vec<String>,
    ) {
        if self.has_class(candidate) {
            owners.push(candidate.to_string());
        }
        if let Some(target) =
            self.resolve_constant_alias_target_for_completion(candidate, class_context)
        {
            owners.push(target);
        }
        if let Some((prefix, last)) = candidate.rsplit_once("::")
            && let Some(target_prefix) =
                self.resolve_constant_alias_target_for_completion(prefix, class_context)
        {
            let aliased = crate::sym::join_scope(&target_prefix, last);
            if self.has_class(&aliased) {
                owners.push(aliased.clone());
            }
            if let Some(target) = self.resolve_constant_alias_target_for_completion(&aliased, "") {
                owners.push(target);
            }
        }
    }

    fn resolve_constant_alias_target_for_completion(
        &self,
        path: &str,
        class_context: &str,
    ) -> Option<String> {
        let mut current = path.trim_scope_prefix().to_string();
        let mut seen = HashSet::new();
        for _ in 0..16 {
            if current.is_empty() {
                return Some(String::new());
            }
            if self.has_class(&current) {
                return Some(current);
            }
            if !seen.insert(current.clone()) {
                return None;
            }
            let ty = self.constant_type_for_path_without_alias(&current, class_context)?;
            match ty {
                Type::Singleton(target) | Type::Class(target) => {
                    let target = Self::strip_type_arguments(&target).to_string();
                    if target == current {
                        return self.has_class(&target).then_some(target);
                    }
                    current = target;
                }
                _ => return None,
            }
        }
        None
    }

    fn constant_type_for_path_without_alias(
        &self,
        path: &str,
        class_context: &str,
    ) -> Option<Type> {
        let bare = path.strip_prefix("::").unwrap_or(path);
        if bare.is_empty() {
            return None;
        }

        if path.starts_with("::") {
            return self.constant_type_for_absolute_path(bare);
        }

        if let Some((prefix, const_name)) = bare.rsplit_once("::") {
            for owner in Self::scoped_constant_candidates_for_context(prefix, class_context) {
                if let Some(ty) = self.lookup_constant_through_ancestors(&owner, const_name) {
                    return Some(ty);
                }
            }
            return self.lookup_constant_through_ancestors(prefix, const_name);
        }

        for candidate in Self::scoped_constant_candidates_for_context(bare, class_context) {
            if let Some((owner, const_name)) = candidate.rsplit_once("::")
                && let Some(ty) = self.lookup_constant_type(owner, const_name)
            {
                return Some(ty);
            }
        }
        if !class_context.is_empty()
            && let Some(ty) = self.lookup_constant_through_ancestors(class_context, bare)
        {
            return Some(ty);
        }
        self.lookup_constant_through_ancestors("Object", bare)
    }

    fn constant_type_for_absolute_path(&self, bare: &str) -> Option<Type> {
        if let Some((owner, const_name)) = bare.rsplit_once("::") {
            self.lookup_constant_through_ancestors(owner, const_name)
        } else {
            self.lookup_constant_through_ancestors("Object", bare)
        }
    }

    fn scoped_constant_candidates_for_context(bare: &str, class_context: &str) -> Vec<String> {
        let mut candidates = Vec::new();
        if !class_context.is_empty() {
            let mut scope: Option<&str> = Some(class_context);
            while let Some(s) = scope {
                candidates.push(crate::sym::join_scope(s, bare));
                scope = s.rsplit_once("::").map(|(parent, _)| parent);
            }
        }
        candidates.push(bare.to_string());
        candidates
    }

    fn collect_constant_completion_candidates_from_owner(
        &self,
        owner_path: &str,
        items: &mut BTreeMap<String, ConstantCompletionCandidate>,
    ) {
        for class_name in self.class_names() {
            let Some(name) = Self::direct_nested_constant_name(owner_path, &class_name) else {
                continue;
            };
            let Some(data) = self.class_data_for(&class_name) else {
                continue;
            };
            let kind = if data.is_module {
                ConstantCompletionKind::Module
            } else {
                ConstantCompletionKind::Class
            };
            items.insert(
                name.clone(),
                ConstantCompletionCandidate {
                    name,
                    full_name: class_name.clone(),
                    kind,
                    const_type: Some(Type::Singleton(Sym::new(class_name))),
                },
            );
        }

        let owner_class = if owner_path.is_empty() {
            "Object"
        } else {
            owner_path
        };
        if let Some(data) = self.class_data_for(owner_class) {
            for (name, constant) in &data.constants {
                let full_name = Self::constant_full_name(owner_path, name.as_str());
                items
                    .entry(name.to_string())
                    .or_insert_with(|| ConstantCompletionCandidate {
                        name: name.to_string(),
                        full_name,
                        kind: ConstantCompletionKind::Constant,
                        const_type: Some(constant.const_type.clone()),
                    });
            }
        }
    }

    fn direct_nested_constant_name(owner_path: &str, class_name: &str) -> Option<String> {
        let rest = if owner_path.is_empty() {
            class_name
        } else {
            class_name.strip_prefix(owner_path)?.strip_prefix("::")?
        };
        (!rest.is_empty() && !rest.contains("::")).then(|| rest.to_string())
    }

    fn constant_full_name(owner_path: &str, constant_name: &str) -> String {
        if owner_path.is_empty() {
            constant_name.to_string()
        } else {
            crate::sym::join_scope(owner_path, constant_name)
        }
    }

    fn strip_type_arguments(class_name: &str) -> &str {
        class_name
            .find('[')
            .map(|idx| &class_name[..idx])
            .unwrap_or(class_name)
    }

    /// bare nested-namespace reference from an includer: resolves the FQN `{ancestor}::{name}` up the ancestor chain.
    pub fn resolve_nested_namespace_through_ancestors(
        &self,
        class_name: &str,
        name: &str,
    ) -> Option<String> {
        let mut seen = Vec::new();
        self.resolve_nested_namespace_through_ancestors_inner(class_name, name, &mut seen)
    }

    fn resolve_nested_namespace_through_ancestors_inner<'a>(
        &'a self,
        class_name: &'a str,
        name: &str,
        seen: &mut Vec<&'a str>,
    ) -> Option<String> {
        if seen.contains(&class_name) {
            return None;
        }
        seen.push(class_name);
        let data = self.class_data.get(class_name)?;
        let nested = crate::sym::join_scope(class_name, name);
        if self.class_data.contains_key(nested.as_str()) {
            return Some(nested);
        }
        // mixin/superclass names are first resolved to an FQN in the enclosing scope, just like
        // `lookup_constant_through_ancestors_inner`, then walked (supports nested includes).
        for mixin in data.mixins.iter().rev() {
            if matches!(mixin.kind, MixinKind::Include | MixinKind::Prepend) {
                let resolved =
                    self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
                if let Some(found) =
                    self.resolve_nested_namespace_through_ancestors_inner(resolved, name, seen)
                {
                    return Some(found);
                }
            }
        }
        if let Some(superclass) = &data.superclass {
            let resolved = self.resolve_scoped_class_ref_borrow(class_name, superclass.as_ref());
            if let Some(found) =
                self.resolve_nested_namespace_through_ancestors_inner(resolved, name, seen)
            {
                return Some(found);
            }
        }
        None
    }

    fn lookup_constant_through_ancestors_inner<'a>(
        &'a self,
        class_name: &'a str,
        constant_name: &str,
        seen: &mut Vec<&'a str>,
    ) -> Option<Type> {
        if seen.contains(&class_name) {
            return None;
        }
        seen.push(class_name);
        let data = self.class_data.get(class_name)?;
        if let Some(def) = data.constants.get(constant_name) {
            return Some(def.const_type.clone());
        }
        // shortened mixin/superclass names are resolved to an FQN in the enclosing scope before being walked.
        for mixin in data.mixins.iter().rev() {
            if matches!(mixin.kind, MixinKind::Include | MixinKind::Prepend) {
                let resolved =
                    self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
                if let Some(ty) =
                    self.lookup_constant_through_ancestors_inner(resolved, constant_name, seen)
                {
                    return Some(ty);
                }
            }
        }
        if let Some(superclass) = &data.superclass {
            let resolved = self.resolve_scoped_class_ref_borrow(class_name, superclass.as_ref());
            if let Some(ty) =
                self.lookup_constant_through_ancestors_inner(resolved, constant_name, seen)
            {
                return Some(ty);
            }
        }
        None
    }

    /// for `attr_reader`, return the ivar's type if the backing ivar is concrete, even when the raw type is Untyped.
    pub(crate) fn resolve_attr_reader_return_type(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<Type> {
        if !self
            .attr_reader_return_cache_enabled
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return self.resolve_attr_reader_return_type_inner(
                class_name,
                method_name,
                is_singleton,
            );
        }
        // only a call entered with the full depth budget and no attr-reader guard held is
        // context-free; a nested one can be cut short by either guard, so its result is
        // not a function of the key alone.
        let context_free = RESOLVE_DEPTH.with(|depth| depth.get()) == 0
            && ATTR_READER_VISITING.with(|cell| cell.borrow().is_empty());
        if !context_free {
            return self.resolve_attr_reader_return_type_inner(
                class_name,
                method_name,
                is_singleton,
            );
        }
        let key = (Sym::new(class_name), Sym::new(method_name), is_singleton);
        if let Some(entry) = self.attr_reader_return_cache.get(&key) {
            for &(owner, method) in &entry.reads {
                note_return_type_read(owner, method);
            }
            return entry.ty.clone();
        }
        let outer = RETURN_TYPE_READS.with(|cell| cell.borrow_mut().replace(FxHashSet::default()));
        let ty = self.resolve_attr_reader_return_type_inner(class_name, method_name, is_singleton);
        let inner = RETURN_TYPE_READS.with(|cell| {
            let mut slot = cell.borrow_mut();
            let inner = slot.take().unwrap_or_default();
            *slot = outer;
            if let Some(set) = slot.as_mut() {
                set.extend(inner.iter().copied());
            }
            inner
        });
        self.attr_reader_return_cache.insert(
            key,
            Arc::new(AttrReaderReturn {
                ty: ty.clone(),
                reads: inner.into_iter().collect(),
            }),
        );
        ty
    }

    fn resolve_attr_reader_return_type_inner(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<Type> {
        // the ivar name comes from the class supplying the method, but ivar state is looked up on the receiver class (it gets assigned where included).
        let ivar = self.attr_ivar_name_through_ancestors(class_name, method_name, is_singleton)?;
        // the attr-reader->initialize->param->attr-reader mutual recursion is cut off via `visiting`.
        let _attr_guard = AttrReaderVisitGuard::enter(class_name, &ivar, is_singleton)?;
        let data = self.class_data.get(class_name)?;
        let types = if is_singleton {
            data.cold().singleton_ivars.get(ivar.as_str())
        } else {
            data.ivars.get(ivar.as_str())
        };
        if let Some(types) = types
            && !types.is_empty()
        {
            let ivar_ty = if types.len() == 1 {
                types[0].clone()
            } else {
                Type::from_type_vec(types.clone())
            };
            if !matches!(
                ivar_ty,
                Type::Untyped
                    | Type::ParamRef(_)
                    | Type::KeywordParamRef(_)
                    | Type::IvarRef(_)
                    | Type::MethodReturnRef(..)
                    | Type::ReceiverMethodRef(..)
            ) {
                return Some(ivar_ty);
            }
        }
        if !is_singleton
            && let Some(ty) = self.infer_attr_type_from_initialize(class_name, &ivar)
            && Self::is_concrete_for_global_resolve(&ty)
        {
            return Some(ty);
        }
        None
    }

    fn attr_ivar_name_through_ancestors(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<String> {
        let (owner, owner_is_singleton) =
            self.resolve_method_call_owner_ref(class_name, method_name, is_singleton)?;
        let data = self.class_data.get(owner)?;
        Self::method_for_lookup_kind(data, method_name, Some(owner_is_singleton))
            .and_then(|method| method.attr_ivar.clone())
    }

    /// determines whether a method is a pure reader (attr/ivar/AR column reader, excluding associations): used for self-fact narrowing. Also walks the mixin chain.
    pub(crate) fn is_pure_ivar_reader_method(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> bool {
        let mut seen = Vec::new();
        self.is_pure_ivar_reader_through_ancestors(class_name, method_name, is_singleton, &mut seen)
            .unwrap_or(false)
    }

    fn is_pure_ivar_reader_through_ancestors<'a>(
        &'a self,
        class_name: &'a str,
        method_name: &str,
        is_singleton: bool,
        seen: &mut Vec<&'a str>,
    ) -> Option<bool> {
        if seen.contains(&class_name) {
            return None;
        }
        seen.push(class_name);
        let data = self.class_data.get(class_name)?;

        if !is_singleton {
            for mixin in data.mixins.iter().rev() {
                if mixin.kind != MixinKind::Prepend {
                    continue;
                }
                let mixin_ref =
                    self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
                if let Some(result) =
                    self.is_pure_ivar_reader_through_ancestors(mixin_ref, method_name, false, seen)
                {
                    return Some(result);
                }
            }
        }

        // a hand-written bare ivar reader (`def x = @x` / `def x; @x; end`).
        if !is_singleton
            && data
                .cold()
                .bare_ivar_readers
                .contains(&(Sym::new(method_name), false))
        {
            return Some(true);
        }
        // comes from `attr_reader` / `attr_accessor` (holds a backing ivar).
        if let Some(method) = Self::method_for_lookup_kind(data, method_name, Some(is_singleton)) {
            return Some(method.attr_ivar.is_some());
        }
        // a DB column reader from the AR schema (matches a column name; excludes associations/writers/predicates).
        if !is_singleton
            && let Some(pattern) = data.cold().dirty_method_pattern.as_ref()
            && pattern.has_column(method_name)
        {
            return Some(true);
        }

        if !is_singleton {
            for mixin in data.mixins.iter().rev() {
                if mixin.kind != MixinKind::Include {
                    continue;
                }
                let mixin_ref =
                    self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
                if let Some(result) =
                    self.is_pure_ivar_reader_through_ancestors(mixin_ref, method_name, false, seen)
                {
                    return Some(result);
                }
            }
        }
        if let Some(superclass) = &data.superclass
            && let Some(result) = self.is_pure_ivar_reader_through_ancestors(
                self.resolve_scoped_class_ref_borrow(class_name, superclass.as_ref()),
                method_name,
                is_singleton,
                seen,
            )
        {
            return Some(result);
        }
        None
    }

    fn enable_owner_lookup_cache(&self) {
        self.owner_lookup_cache.clear();
        self.owner_lookup_cache_enabled
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn apply_global_resolution(&mut self) {
        self.apply_mixin_hook_mixins();
        self.propagate_call_sites_for_hover();
        self.build_subclass_index();
        self.finalize_pending_scoped_type_refs();
        self.enable_owner_lookup_cache();
        self.resolve_subclass_method_refs_global();
        self.resolve_method_param_refs_from_call_sites();
        self.resolve_param_refs_global();
        self.resolve_method_return_refs_global();
    }

    pub fn apply_display_resolution_for_targets(&mut self, target_classes: &HashSet<String>) {
        if target_classes.is_empty() {
            self.apply_global_resolution();
            return;
        }
        let timing = std::env::var_os("TYDA_RESOLUTION_TIMING").is_some();
        let mut stamp = std::time::Instant::now();
        let report = |label: &str, stamp: &mut std::time::Instant| {
            if timing {
                eprintln!(
                    "display-resolution {label}: {:.3}s",
                    stamp.elapsed().as_secs_f64()
                );
            }
            *stamp = std::time::Instant::now();
        };
        self.apply_mixin_hook_mixins();
        report("apply_mixin_hook_mixins", &mut stamp);
        self.propagate_call_sites_for_target_classes(target_classes);
        report("propagate_call_sites_for_target_classes", &mut stamp);
        self.build_subclass_index();
        report("build_subclass_index", &mut stamp);
        self.finalize_pending_scoped_type_refs();
        report("finalize_pending_scoped_type_refs", &mut stamp);
        self.enable_owner_lookup_cache();
        report("enable_owner_lookup_cache", &mut stamp);
        self.resolve_subclass_method_refs_global();
        report("resolve_subclass_method_refs_global", &mut stamp);
        let target_class_names: Vec<Sym> = self
            .user_defined_class_names_unsorted()
            .into_iter()
            .filter(|class_name| target_classes.contains(class_name.as_str()))
            .collect();
        self.resolve_method_param_refs_from_call_sites_for_classes(&target_class_names);
        report(
            "resolve_method_param_refs_from_call_sites_for_classes",
            &mut stamp,
        );
        self.resolve_param_refs_for_classes(&target_class_names);
        report("resolve_param_refs_for_classes", &mut stamp);
        self.resolve_method_return_refs_for_classes(&target_class_names);
        report("resolve_method_return_refs_for_classes", &mut stamp);
    }

    /// CLI rendering keeps propagated user-defined call sites so rendered
    /// signatures can still show project-derived parameter types.
    pub fn apply_cli_resolution(&mut self) {
        let timing = std::env::var_os("TYDA_RESOLUTION_TIMING").is_some();
        let mut stamp = std::time::Instant::now();
        let report = |label: &str, stamp: &mut std::time::Instant| {
            if timing {
                eprintln!(
                    "cli-resolution {label}: {:.3}s",
                    stamp.elapsed().as_secs_f64()
                );
            }
            *stamp = std::time::Instant::now();
        };
        self.apply_mixin_hook_mixins();
        report("mixin-hooks", &mut stamp);
        self.build_subclass_index();
        self.finalize_pending_scoped_type_refs();
        self.enable_owner_lookup_cache();
        report("subclass-index", &mut stamp);
        self.resolve_subclass_method_refs_global();
        report("subclass-method-refs", &mut stamp);
        let target_classes: HashSet<String> = self
            .user_defined_class_names_unsorted()
            .into_iter()
            .map(|name| name.to_string())
            .collect();
        self.propagate_call_sites_for_target_classes(&target_classes);
        report("call-site-propagation", &mut stamp);
        self.resolve_method_param_refs_from_call_sites();
        report("method-param-refs", &mut stamp);
        self.resolve_param_refs_global();
        report("param-refs", &mut stamp);
        self.resolve_method_return_refs_global();
        report("method-return-refs", &mut stamp);
    }

    pub fn propagate_call_sites_for_hover(&mut self) {
        self.apply_mixin_hook_mixins();
        let class_names = self.class_names_unsorted();
        let class_index: HashMap<&str, usize> = class_names
            .iter()
            .enumerate()
            .map(|(idx, name)| (name.as_str(), idx))
            .collect();
        let mut propagated_to_owners: Vec<GroupedCallSites> = std::iter::repeat_with(HashMap::new)
            .take(class_names.len())
            .collect();
        let mut propagated_to_object: GroupedCallSites = HashMap::new();
        let mut mro_cache: FxHashMap<(usize, SharedName, bool), Vec<(usize, bool)>> =
            FxHashMap::default();

        for (class_idx, class_name) in class_names.iter().enumerate() {
            if class_name.as_str() == "Object" {
                continue;
            }
            let Some(data) = self.class_data.get(class_name) else {
                continue;
            };
            for site in &data.call_sites {
                if site.method_name.as_ref() != "initialize"
                    && !self.has_method_variant(
                        class_name,
                        site.method_name.as_ref(),
                        site.method_is_singleton,
                    )
                {
                    let cache_key = (
                        class_idx,
                        site.method_name.clone(),
                        site.method_is_singleton,
                    );
                    let owners = mro_cache.entry(cache_key).or_insert_with(|| {
                        self.resolve_method_call_owner_ref(
                            class_name,
                            site.method_name.as_ref(),
                            site.method_is_singleton,
                        )
                        .into_iter()
                        .filter_map(|(owner, is_singleton)| {
                            class_index
                                .get(owner)
                                .copied()
                                .map(|idx| (idx, is_singleton))
                        })
                        .collect()
                    });
                    if owners.is_empty() {
                        if !site.method_is_singleton {
                            Self::push_or_merge_call_site(&mut propagated_to_object, site.clone());
                        }
                    } else {
                        for (owner_idx, owner_method_is_singleton) in owners.iter() {
                            if *owner_idx != class_idx
                                || *owner_method_is_singleton != site.method_is_singleton
                            {
                                let mut propagated = site.clone();
                                propagated.method_is_singleton = *owner_method_is_singleton;
                                Self::push_or_merge_call_site(
                                    &mut propagated_to_owners[*owner_idx],
                                    propagated,
                                );
                            }
                        }
                    }
                }
            }
        }

        for (owner_idx, new_sites) in propagated_to_owners.into_iter().enumerate() {
            if new_sites.is_empty() {
                continue;
            }
            let data = self.class_data_mut(&class_names[owner_idx]);
            Self::merge_summarized_call_sites(
                data,
                new_sites
                    .into_values()
                    .map(CallSiteSummaryAccumulator::finish),
            );
        }
        let object_data = self.class_data_mut("Object");
        Self::merge_summarized_call_sites(
            object_data,
            propagated_to_object
                .into_values()
                .map(CallSiteSummaryAccumulator::finish),
        );
    }

    pub fn propagate_call_sites_for_target_classes(&mut self, target_classes: &HashSet<String>) {
        self.apply_mixin_hook_mixins();
        let include_object = target_classes.contains("Object");
        // Name only: `extend self` maps singleton calls onto instance methods.
        let target_method_names: Option<FxHashSet<Sym>> = if include_object {
            None
        } else {
            let mut names = FxHashSet::default();
            for class_name in target_classes {
                let Some(data) = self.class_data.get(class_name.as_str()) else {
                    continue;
                };
                for (method_name, slots) in data.method_index.iter() {
                    if slots.instance.is_some() || slots.singleton.is_some() {
                        names.insert(method_name);
                    }
                }
            }
            if names.is_empty() {
                return;
            }
            Some(names)
        };

        let class_names = self.class_names_unsorted();
        let class_index: HashMap<&str, usize> = class_names
            .iter()
            .enumerate()
            .map(|(idx, name)| (name.as_str(), idx))
            .collect();
        let target_flags: Vec<bool> = class_names
            .iter()
            .map(|name| target_classes.contains(name.as_str()))
            .collect();
        let mut propagated_to_owners: Vec<GroupedCallSites> = std::iter::repeat_with(HashMap::new)
            .take(class_names.len())
            .collect();
        let mut propagated_to_object: GroupedCallSites = HashMap::new();
        let mut mro_cache: FxHashMap<(usize, SharedName, bool), Vec<(usize, bool)>> =
            FxHashMap::default();

        for (class_idx, class_name) in class_names.iter().enumerate() {
            if class_name.as_str() == "Object" {
                continue;
            }
            let Some(data) = self.class_data.get(class_name) else {
                continue;
            };
            for site in &data.call_sites {
                if site.method_name.as_ref() == "initialize" {
                    continue;
                }
                if let Some(target_method_names) = target_method_names.as_ref()
                    && !target_method_names.contains(&Sym::new(site.method_name.as_ref()))
                {
                    continue;
                }
                if !self.has_method_variant(
                    class_name,
                    site.method_name.as_ref(),
                    site.method_is_singleton,
                ) {
                    let cache_key = (
                        class_idx,
                        site.method_name.clone(),
                        site.method_is_singleton,
                    );
                    let owners = mro_cache.entry(cache_key).or_insert_with(|| {
                        self.resolve_method_call_owner_ref(
                            class_name,
                            site.method_name.as_ref(),
                            site.method_is_singleton,
                        )
                        .into_iter()
                        .filter_map(|(owner, is_singleton)| {
                            class_index
                                .get(owner)
                                .copied()
                                .map(|idx| (idx, is_singleton))
                        })
                        .collect()
                    });
                    if owners.is_empty() {
                        if include_object && !site.method_is_singleton {
                            Self::push_or_merge_call_site(&mut propagated_to_object, site.clone());
                        }
                    } else {
                        for (owner_idx, owner_method_is_singleton) in owners.iter() {
                            if !target_flags[*owner_idx] {
                                continue;
                            }
                            if *owner_idx != class_idx
                                || *owner_method_is_singleton != site.method_is_singleton
                            {
                                let mut propagated = site.clone();
                                propagated.method_is_singleton = *owner_method_is_singleton;
                                Self::push_or_merge_call_site(
                                    &mut propagated_to_owners[*owner_idx],
                                    propagated,
                                );
                            }
                        }
                    }
                }
            }
        }

        for (owner_idx, new_sites) in propagated_to_owners.into_iter().enumerate() {
            if new_sites.is_empty() || !target_flags[owner_idx] {
                continue;
            }
            let data = self.class_data_mut(&class_names[owner_idx]);
            Self::merge_summarized_call_sites(
                data,
                new_sites
                    .into_values()
                    .map(CallSiteSummaryAccumulator::finish),
            );
        }

        if include_object {
            let object_data = self.class_data_mut("Object");
            Self::merge_summarized_call_sites(
                object_data,
                propagated_to_object
                    .into_values()
                    .map(CallSiteSummaryAccumulator::finish),
            );
        }
    }

    pub fn get_annotated_param_type(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
        param_index: usize,
    ) -> Option<Type> {
        self.class_data
            .get(class_name)
            .and_then(|data| {
                data.cold()
                    .annotated_params
                    .get(&(Sym::new(method_name), is_singleton))
                    .and_then(|params| params.get(&param_index))
            })
            .cloned()
    }

    pub fn set_annotated_param_type(
        &mut self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
        param_index: usize,
        ty: Type,
    ) {
        let method_name = Sym::new(method_name);
        self.class_data_mut(class_name)
            .cold_mut()
            .annotated_params
            .entry((method_name, is_singleton))
            .or_default()
            .insert(param_index, ty);
    }

    pub fn add_ivar_type(&mut self, class_name: &str, ivar_name: &str, ty: Type) {
        self.mark_class_has_method_body_facts(class_name);
        let data = self.class_data_mut(class_name);
        let types = data.ivars.entry(Sym::new(ivar_name)).or_default();
        Type::merge_into_vec(types, ty);
    }

    pub fn replace_ivar_type(&mut self, class_name: &str, ivar_name: &str, ty: Type) {
        self.mark_class_has_method_body_facts(class_name);
        let data = self.class_data_mut(class_name);
        data.ivars.insert(Sym::new(ivar_name), vec![ty]);
    }

    pub fn snapshot_ivar_types(&self, class_name: &str) -> Vec<(String, Type)> {
        let Some(data) = self.class_data.get(class_name) else {
            return Vec::new();
        };
        data.ivars
            .iter()
            .filter_map(|(name, types)| {
                if types.is_empty() {
                    None
                } else {
                    Some((name.to_string(), Type::from_type_vec(types.clone())))
                }
            })
            .collect()
    }

    pub fn replace_singleton_ivar_type(&mut self, class_name: &str, ivar_name: &str, ty: Type) {
        self.class_data_mut(class_name)
            .cold_mut()
            .singleton_ivars
            .insert(Sym::new(ivar_name), vec![ty]);
    }

    pub fn add_singleton_ivar_type(&mut self, class_name: &str, ivar_name: &str, ty: Type) {
        let types = self
            .class_data_mut(class_name)
            .cold_mut()
            .singleton_ivars
            .entry(Sym::new(ivar_name))
            .or_default();
        Type::merge_into_vec(types, ty);
    }

    pub fn lookup_singleton_ivar_type(&self, class_name: &str, ivar_name: &str) -> Option<Type> {
        let data = self.class_data.get(class_name)?;
        let types = data.cold().singleton_ivars.get(ivar_name)?;
        if types.is_empty() {
            return None;
        }
        Some(Type::from_type_vec(types.clone()))
    }

    pub fn replace_class_variable_type(&mut self, class_name: &str, var_name: &str, ty: Type) {
        self.class_data_mut(class_name)
            .cold_mut()
            .class_variables
            .insert(Sym::new(var_name), vec![ty]);
    }

    pub fn add_class_variable_type(&mut self, class_name: &str, var_name: &str, ty: Type) {
        let types = self
            .class_data_mut(class_name)
            .cold_mut()
            .class_variables
            .entry(Sym::new(var_name))
            .or_default();
        Type::merge_into_vec(types, ty);
    }

    pub fn lookup_class_variable_type(&self, class_name: &str, var_name: &str) -> Option<Type> {
        let mut current = Some(class_name);
        while let Some(name) = current {
            let data = self.class_data.get(name)?;
            if let Some(types) = data.cold().class_variables.get(var_name)
                && !types.is_empty()
            {
                return Some(Type::from_type_vec(types.clone()));
            }
            current = data.superclass.as_deref();
        }
        None
    }

    pub fn lookup_method_return_type(&self, class_name: &str, method_name: &str) -> Option<Type> {
        self.lookup_method_return_type_with_hint(class_name, method_name, false)
    }

    /// owners valid for singleton dispatch: only universal instance methods that a class object inherits through the Class/Module/Object chain.
    fn is_class_object_instance_owner(owner: &str) -> bool {
        matches!(
            owner,
            "Class" | "Module" | "Object" | "Kernel" | "BasicObject"
        )
    }

    pub fn lookup_method_return_type_with_hint(
        &self,
        class_name: &str,
        method_name: &str,
        prefer_singleton: bool,
    ) -> Option<Type> {
        let ty = if let Some(ty) =
            self.lookup_method_return_type_resolved(class_name, method_name, prefer_singleton)
        {
            ty
        } else {
            self.synthesize_dirty_method_through_ancestors(class_name, method_name)
                .map(|(_, method)| method.raw_return_type)?
        };
        Some(self.refine_untyped_synthetic_accessor(class_name, method_name, prefer_singleton, ty))
    }

    fn refine_untyped_synthetic_accessor(
        &self,
        class_name: &str,
        method_name: &str,
        prefer_singleton: bool,
        ty: Type,
    ) -> Type {
        if prefer_singleton || !matches!(ty, Type::Untyped) {
            return ty;
        }
        if !self.resolved_method_is_synthetic_dsl(class_name, method_name) {
            return ty;
        }
        self.schema_column_accessor_type(class_name, method_name)
            .unwrap_or(ty)
    }

    fn resolved_method_is_synthetic_dsl(&self, class_name: &str, method_name: &str) -> bool {
        self.resolve_first_method_call_owner_ref(class_name, method_name, false)
            .and_then(|(owner, is_singleton)| {
                self.class_data.get(owner).and_then(|data| {
                    Self::method_for_lookup_kind(data, method_name, Some(is_singleton))
                })
            })
            .is_some_and(|method| method.synthetic_dsl_source)
    }

    fn lookup_method_return_type_resolved(
        &self,
        class_name: &str,
        method_name: &str,
        prefer_singleton: bool,
    ) -> Option<Type> {
        if self
            .owner_lookup_cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return self.lookup_method_return_type_with_hint_cached(
                class_name,
                method_name,
                prefer_singleton,
            );
        }
        let order = if prefer_singleton {
            [true, false]
        } else {
            [false, true]
        };
        for singleton in order {
            if let Some((owner, is_singleton)) =
                self.resolve_first_method_call_owner_ref(class_name, method_name, singleton)
                && let Some(ty) = self.lookup_method_in_class_kind(owner, method_name, is_singleton)
            {
                // block non-universal instance methods picked up by the instance fallback search, to prevent incorrect singleton application.
                if prefer_singleton && !singleton && !Self::is_class_object_instance_owner(owner) {
                    continue;
                }
                return Some(ty);
            }
        }
        if let Some(ty) = self.lookup_method_return_type_in_subclasses(class_name, method_name) {
            return Some(ty);
        }
        self.lookup_method_return_type_via_including_classes(class_name, method_name)
    }

    /// cached version of return lookup: memoizes the costly owner search, but re-reads the return type on every hit (ownership is immutable after structure freeze).
    fn lookup_method_return_type_with_hint_cached(
        &self,
        class_name: &str,
        method_name: &str,
        prefer_singleton: bool,
    ) -> Option<Type> {
        let key = (
            self.shared_name(class_name),
            self.shared_name(method_name),
            prefer_singleton,
        );
        if let Some(entry) = self.owner_lookup_cache.get(&key) {
            return self.fetch_owner_list_type(method_name, &entry);
        }
        let entry = self.compute_owner_list(class_name, method_name, prefer_singleton);
        let result = self.fetch_owner_list_type(method_name, &entry);
        self.owner_lookup_cache.insert(key, entry);
        result
    }

    fn compute_owner_list(
        &self,
        class_name: &str,
        method_name: &str,
        prefer_singleton: bool,
    ) -> OwnerListEntry {
        let order = if prefer_singleton {
            [true, false]
        } else {
            [false, true]
        };
        for singleton in order {
            if let Some((owner, is_singleton)) =
                self.resolve_first_method_call_owner_ref(class_name, method_name, singleton)
                && self
                    .lookup_method_in_class_kind(owner, method_name, is_singleton)
                    .is_some()
            {
                // only block non-universal instance methods that came from the instance fallback.
                if prefer_singleton && !singleton && !Self::is_class_object_instance_owner(owner) {
                    continue;
                }
                let owners: Arc<[(Sym, bool)]> =
                    Arc::from(vec![(self.class_key(owner), is_singleton)]);
                return Some((OwnerListKind::Direct, owners));
            }
        }
        if self.tail().subclass_index.is_some() {
            let owners = self.resolve_method_in_subclasses_refs(class_name, method_name, false);
            let resolved: Vec<(Sym, bool)> = owners
                .iter()
                .filter(|&&(owner, is_singleton)| {
                    self.lookup_method_in_class_kind(owner, method_name, is_singleton)
                        .is_some()
                })
                .map(|&(owner, is_singleton)| (self.class_key(owner), is_singleton))
                .collect();
            if !resolved.is_empty() {
                return Some((OwnerListKind::Union, resolved.into()));
            }
        }
        // including-classes fallback (resolves module methods through the classes that include them).
        if self
            .class_data
            .get(class_name)
            .is_some_and(|d| !d.is_module)
        {
            return None;
        }
        let includers = self.includers_of(class_name);
        if includers.is_empty() {
            return None;
        }
        let mut resolved: Vec<(Sym, bool)> = Vec::new();
        for candidate_shared in includers {
            let candidate_name = candidate_shared.as_ref();
            if candidate_name == class_name {
                continue;
            }
            let mut seen = Vec::from([(class_name, false)]);
            if let Some((owner, is_singleton)) = self.resolve_method_call_owners_inner_refs(
                candidate_name,
                method_name,
                false,
                &mut seen,
            ) {
                {
                    if self
                        .lookup_method_in_class_kind(owner, method_name, is_singleton)
                        .is_some()
                    {
                        let owner_key = self.class_key(owner);
                        if !resolved
                            .iter()
                            .any(|&(o, k)| k == is_singleton && o == owner_key)
                        {
                            resolved.push((owner_key, is_singleton));
                        }
                    }
                }
            }
        }
        if resolved.is_empty() {
            None
        } else {
            Some((OwnerListKind::Union, resolved.into()))
        }
    }

    fn fetch_owner_list_type(&self, method_name: &str, entry: &OwnerListEntry) -> Option<Type> {
        let (kind, owners) = entry.as_ref()?;
        match kind {
            OwnerListKind::Direct => {
                let (owner, is_singleton) = owners[0];
                self.lookup_method_in_class_kind_sym(owner, method_name, is_singleton)
            }
            OwnerListKind::Union => {
                let mut types: Vec<Type> = Vec::new();
                for &(owner, is_singleton) in owners.iter() {
                    if let Some(ty) =
                        self.lookup_method_in_class_kind_sym(owner, method_name, is_singleton)
                        && !types.contains(&ty)
                    {
                        types.push(ty);
                    }
                }
                if types.is_empty() {
                    None
                } else {
                    Some(Type::from_type_vec(types))
                }
            }
        }
    }

    /// owned version of `resolve_scoped_class_ref_borrow` (for `&mut self` contexts).
    pub fn resolve_scoped_class_ref(&self, scope_class: &str, raw_name: &str) -> String {
        self.resolve_scoped_class_ref_borrow(scope_class, raw_name)
            .to_string()
    }

    /// resolve a shortened mixin/superclass name to an FQN `class_data` key via the enclosing scope chain.
    fn resolve_scoped_class_ref_borrow<'a>(
        &'a self,
        scope_class: &str,
        raw_name: &'a str,
    ) -> &'a str {
        // Ruby constant order (enclosing->top), plus skip on self-match (so `class B::A < A` doesn't land on self).
        let mut scope: &str = scope_class;
        // One buffer for the whole walk: the first scope is the longest, so no
        // later candidate reallocates. This walk runs on every mixin and
        // superclass edge of every ancestor lookup.
        let mut candidate = String::with_capacity(scope_class.len() + 2 + raw_name.len());
        loop {
            if !scope.is_empty() {
                candidate.clear();
                candidate.push_str(scope);
                candidate.push_str("::");
                candidate.push_str(raw_name);
                if candidate != scope_class
                    && let Some((k, _)) = self.class_data.get_key_value(candidate.as_str())
                {
                    return k.as_str();
                }
            }
            if scope.is_empty() {
                break;
            }
            match scope.rfind_scope_sep() {
                Some(idx) => scope = &scope[..idx],
                None => scope = "",
            }
        }
        if raw_name != scope_class
            && let Some((k, _)) = self.class_data.get_key_value(raw_name)
        {
            return k.as_str();
        }
        // Absolute reference (`include ::Foo::Bar`): the leading `::` is not part
        // of the stored key, so retry against the canonical (unprefixed) name.
        if let Some(stripped) = raw_name.strip_prefix("::")
            && stripped != scope_class
            && let Some((k, _)) = self.class_data.get_key_value(stripped)
        {
            return k.as_str();
        }
        raw_name
    }

    /// self-call fallback within a module: goes through the including class (resolves methods via sibling mixins).
    pub fn lookup_method_return_type_via_including_classes(
        &self,
        module_name: &str,
        method_name: &str,
    ) -> Option<Type> {
        // for a local class (not a module), ancestors are already resolved so skip the fallback.
        if self
            .class_data
            .get(module_name)
            .is_some_and(|d| !d.is_module)
        {
            return None;
        }
        let includers = self.includers_of(module_name);
        if includers.is_empty() {
            return None;
        }
        let mut types: Vec<Type> = Vec::new();
        for candidate_shared in includers {
            let candidate_name = candidate_shared.as_ref();
            if candidate_name == module_name {
                continue;
            }
            // Resolve in the including class's own chain, skipping the
            // module we're currently in so we don't recurse back.
            let mut seen = Vec::from([(module_name, false)]);
            if let Some((owner, is_singleton)) = self.resolve_method_call_owners_inner_refs(
                candidate_name,
                method_name,
                false,
                &mut seen,
            ) && let Some(ty) =
                self.lookup_method_in_class_kind(owner, method_name, is_singleton)
                && !types.contains(&ty)
            {
                types.push(ty);
            }
        }
        if types.is_empty() {
            None
        } else {
            Some(Type::from_type_vec(types))
        }
    }

    pub fn lookup_method_return_type_in_subclasses(
        &self,
        class_name: &str,
        method_name: &str,
    ) -> Option<Type> {
        let _ = self.tail().subclass_index.as_ref()?;
        let owners = self.resolve_method_in_subclasses_refs(class_name, method_name, false);
        if owners.is_empty() {
            return None;
        }
        let mut types: Vec<Type> = Vec::new();
        for &(owner, is_singleton) in &owners {
            if let Some(ty) = self.lookup_method_in_class_kind(owner, method_name, is_singleton)
                && !types.contains(&ty)
            {
                types.push(ty);
            }
        }
        if types.is_empty() {
            None
        } else {
            Some(Type::from_type_vec(types))
        }
    }

    pub fn lookup_method_sig(&self, class_name: &str, method_name: &str) -> Option<MethodSig> {
        self.lookup_method_sig_with_hint(class_name, method_name, false)
    }

    pub fn lookup_method_sig_with_hint(
        &self,
        class_name: &str,
        method_name: &str,
        prefer_singleton: bool,
    ) -> Option<MethodSig> {
        let order = if prefer_singleton {
            [true, false]
        } else {
            [false, true]
        };
        for singleton in order {
            if let Some((owner, is_singleton)) =
                self.resolve_first_method_call_owner_ref(class_name, method_name, singleton)
                && let Some(sig) =
                    self.lookup_method_sig_in_class_kind(owner, method_name, is_singleton)
            {
                return Some(sig);
            }
        }
        self.lookup_method_sig_in_subclasses(class_name, method_name)
    }

    pub fn lookup_method_definition_location_with_hint(
        &self,
        class_name: &str,
        method_name: &str,
        prefer_singleton: bool,
    ) -> Option<(String, SourceLocation)> {
        let order = if prefer_singleton {
            [true, false]
        } else {
            [false, true]
        };
        for singleton in order {
            if let Some((owner, is_singleton)) =
                self.resolve_first_method_call_owner_ref(class_name, method_name, singleton)
                && let Some(location) =
                    self.method_definition_location_in_class_kind(owner, method_name, is_singleton)
            {
                return Some(location);
            }
        }
        None
    }

    pub fn lookup_method_definition_location_for_dispatch(
        &self,
        class_name: &str,
        method_name: &str,
        method_is_singleton: bool,
    ) -> Option<(String, SourceLocation)> {
        let (owner, is_singleton) =
            self.resolve_first_method_call_owner_ref(class_name, method_name, method_is_singleton)?;
        self.method_definition_location_in_class_kind(owner, method_name, is_singleton)
    }

    pub fn lookup_method_definition_location_exact(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<(String, SourceLocation)> {
        self.method_definition_location_in_class_kind(class_name, method_name, is_singleton)
    }

    fn method_definition_location_in_class_kind(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<(String, SourceLocation)> {
        let data = self.class_data.get(class_name)?;
        let method = Self::method_for_lookup_kind(data, method_name, Some(is_singleton))?;
        let file_path = data
            .method_file_paths
            .get(&(method.name, method.is_singleton))
            .map(AsRef::as_ref)
            .or(data.file_path.as_deref())?;
        let loc = method.loc?;
        Some((file_path.to_string(), loc))
    }

    fn lookup_method_sig_in_subclasses(
        &self,
        class_name: &str,
        method_name: &str,
    ) -> Option<MethodSig> {
        let _ = self.tail().subclass_index.as_ref()?;
        let owners = self.resolve_method_in_subclasses_refs(class_name, method_name, false);
        for &(owner, is_singleton) in &owners {
            if let Some(sig) =
                self.lookup_method_sig_in_class_kind(owner, method_name, is_singleton)
            {
                return Some(sig);
            }
        }
        None
    }

    pub fn lookup_method_sig_for_receiver(
        &self,
        receiver_class: &str,
        method_name: &str,
    ) -> Option<MethodSig> {
        self.lookup_method_sig_for_receiver_with_hint(receiver_class, method_name, false)
    }

    pub fn lookup_method_sig_for_receiver_with_hint(
        &self,
        receiver_class: &str,
        method_name: &str,
        prefer_singleton: bool,
    ) -> Option<MethodSig> {
        let order = if prefer_singleton {
            [true, false]
        } else {
            [false, true]
        };
        for singleton in order {
            if let Some((owner, is_singleton)) =
                self.resolve_first_method_call_owner_ref(receiver_class, method_name, singleton)
                && let Some(data) = self.class_data.get(owner)
                && let Some(method) =
                    Self::method_for_lookup_kind(data, method_name, Some(is_singleton))
            {
                return Some(self.build_method_sig_for_receiver(receiver_class, owner, method));
            }
        }
        if self.tail().subclass_index.is_some() {
            let owners = self.resolve_method_in_subclasses_refs(receiver_class, method_name, false);
            for &(owner, is_singleton) in &owners {
                if let Some(data) = self.class_data.get(owner)
                    && let Some(method) =
                        Self::method_for_lookup_kind(data, method_name, Some(is_singleton))
                {
                    return Some(self.build_method_sig_for_receiver(receiver_class, owner, method));
                }
            }
        }
        // dirty-tracking family methods are synthesized here from the skeleton pattern (this is the final
        // stage, after no real method was found). Instance side only, regardless of `prefer_singleton`.
        if let Some((owner, method)) =
            self.synthesize_dirty_method_through_ancestors(receiver_class, method_name)
        {
            return Some(self.build_method_sig_for_receiver(receiver_class, &owner, &method));
        }
        None
    }

    /// dirty-family synthesis: search the superclass chain for the pattern owner (STI inheritance). Call this after real resolution.
    fn synthesize_dirty_method_through_ancestors(
        &self,
        receiver_class: &str,
        method_name: &str,
    ) -> Option<(String, MethodDef)> {
        // return immediately if no class has a pattern (non-Rails / no schema).
        // limits the per-miss name-matching cost to registries that actually have patterns.
        if !self.has_dirty_patterns {
            return None;
        }
        // cheap pre-filter: return immediately for names that don't match a dirty suffix/prefix.
        // most of the hot path (ordinary method calls) exits here.
        split_dirty_method_name(method_name)?;
        let mut current = Some(receiver_class.to_string());
        let mut depth = 0;
        while let Some(cls) = current {
            if depth >= MAX_RESOLVE_DEPTH {
                break;
            }
            depth += 1;
            let data = self.class_data.get(cls.as_str())?;
            if let Some(pattern) = &data.cold().dirty_method_pattern
                && let Some(method) = pattern.synthesize(method_name)
            {
                return Some((cls, method));
            }
            current = data.superclass.as_ref().map(|sc| {
                self.resolve_scoped_class_ref_borrow(&cls, sc.as_ref())
                    .to_string()
            });
        }
        None
    }

    pub fn method_completion_candidates_for_type(
        &self,
        receiver_type: &Type,
    ) -> Vec<MethodCompletionCandidate> {
        match receiver_type {
            Type::Union(parts) => self.method_completion_candidates_for_union(parts),
            Type::Intersection(parts) => self.method_completion_candidates_for_intersection(parts),
            _ => self
                .method_completion_candidates_for_non_union(receiver_type)
                .unwrap_or_default(),
        }
    }

    fn method_completion_candidates_for_union(
        &self,
        parts: &[Type],
    ) -> Vec<MethodCompletionCandidate> {
        let mut per_part = Vec::new();
        for part in parts {
            let candidates = self.method_completion_candidates_for_non_union(part);
            let Some(candidates) = candidates else {
                return Vec::new();
            };
            per_part.push(candidates);
        }
        let Some((first, rest)) = per_part.split_first() else {
            return Vec::new();
        };
        let mut common: HashSet<String> = first
            .iter()
            .map(|candidate| candidate.name.clone())
            .collect();
        for candidates in rest {
            let names: HashSet<String> = candidates
                .iter()
                .map(|candidate| candidate.name.clone())
                .collect();
            common.retain(|name| names.contains(name));
        }
        let mut names: Vec<String> = common.into_iter().collect();
        names.sort();
        let mut candidates = Vec::new();
        for name in names {
            let variants: Vec<MethodCompletionCandidate> = per_part
                .iter()
                .filter_map(|candidates| {
                    candidates
                        .iter()
                        .find(|candidate| candidate.name == name)
                        .cloned()
                })
                .collect();
            if variants.len() == per_part.len() {
                candidates.push(Self::merge_union_method_completion_candidate(variants));
            }
        }
        candidates.sort_by(|a, b| a.name.cmp(&b.name));
        candidates
    }

    fn merge_union_method_completion_candidate(
        variants: Vec<MethodCompletionCandidate>,
    ) -> MethodCompletionCandidate {
        let mut merged = variants
            .first()
            .cloned()
            .expect("union completion variants must not be empty");
        let mut owner_classes: Vec<String> = variants
            .iter()
            .map(|candidate| candidate.owner_class.clone())
            .collect();
        owner_classes.sort();
        owner_classes.dedup();
        if owner_classes.len() > 1 {
            merged.owner_class = owner_classes.join(" | ");
        }

        merged.sig.return_type = Type::from_type_vec(
            variants
                .iter()
                .map(|candidate| candidate.sig.return_type.clone())
                .collect(),
        );
        if variants
            .iter()
            .all(|candidate| Self::same_completion_param_shape(&merged.sig, &candidate.sig))
        {
            for idx in 0..merged.sig.params.len() {
                merged.sig.params[idx].param_type = Type::from_type_vec(
                    variants
                        .iter()
                        .map(|candidate| candidate.sig.params[idx].param_type.clone())
                        .collect(),
                );
            }
        }
        merged
    }

    fn same_completion_param_shape(left: &MethodSig, right: &MethodSig) -> bool {
        left.params.len() == right.params.len()
            && left
                .params
                .iter()
                .zip(&right.params)
                .all(|(left, right)| left.kind == right.kind)
    }

    fn method_completion_candidates_for_intersection(
        &self,
        parts: &[Type],
    ) -> Vec<MethodCompletionCandidate> {
        let mut candidates = Vec::new();
        let mut seen_methods = HashSet::new();
        for part in parts {
            let Some(part_candidates) = self.method_completion_candidates_for_non_union(part)
            else {
                continue;
            };
            for candidate in part_candidates {
                if seen_methods.insert(candidate.name.clone()) {
                    candidates.push(candidate);
                }
            }
        }
        candidates.sort_by(|a, b| a.name.cmp(&b.name));
        candidates
    }

    fn method_completion_candidates_for_non_union(
        &self,
        receiver_type: &Type,
    ) -> Option<Vec<MethodCompletionCandidate>> {
        let receiver_class = Self::type_to_class_name(receiver_type)?;
        let method_is_singleton = matches!(receiver_type, Type::Singleton(_));
        let mut candidates = Vec::new();
        let mut seen_owners = Vec::new();
        let mut seen_methods = HashSet::new();
        self.collect_method_completion_candidates(
            &receiver_class,
            &receiver_class,
            method_is_singleton,
            &mut seen_owners,
            &mut seen_methods,
            &mut candidates,
        );
        candidates.sort_by(|a, b| a.name.cmp(&b.name));
        Some(candidates)
    }

    fn collect_method_completion_candidates<'a>(
        &'a self,
        receiver_class: &'a str,
        owner_class: &'a str,
        method_is_singleton: bool,
        seen_owners: &mut Vec<(&'a str, bool)>,
        seen_methods: &mut HashSet<String>,
        candidates: &mut Vec<MethodCompletionCandidate>,
    ) {
        if seen_owners.contains(&(owner_class, method_is_singleton)) {
            return;
        }
        seen_owners.push((owner_class, method_is_singleton));

        let Some(data) = self.class_data.get(owner_class) else {
            return;
        };

        if !method_is_singleton {
            for mixin in data.mixins.iter().rev() {
                if mixin.kind != MixinKind::Prepend {
                    continue;
                }
                let mixin_ref =
                    self.resolve_scoped_class_ref_borrow(owner_class, mixin.module_name.as_ref());
                self.collect_method_completion_candidates(
                    receiver_class,
                    mixin_ref,
                    false,
                    seen_owners,
                    seen_methods,
                    candidates,
                );
            }
        }

        for (method_name, is_singleton) in &data.cold().undefined_methods {
            if *is_singleton == method_is_singleton {
                seen_methods.insert(method_name.to_string());
            }
        }

        self.push_method_completion_candidates_from_owner(
            receiver_class,
            owner_class,
            data,
            method_is_singleton,
            seen_methods,
            candidates,
        );

        if method_is_singleton {
            for mixin in data.mixins.iter().rev() {
                let mixin_ref =
                    self.resolve_scoped_class_ref_borrow(owner_class, mixin.module_name.as_ref());
                if mixin.kind == MixinKind::Extend {
                    self.collect_method_completion_candidates(
                        receiver_class,
                        mixin_ref,
                        false,
                        seen_owners,
                        seen_methods,
                        candidates,
                    );
                } else {
                    // a Concern's `class_methods do` block lands on the module's singleton.
                    if self.is_concern_module(mixin_ref) {
                        self.collect_method_completion_candidates(
                            receiver_class,
                            mixin_ref,
                            true,
                            seen_owners,
                            seen_methods,
                            candidates,
                        );
                    }
                    // treat `M::ClassMethods` instance methods as includer class-method candidates (independent of Concern).
                    if let Some(class_methods) = self.concern_class_methods_owner(mixin_ref) {
                        self.collect_method_completion_candidates(
                            receiver_class,
                            class_methods,
                            false,
                            seen_owners,
                            seen_methods,
                            candidates,
                        );
                    }
                }
            }
            if let Some(superclass) = &data.superclass {
                let super_ref =
                    self.resolve_scoped_class_ref_borrow(owner_class, superclass.as_ref());
                self.collect_method_completion_candidates(
                    receiver_class,
                    super_ref,
                    true,
                    seen_owners,
                    seen_methods,
                    candidates,
                );
            }
            for fallback in ["Class", "Module", "Object"] {
                if owner_class == fallback {
                    continue;
                }
                self.collect_method_completion_candidates(
                    receiver_class,
                    fallback,
                    false,
                    seen_owners,
                    seen_methods,
                    candidates,
                );
            }
        } else {
            for mixin in data.mixins.iter().rev() {
                if mixin.kind != MixinKind::Include {
                    continue;
                }
                let mixin_ref =
                    self.resolve_scoped_class_ref_borrow(owner_class, mixin.module_name.as_ref());
                self.collect_method_completion_candidates(
                    receiver_class,
                    mixin_ref,
                    false,
                    seen_owners,
                    seen_methods,
                    candidates,
                );
            }

            if let Some(superclass) = &data.superclass {
                let super_ref =
                    self.resolve_scoped_class_ref_borrow(owner_class, superclass.as_ref());
                self.collect_method_completion_candidates(
                    receiver_class,
                    super_ref,
                    false,
                    seen_owners,
                    seen_methods,
                    candidates,
                );
            }

            for ancestor in &data.cold().required_ancestors {
                self.collect_method_completion_candidates(
                    receiver_class,
                    ancestor.as_ref(),
                    false,
                    seen_owners,
                    seen_methods,
                    candidates,
                );
            }

            if owner_class != "Object" {
                self.collect_method_completion_candidates(
                    receiver_class,
                    "Object",
                    false,
                    seen_owners,
                    seen_methods,
                    candidates,
                );
            }

            for fallback in ["Kernel", "Comparable", "BasicObject"] {
                self.collect_method_completion_candidates(
                    receiver_class,
                    fallback,
                    false,
                    seen_owners,
                    seen_methods,
                    candidates,
                );
            }
        }
    }

    fn push_method_completion_candidates_from_owner(
        &self,
        receiver_class: &str,
        owner_class: &str,
        data: &ClassData,
        method_is_singleton: bool,
        seen_methods: &mut HashSet<String>,
        candidates: &mut Vec<MethodCompletionCandidate>,
    ) {
        let mut method_names: Vec<&str> = data
            .method_index
            .iter()
            .filter_map(|(name, slots)| slots.has(method_is_singleton).then_some(name.as_str()))
            .collect();
        method_names.sort_unstable();

        for method_name in method_names {
            if !seen_methods.insert(method_name.to_string()) {
                continue;
            }
            let Some(method) =
                Self::method_for_lookup_kind(data, method_name, Some(method_is_singleton))
            else {
                continue;
            };
            candidates.push(MethodCompletionCandidate {
                name: method.name.to_string(),
                owner_class: owner_class.to_string(),
                is_singleton: method.is_singleton,
                sig: self.build_method_sig_for_receiver(receiver_class, owner_class, method),
            });
        }
    }

    pub fn lookup_method_sig_exact(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<MethodSig> {
        self.lookup_method_sig_in_class_kind(class_name, method_name, is_singleton)
    }

    pub fn has_method_named(&self, class_name: &str, method_name: &str) -> bool {
        self.class_data
            .get(class_name)
            .is_some_and(|data| data.method_index.contains_key(method_name))
    }

    pub fn has_method_variant(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> bool {
        self.class_data
            .get(class_name)
            .and_then(|data| data.method_index.get(method_name))
            .is_some_and(|slots| slots.has(is_singleton))
    }

    pub fn has_any_method_variant(&self, class_name: &str, method_name: &str) -> bool {
        self.class_data
            .get(class_name)
            .and_then(|data| data.method_index.get(method_name))
            .is_some_and(|slots| slots.instance.is_some() || slots.singleton.is_some())
    }

    /// interned `class_data` key for `class_name`, taken from the map itself so a
    /// name already registered as a class costs no interner lock.
    #[inline]
    fn class_key(&self, class_name: &str) -> Sym {
        self.class_data
            .get_key_value(class_name)
            .map(|(key, _)| *key)
            .unwrap_or_else(|| Sym::new(class_name))
    }

    fn lookup_method_in_class_kind(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<Type> {
        let data = self.class_data.get(class_name)?;
        let result = Self::method_for_lookup_kind(data, method_name, Some(is_singleton))
            .map(|m| m.raw_return_type.clone());
        if result.is_some() {
            note_return_type_read(class_name, method_name);
        }
        result
    }

    /// `Sym`-keyed twin of `lookup_method_in_class_kind`: probes `class_data` by
    /// pointer instead of by string content.
    fn lookup_method_in_class_kind_sym(
        &self,
        class_name: Sym,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<Type> {
        let data = self.class_data.get(&class_name)?;
        let result = Self::method_for_lookup_kind(data, method_name, Some(is_singleton))
            .map(|m| m.raw_return_type.clone());
        if result.is_some() {
            note_return_type_read(class_name, method_name);
        }
        result
    }

    pub fn lookup_method_return_type_direct(
        &self,
        class_name: &str,
        method_name: &str,
    ) -> Option<Type> {
        let data = self.class_data.get(class_name)?;
        let result = Self::method_for_lookup_kind(data, method_name, None)
            .map(|m| m.raw_return_type.clone());
        if result.is_some() {
            note_return_type_read(class_name, method_name);
        }
        result
    }

    /// `Sym`-keyed twin of `lookup_method_return_type_direct`.
    fn lookup_method_return_type_direct_sym(
        &self,
        class_name: Sym,
        method_name: Sym,
    ) -> Option<Type> {
        let data = self.class_data.get(&class_name)?;
        let result = Self::method_for_lookup_kind(data, method_name.as_str(), None)
            .map(|m| m.raw_return_type.clone());
        if result.is_some() {
            note_return_type_read(class_name, method_name);
        }
        result
    }

    pub fn resolve_instance_method_call_owners(
        &self,
        class_name: &str,
        method_name: &str,
    ) -> Vec<String> {
        self.resolve_method_call_owner_ref(class_name, method_name, false)
            .into_iter()
            .map(|(owner, _)| owner.to_string())
            .collect()
    }

    pub fn resolve_method_call_owners(
        &self,
        class_name: &str,
        method_name: &str,
        method_is_singleton: bool,
    ) -> Vec<(String, bool)> {
        self.resolve_method_call_owner_ref(class_name, method_name, method_is_singleton)
            .into_iter()
            .map(|(owner, is_singleton)| (owner.to_string(), is_singleton))
            .collect()
    }

    fn resolve_method_call_owner_ref<'a>(
        &'a self,
        class_name: &'a str,
        method_name: &str,
        method_is_singleton: bool,
    ) -> Option<(&'a str, bool)> {
        // Sized for a typical ancestor chain so the walk doesn't regrow.
        let mut seen = Vec::with_capacity(16);
        self.resolve_method_call_owners_inner_refs(
            class_name,
            method_name,
            method_is_singleton,
            &mut seen,
        )
    }

    /// first-owner lookup memo (walks the ancestor chain directly when the cache is disabled).
    fn first_method_call_owner_cached(
        &self,
        class_name: &str,
        method_name: &str,
        method_is_singleton: bool,
    ) -> Option<(SharedName, bool)> {
        if !self
            .owner_lookup_cache_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return self
                .resolve_first_method_call_owner_ref(class_name, method_name, method_is_singleton)
                .map(|(owner, singleton)| (self.shared_name(owner), singleton));
        }
        let key = (
            self.shared_name(class_name),
            self.shared_name(method_name),
            method_is_singleton,
        );
        let shard = self.first_owner_cache.shard(&key);
        if let Ok(guard) = shard.read()
            && let Some(entry) = guard.get(&key)
        {
            return entry.clone();
        }
        let computed = self
            .resolve_first_method_call_owner_ref(class_name, method_name, method_is_singleton)
            .map(|(owner, singleton)| (self.shared_name(owner), singleton));
        if let Ok(mut guard) = shard.write() {
            guard.insert(key, computed.clone());
        }
        computed
    }

    fn resolve_first_method_call_owner_ref<'a>(
        &'a self,
        class_name: &'a str,
        method_name: &str,
        method_is_singleton: bool,
    ) -> Option<(&'a str, bool)> {
        self.resolve_method_call_owner_ref(class_name, method_name, method_is_singleton)
    }

    pub fn build_subclass_index(&mut self) {
        self.apply_mixin_hook_mixins();
        let mut subclass_index: FxHashMap<SharedName, Vec<SharedName>> = FxHashMap::default();
        let mut includer_index: FxHashMap<SharedName, Vec<SharedName>> = FxHashMap::default();
        // pre-resolve shortened mixin names to FQN (turns includer lookup into an O(1) hash hit).
        for (class_name, data) in &self.class_data {
            if let Some(ref superclass) = data.superclass {
                subclass_index
                    .entry(superclass.clone())
                    .or_default()
                    .push(self.shared_name(class_name));
            }
            for mixin in &data.mixins {
                // resolve a shortened mixin name to an FQN in the including scope and cache it.
                let resolved =
                    self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
                let key = self.shared_name(resolved);
                includer_index
                    .entry(key)
                    .or_default()
                    .push(self.shared_name(class_name));
            }
        }
        let tail = self.tail_mut();
        tail.subclass_index = Some(subclass_index);
        tail.module_includer_index = Some(includer_index);
    }

    fn subclasses_of(&self, class_name: &str) -> &[SharedName] {
        self.tail()
            .subclass_index
            .as_ref()
            .and_then(|idx| idx.get(class_name))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(crate) fn includers_of(&self, module_name: &str) -> &[SharedName] {
        self.tail()
            .module_includer_index
            .as_ref()
            .and_then(|idx| idx.get(module_name))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn resolve_method_in_subclasses_refs<'a>(
        &'a self,
        class_name: &'a str,
        method_name: &str,
        method_is_singleton: bool,
    ) -> Vec<(&'a str, bool)> {
        let mut result = Vec::new();
        let mut queue: Vec<&str> = self
            .subclasses_of(class_name)
            .iter()
            .map(|name| name.as_ref())
            .collect();
        // membership only, so the set walks the same classes in the same order the
        // linear `Vec` scan did, without being quadratic on wide hierarchies.
        let mut visited: FxHashSet<&str> = FxHashSet::default();
        visited.insert(class_name);
        while let Some(sub) = queue.pop() {
            if !visited.insert(sub) {
                continue;
            }
            if let Some(data) = self.class_data.get(sub) {
                if Self::method_for_lookup_kind(data, method_name, Some(method_is_singleton))
                    .is_some()
                {
                    result.push((sub, method_is_singleton));
                } else {
                    for child in self.subclasses_of(sub) {
                        queue.push(child.as_ref());
                    }
                }
            }
        }
        result
    }

    pub fn lookup_ivar_type(&self, class_name: &str, ivar_name: &str) -> Option<Type> {
        let data = self.class_data.get(class_name)?;
        let types = data.ivars.get(ivar_name)?;
        if types.is_empty() {
            return None;
        }
        Some(Type::from_type_vec(types.clone()))
    }

    /// ivar lookup also walks the superclass chain (a child may reference an ivar set by the parent's `initialize`). Has a depth guard.
    pub fn lookup_ivar_type_through_ancestors(
        &self,
        class_name: &str,
        ivar_name: &str,
    ) -> Option<Type> {
        let mut current: Option<&str> = Some(class_name);
        let mut depth = 0;
        while let Some(cls) = current {
            if depth >= MAX_RESOLVE_DEPTH {
                break;
            }
            depth += 1;
            if let Some(ty) = self.lookup_ivar_type(cls, ivar_name) {
                return Some(ty);
            }
            current = self
                .class_data
                .get(cls)
                .and_then(|data| data.superclass.as_ref())
                .map(SharedName::as_ref);
        }
        None
    }

    /// untyped ivar: uses the call-site type of the same-named `initialize` param, falling back to a param-name->class heuristic.
    pub(crate) fn infer_attr_type_from_initialize(
        &self,
        class_name: &str,
        ivar_name: &str,
    ) -> Option<Type> {
        let _visit_guard = AttrInitVisitGuard::enter(class_name, ivar_name)?;
        let param_name = ivar_name.strip_prefix('@')?;
        let data = self.class_data.get(class_name)?;

        let init_method = data
            .methods
            .iter()
            .find(|m| m.name == "initialize" && !m.is_singleton);

        if let Some(init_method) = init_method {
            let param_index = init_method
                .param_infos
                .iter()
                .position(|pi| pi.name == param_name);

            let positional_count = Self::positional_param_count(&init_method.param_infos);

            if let Some(param_index) = param_index {
                if let Some(annotated_type) =
                    self.get_annotated_param_type(class_name, "initialize", false, param_index)
                {
                    let resolved = self.resolve_deferred_refs(class_name, &annotated_type);
                    if Self::is_concrete_for_global_resolve(&resolved) {
                        return Some(resolved);
                    }
                }

                let mut types: Vec<Type> = Vec::new();
                for call_site in &data.call_sites {
                    if call_site.method_name.as_ref() == "initialize"
                        && !call_site.method_is_singleton
                        && param_index < positional_count
                        && param_index < call_site.arg_types.len()
                    {
                        let mut visiting = FxHashSet::from_iter([(
                            class_name.to_string(),
                            "initialize".to_string(),
                            false,
                        )]);
                        let ty = self.resolve_call_site_type_from_caller_context(
                            call_site,
                            &call_site.arg_types[param_index],
                            &mut visiting,
                        );
                        if ty != Type::Untyped && !types.contains(&ty) {
                            types.push(ty);
                        }
                    }
                }

                if types.is_empty() {
                    for call_site in &data.call_sites {
                        if call_site.method_name.as_ref() == "initialize"
                            && !call_site.method_is_singleton
                            && let Some(ty) = call_site.keyword_arg_types.get(param_name)
                        {
                            let mut visiting = FxHashSet::from_iter([(
                                class_name.to_string(),
                                "initialize".to_string(),
                                false,
                            )]);
                            let ty = self.resolve_call_site_type_from_caller_context(
                                call_site,
                                ty,
                                &mut visiting,
                            );
                            if ty != Type::Untyped && !types.contains(&ty) {
                                types.push(ty);
                            }
                        }
                    }
                }

                if !types.is_empty() {
                    return Some(Type::from_type_vec(types));
                }

                if let Some(param_info) = init_method.param_infos.get(param_index)
                    && matches!(
                        param_info.kind,
                        ParamKind::Optional | ParamKind::KeywordOptional
                    )
                    && let Some(default_type) = param_info.default_type.as_ref()
                {
                    let resolved_default = self.resolve_deferred_refs(class_name, default_type);
                    if resolved_default != Type::Untyped {
                        return Some(resolved_default);
                    }
                }
            }
        }

        self.infer_attr_type_from_initialize_camel_for_class(class_name, ivar_name)
    }

    fn infer_attr_type_from_initialize_camel(&self, ivar_name: &str) -> Option<Type> {
        let param_name = ivar_name.strip_prefix('@').unwrap_or(ivar_name);
        let camel = to_camel_case(param_name);
        if self.class_data.contains_key(camel.as_str()) {
            return Some(Type::Class(Sym::new(camel)));
        }
        None
    }

    /// camel-case ivar heuristic: only adopt the class type when `initialize` has a same-named param (prevents misinferring `@key`->`Key`).
    fn infer_attr_type_from_initialize_camel_for_class(
        &self,
        class_name: &str,
        ivar_name: &str,
    ) -> Option<Type> {
        let param_name = ivar_name.strip_prefix('@').unwrap_or(ivar_name);
        let data = self.class_data.get(class_name)?;
        let init_method = data
            .methods
            .iter()
            .find(|m| m.name == "initialize" && !m.is_singleton)?;
        if !init_method
            .param_infos
            .iter()
            .any(|pi| pi.name == param_name)
        {
            return None;
        }
        self.infer_attr_type_from_initialize_camel(ivar_name)
    }

    fn resolve_initialize_param_passthrough_type(
        &self,
        class_name: &str,
        param_name: &str,
    ) -> Option<Type> {
        let data = self.class_data.get(class_name)?;
        let targets = data.cold().initialize_param_passthroughs.get(param_name)?;
        let mut types = Vec::new();
        for target in targets {
            if let Some(ty) = self.resolve_passthrough_target_type(class_name, target)
                && Self::is_concrete_for_global_resolve(&ty)
            {
                Type::merge_into_vec(&mut types, ty);
            }
        }
        (!types.is_empty()).then(|| Type::from_type_vec(types))
    }

    fn resolve_passthrough_target_type(&self, class_name: &str, target: &str) -> Option<Type> {
        if target.starts_with('@') {
            let direct = self.lookup_ivar_type(class_name, target);
            return match direct {
                Some(ref ty) if Self::is_concrete_for_global_resolve(ty) => Some(ty.clone()),
                _ => self.infer_attr_type_from_initialize(class_name, target),
            };
        }

        if let Some(sig) = self.lookup_method_sig(class_name, target)
            && Self::is_concrete_for_global_resolve(&sig.return_type)
        {
            return Some(sig.return_type);
        }

        let setter_name = format!("{target}=");
        if let Some(sig) = self.lookup_method_sig(class_name, &setter_name) {
            if let Some(param) = sig.params.first()
                && Self::is_concrete_for_global_resolve(&param.param_type)
            {
                return Some(param.param_type.clone());
            }
            if Self::is_concrete_for_global_resolve(&sig.return_type) {
                return Some(sig.return_type);
            }
        }

        self.infer_attr_type_from_initialize(class_name, &format!("@{target}"))
    }

    pub fn update_method_return_type_variant(
        &mut self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
        return_type: Type,
    ) {
        if let Some(data) = self.class_data.get_mut(class_name)
            && let Some(method_idx) = data
                .method_index
                .get(method_name)
                .and_then(|slots| slots.get(is_singleton))
            && let Some(method) = data.methods.get_mut(method_idx)
        {
            Arc::make_mut(method).raw_return_type = return_type;
        }
    }

    pub fn update_instance_method_return_type(
        &mut self,
        class_name: &str,
        method_name: &str,
        return_type: Type,
    ) {
        self.update_method_return_type_variant(class_name, method_name, false, return_type);
    }

    pub fn update_method_params(
        &mut self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
        param_infos: Vec<ParamInfo>,
    ) {
        if let Some(data) = self.class_data.get_mut(class_name)
            && let Some(method) = data
                .methods
                .iter_mut()
                .find(|m| m.name == method_name && m.is_singleton == is_singleton)
        {
            let method = Arc::make_mut(method);
            method.param_infos = param_infos;
        }
    }

    pub fn update_method_param_default_type(
        &mut self,
        class_name: &str,
        method_name: &str,
        param_index: usize,
        default_type: Type,
    ) {
        if let Some(data) = self.class_data.get_mut(class_name)
            && let Some(method_idx) = data
                .method_index
                .get(method_name)
                .and_then(|slots| slots.instance.or(slots.singleton))
            && let Some(method) = data.methods.get_mut(method_idx)
            && let Some(param) = Arc::make_mut(method).param_infos.get_mut(param_index)
        {
            param.default_type = Some(default_type);
        }
    }

    pub fn update_method_param_default_type_variant(
        &mut self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
        param_index: usize,
        default_type: Type,
    ) {
        if let Some(data) = self.class_data.get_mut(class_name)
            && let Some(method_idx) = data
                .method_index
                .get(method_name)
                .and_then(|slots| slots.get(is_singleton))
            && let Some(method) = data.methods.get_mut(method_idx)
            && let Some(param) = Arc::make_mut(method).param_infos.get_mut(param_index)
        {
            param.default_type = Some(default_type);
        }
    }

    /// runs once per method after merge: replaces param markers from its own call sites (cross-file type recovery, shared by CLI/LSP).
    pub(crate) fn resolve_method_param_refs_from_call_sites(&mut self) {
        let class_names = self.user_defined_class_names_unsorted();
        self.resolve_method_param_refs_from_call_sites_for_classes(&class_names);
    }

    fn resolve_method_param_refs_from_call_sites_for_classes(&mut self, class_names: &[Sym]) {
        for class_name in class_names {
            let Some(data) = self.class_data.get(class_name) else {
                continue;
            };
            let has_candidate = data.methods.iter().any(|method| {
                !method.synthetic_dsl_source
                    && Self::type_contains_param_ref_static(&method.raw_return_type)
            });
            if !has_candidate || data.call_sites.is_empty() {
                continue;
            }
            let mut grouped: FxHashMap<(&str, bool), Vec<&CallSite>> = FxHashMap::default();
            for site in &data.call_sites {
                grouped
                    .entry((site.method_name.as_ref(), site.method_is_singleton))
                    .or_default()
                    .push(site);
            }
            let updates: Vec<(Sym, bool, Type)> = data
                .methods
                .iter()
                .filter_map(|method| {
                    if method.synthetic_dsl_source
                        || !Self::type_contains_param_ref_static(&method.raw_return_type)
                    {
                        return None;
                    }
                    let sites = grouped.get(&(method.name.as_str(), method.is_singleton))?;
                    let positional_count = Self::positional_param_count(&method.param_infos);
                    let mut param_types: Vec<Vec<Type>> = vec![Vec::new(); positional_count];
                    for site in sites {
                        Self::merge_call_site_positional_types(
                            &mut param_types,
                            site,
                            &method.param_infos,
                        );
                    }
                    // if any argument position is untyped, treat the whole thing as Untyped (a partially-untyped union would get incorrectly narrowed to e.g. `nil` downstream).
                    let finalize = |ty: Type| -> Type {
                        if Self::type_contains_untyped_member(&ty) {
                            Type::Untyped
                        } else {
                            ty
                        }
                    };
                    let resolved_params: Vec<Type> = param_types
                        .into_iter()
                        .map(|types| {
                            if types.is_empty() {
                                Type::Untyped
                            } else {
                                finalize(Type::from_type_vec(types).widen_arg_for_param())
                            }
                        })
                        .collect();
                    let mut keyword_merged: FxHashMap<String, Vec<Type>> = FxHashMap::default();
                    for site in sites {
                        for (name, ty) in &site.keyword_arg_types {
                            keyword_merged
                                .entry(name.as_ref().to_string())
                                .or_default()
                                .push(ty.clone());
                        }
                    }
                    let keyword_types: HashMap<String, Type> = keyword_merged
                        .into_iter()
                        .map(|(name, types)| {
                            (
                                name,
                                finalize(Type::from_type_vec(types).widen_arg_for_param()),
                            )
                        })
                        .collect();
                    let resolved = Self::substitute_param_refs_static_with_keywords(
                        &method.raw_return_type,
                        &resolved_params,
                        &keyword_types,
                    );
                    (resolved != method.raw_return_type).then_some((
                        method.name,
                        method.is_singleton,
                        resolved,
                    ))
                })
                .collect();
            for (method_name, is_singleton, resolved) in updates {
                self.update_method_return_type_variant(
                    class_name,
                    &method_name,
                    is_singleton,
                    resolved,
                );
            }
        }
    }

    /// Resolve ParamRef in ivar types and method return types using CallSite
    /// data. Only resolves from `initialize` call sites for ivars.
    pub fn resolve_param_refs_global(&mut self) {
        let class_names = self.user_defined_class_names_unsorted();
        self.resolve_param_refs_for_classes(&class_names);
    }

    fn resolve_param_refs_for_classes(&mut self, class_names: &[Sym]) {
        for class_name in class_names {
            let Some(data) = self.class_data.get(class_name) else {
                continue;
            };
            let Some(init_method) = data
                .methods
                .iter()
                .find(|method| method.name == "initialize" && !method.is_singleton)
            else {
                continue;
            };

            let positional_count = Self::positional_param_count(&init_method.param_infos);
            let mut param_types: Vec<Vec<Type>> = vec![Vec::new(); positional_count];
            for site in &data.call_sites {
                if site.method_name.as_ref() == "initialize" && !site.method_is_singleton {
                    Self::merge_call_site_positional_types(
                        &mut param_types,
                        site,
                        &init_method.param_infos,
                    );
                }
            }
            let resolved_params: Vec<Type> = param_types
                .into_iter()
                .map(|types| {
                    if types.is_empty() {
                        Type::Untyped
                    } else {
                        Type::from_type_vec(types)
                    }
                })
                .collect();
            let widened_params: Vec<Type> = resolved_params.iter().map(|ty| ty.widen()).collect();

            if !resolved_params
                .iter()
                .any(|ty| !matches!(ty, Type::Untyped))
            {
                continue;
            }

            let ivar_names: Vec<Sym> = data.ivars.keys().copied().collect();
            let method_updates: Vec<(Sym, bool, Type)> = data
                .methods
                .iter()
                .filter_map(|method| {
                    // `initialize` param substitution only applies to synthesized Struct/Data `initialize`/`with` and `initialize` itself (`ParamRef` for ordinary methods is handled elsewhere).
                    if !method.synthetic_dsl_source && method.name != "initialize" {
                        return None;
                    }
                    if !Self::type_contains_param_ref_static(&method.raw_return_type) {
                        return None;
                    }
                    let resolved = Self::substitute_param_refs_static(
                        &method.raw_return_type,
                        &widened_params,
                    );
                    (resolved != method.raw_return_type).then_some((
                        method.name,
                        method.is_singleton,
                        resolved,
                    ))
                })
                .collect();

            for ivar_name in &ivar_names {
                self.resolve_ivar_param_refs(class_name, ivar_name.as_str(), &widened_params);
            }
            for (method_name, is_singleton, resolved) in method_updates {
                self.update_method_return_type_variant(
                    class_name,
                    &method_name,
                    is_singleton,
                    resolved,
                );
            }
        }
    }

    /// checks whether a literal Untyped is present: decides if a call-site type can be written to the method (an untyped union would be incorrectly narrowed downstream).
    fn type_contains_untyped_member(ty: &Type) -> bool {
        match ty {
            Type::Untyped => true,
            Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => {
                parts.iter().any(Self::type_contains_untyped_member)
            }
            Type::Array(Some(inner)) => Self::type_contains_untyped_member(inner),
            Type::Hash(k, v) => {
                k.as_deref().is_some_and(Self::type_contains_untyped_member)
                    || v.as_deref().is_some_and(Self::type_contains_untyped_member)
            }
            Type::Record(fields) => fields
                .iter()
                .any(|field| Self::type_contains_untyped_member(&field.value)),
            Type::Proc { return_type, .. } => Self::type_contains_untyped_member(return_type),
            _ => false,
        }
    }

    fn type_contains_param_ref_static(ty: &Type) -> bool {
        match ty {
            Type::ParamRef(_) | Type::KeywordParamRef(_) => true,
            Type::Union(parts) | Type::Intersection(parts) => {
                parts.iter().any(Self::type_contains_param_ref_static)
            }
            Type::Array(Some(inner)) => Self::type_contains_param_ref_static(inner),
            Type::Hash(Some(k), Some(v)) => {
                Self::type_contains_param_ref_static(k) || Self::type_contains_param_ref_static(v)
            }
            Type::Hash(Some(k), None) => Self::type_contains_param_ref_static(k),
            Type::Hash(None, Some(v)) => Self::type_contains_param_ref_static(v),
            Type::Record(fields) => fields
                .iter()
                .any(|field| Self::type_contains_param_ref_static(&field.value)),
            Type::Tuple(elems) => elems.iter().any(Self::type_contains_param_ref_static),
            Type::Proc { return_type, .. } => Self::type_contains_param_ref_static(return_type),
            Type::PatternIndexRef(subject, _)
            | Type::PatternRestRef(subject)
            | Type::PatternTrailingRef(subject, _)
            | Type::PatternKeyRef(subject, _)
            | Type::PatternKeyRestRef(subject, _)
            | Type::ReceiverMethodRef(subject, _) => Self::type_contains_param_ref_static(subject),
            _ => false,
        }
    }

    // after merge: resolve scoped type refs for `T::Struct` props/consts lexically (unresolved ones stay pending).
    pub(crate) fn finalize_pending_scoped_type_refs(&mut self) {
        let pending = self.take_pending_scoped_type_refs();
        if pending.is_empty() {
            return;
        }
        for p in pending {
            let (resolved, any_resolved, any_unresolved) =
                self.resolve_scoped_refs_in_type(&p.declaration_scope, &p.raw_type);
            if any_resolved {
                self.update_method_return_type_variant(
                    &p.owner_class,
                    &p.method_name,
                    p.is_singleton,
                    resolved,
                );
            }
            if any_unresolved {
                // keep forward references pending if not yet merged; leave the bare name if it doesn't exist.
                self.push_pending_scoped_type_ref(p);
            }
        }
    }

    fn resolve_scoped_refs_in_type(&self, scope_class: &str, ty: &Type) -> (Type, bool, bool) {
        // recursive helper for `Option<Box<Type>>` slots.
        fn resolve_opt(
            this: &TypeRegistry,
            scope: &str,
            slot: &Option<Box<Type>>,
            any_resolved: &mut bool,
            any_unresolved: &mut bool,
        ) -> Option<Box<Type>> {
            slot.as_ref().map(|inner| {
                let (ty, resolved, unresolved) = this.resolve_scoped_refs_in_type(scope, inner);
                *any_resolved |= resolved;
                *any_unresolved |= unresolved;
                Box::new(ty)
            })
        }
        fn resolve_vec(
            this: &TypeRegistry,
            scope: &str,
            parts: &[Type],
            any_resolved: &mut bool,
            any_unresolved: &mut bool,
        ) -> Vec<Type> {
            parts
                .iter()
                .map(|part| {
                    let (ty, resolved, unresolved) = this.resolve_scoped_refs_in_type(scope, part);
                    *any_resolved |= resolved;
                    *any_unresolved |= unresolved;
                    ty
                })
                .collect()
        }

        let mut any_resolved = false;
        let mut any_unresolved = false;
        let resolved_type = match ty {
            Type::Class(name) => match self.resolve_scoped_nominal_name(scope_class, name.as_str())
            {
                Some(canonical) => {
                    any_resolved = true;
                    Type::Class(Sym::new(&canonical))
                }
                None => {
                    any_unresolved = true;
                    ty.clone()
                }
            },
            Type::Singleton(name) => {
                match self.resolve_scoped_nominal_name(scope_class, name.as_str()) {
                    Some(canonical) => {
                        any_resolved = true;
                        Type::Singleton(Sym::new(&canonical))
                    }
                    None => {
                        any_unresolved = true;
                        ty.clone()
                    }
                }
            }
            Type::Array(inner) => Type::Array(resolve_opt(
                self,
                scope_class,
                inner,
                &mut any_resolved,
                &mut any_unresolved,
            )),
            Type::Hash(key, value) => Type::Hash(
                resolve_opt(
                    self,
                    scope_class,
                    key,
                    &mut any_resolved,
                    &mut any_unresolved,
                ),
                resolve_opt(
                    self,
                    scope_class,
                    value,
                    &mut any_resolved,
                    &mut any_unresolved,
                ),
            ),
            Type::Union(parts) => Type::Union(resolve_vec(
                self,
                scope_class,
                parts,
                &mut any_resolved,
                &mut any_unresolved,
            )),
            Type::Intersection(parts) => Type::Intersection(resolve_vec(
                self,
                scope_class,
                parts,
                &mut any_resolved,
                &mut any_unresolved,
            )),
            Type::Tuple(elems) => Type::Tuple(resolve_vec(
                self,
                scope_class,
                elems,
                &mut any_resolved,
                &mut any_unresolved,
            )),
            Type::Generic { base, args } => {
                let base = match self.resolve_scoped_nominal_name(scope_class, base.as_str()) {
                    Some(canonical) => {
                        any_resolved = true;
                        Sym::new(&canonical)
                    }
                    None => {
                        any_unresolved = true;
                        *base
                    }
                };
                Type::Generic {
                    base,
                    args: resolve_vec(
                        self,
                        scope_class,
                        args,
                        &mut any_resolved,
                        &mut any_unresolved,
                    )
                    .into(),
                }
            }
            Type::Proc {
                return_type,
                param_count,
            } => {
                let (ret, resolved, unresolved) =
                    self.resolve_scoped_refs_in_type(scope_class, return_type);
                any_resolved |= resolved;
                any_unresolved |= unresolved;
                Type::Proc {
                    return_type: Box::new(ret),
                    param_count: *param_count,
                }
            }
            // everything else (builtin / literal / various refs) is treated as having no nominal
            // reference and is returned as-is.
            _ => ty.clone(),
        };
        (resolved_type, any_resolved, any_unresolved)
    }

    /// resolve a raw nominal name in lexical scope and return the canonical key that actually
    /// exists in the class graph (no leading `::`). Returns `None` if it doesn't exist (stays pending).
    fn resolve_scoped_nominal_name(&self, scope_class: &str, raw_name: &str) -> Option<String> {
        let resolved = self.resolve_scoped_class_ref(scope_class, raw_name);
        let canonical = resolved.trim_scope_prefix();
        self.class_data
            .contains_key(canonical)
            .then(|| canonical.to_string())
    }

    /// whether the type contains a nominal reference (`Class` / `Singleton` /
    /// `Generic` base) eligible for scoped resolution. Used by prop/const collection to decide whether to queue it as pending.
    pub(crate) fn type_contains_scoped_nominal_ref(ty: &Type) -> bool {
        match ty {
            Type::Class(_) | Type::Singleton(_) | Type::Generic { .. } => true,
            Type::Array(Some(inner)) => Self::type_contains_scoped_nominal_ref(inner),
            Type::Hash(key, value) => {
                key.as_deref()
                    .is_some_and(Self::type_contains_scoped_nominal_ref)
                    || value
                        .as_deref()
                        .is_some_and(Self::type_contains_scoped_nominal_ref)
            }
            Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => {
                parts.iter().any(Self::type_contains_scoped_nominal_ref)
            }
            Type::Proc { return_type, .. } => Self::type_contains_scoped_nominal_ref(return_type),
            _ => false,
        }
    }

    fn resolve_subclass_method_refs_global(&mut self) {
        use rayon::prelude::*;

        let class_names = self.user_defined_class_names_unsorted();
        // evaluate in parallel over the frozen registry, then apply in bulk in a deterministic class order (same Jacobi structure as the method-return-ref worklist).
        let this: &Self = self;
        let jobs: Vec<(&Sym, &MethodDef)> = class_names
            .iter()
            .filter_map(|class_name| {
                this.class_data
                    .get(class_name)
                    .map(|data| (class_name, data))
            })
            .flat_map(|(class_name, data)| {
                data.methods
                    .iter()
                    .filter(|method| {
                        Self::type_contains_subclass_ref_candidate(&method.raw_return_type)
                    })
                    .map(move |method| (class_name, method.as_ref()))
            })
            .collect();
        let updates: Vec<(&Sym, Sym, bool, Type)> = jobs
            .par_iter()
            .filter_map(|&(class_name, method)| {
                let resolved = this.resolve_subclass_refs_in_type(&method.raw_return_type);
                (resolved != method.raw_return_type).then_some((
                    class_name,
                    method.name,
                    method.is_singleton,
                    resolved,
                ))
            })
            .collect();
        for (class_name, method_name, is_singleton, resolved) in updates {
            self.update_method_return_type_variant(
                class_name,
                &method_name,
                is_singleton,
                resolved,
            );
        }
    }

    fn type_contains_subclass_ref_candidate(ty: &Type) -> bool {
        match ty {
            Type::MethodReturnRef(..) | Type::ReceiverMethodRef(..) => true,
            Type::Union(parts) | Type::Intersection(parts) => {
                parts.iter().any(Self::type_contains_subclass_ref_candidate)
            }
            Type::Array(Some(inner))
            | Type::PatternRestRef(inner)
            | Type::PatternTrailingRef(inner, _)
            | Type::PatternIndexRef(inner, _)
            | Type::PatternKeyRef(inner, _)
            | Type::PatternKeyRestRef(inner, _) => {
                Self::type_contains_subclass_ref_candidate(inner)
            }
            Type::Proc { return_type, .. } => {
                Self::type_contains_subclass_ref_candidate(return_type)
            }
            Type::Tuple(elems) => elems.iter().any(Self::type_contains_subclass_ref_candidate),
            Type::Hash(Some(key), Some(value)) => {
                Self::type_contains_subclass_ref_candidate(key)
                    || Self::type_contains_subclass_ref_candidate(value)
            }
            Type::Hash(Some(key), None) => Self::type_contains_subclass_ref_candidate(key),
            Type::Hash(None, Some(value)) => Self::type_contains_subclass_ref_candidate(value),
            Type::Record(fields) => fields
                .iter()
                .any(|field| Self::type_contains_subclass_ref_candidate(&field.value)),
            _ => false,
        }
    }

    fn resolve_subclass_refs_in_type(&self, ty: &Type) -> Type {
        match ty {
            Type::MethodReturnRef(class_name, method_name) => {
                if self
                    .resolve_method_call_owners(class_name, method_name, false)
                    .is_empty()
                    && self
                        .resolve_method_call_owners(class_name, method_name, true)
                        .is_empty()
                    && let Some(ret) =
                        self.lookup_method_return_type_in_subclasses(class_name, method_name)
                    && !matches!(ret, Type::MethodReturnRef(..))
                    && !Self::type_contains_param_ref_static(&ret)
                {
                    return ret;
                }
                ty.clone()
            }
            Type::ReceiverMethodRef(receiver_type, method_name) => {
                let resolved_receiver = self.resolve_subclass_refs_in_type(receiver_type);
                if let Some(receiver_class) = Self::type_to_class_name(&resolved_receiver) {
                    let prefer_singleton = matches!(resolved_receiver, Type::Singleton(_));
                    if let Some(ret) = self.lookup_method_return_type(&receiver_class, method_name)
                        && ret != Type::Untyped
                        && !matches!(ret, Type::MethodReturnRef(..) | Type::ReceiverMethodRef(..))
                        && !Self::type_contains_param_ref_static(&ret)
                    {
                        // resolve an `instance` return to the receiver's instance type (prevents misattribution from baking in an unresolved owner).
                        return ret.replace_instance_type(&Self::instance_type_for_receiver(
                            &resolved_receiver,
                        ));
                    }
                    if let Some(ivar_ty) = self.resolve_attr_reader_return_type(
                        &receiver_class,
                        method_name,
                        prefer_singleton,
                    ) {
                        return ivar_ty;
                    }
                }
                Type::ReceiverMethodRef(Box::new(resolved_receiver), *method_name)
            }
            Type::Union(parts) => Type::from_type_vec(
                parts
                    .iter()
                    .map(|t| self.resolve_subclass_refs_in_type(t))
                    .collect(),
            ),
            Type::Intersection(parts) => Type::Intersection(
                parts
                    .iter()
                    .map(|t| self.resolve_subclass_refs_in_type(t))
                    .collect(),
            ),
            Type::Array(Some(inner)) => {
                Type::Array(Some(Box::new(self.resolve_subclass_refs_in_type(inner))))
            }
            Type::Hash(Some(key), Some(value)) => Type::Hash(
                Some(Box::new(self.resolve_subclass_refs_in_type(key))),
                Some(Box::new(self.resolve_subclass_refs_in_type(value))),
            ),
            Type::Hash(Some(key), None) => Type::Hash(
                Some(Box::new(self.resolve_subclass_refs_in_type(key))),
                None,
            ),
            Type::Hash(None, Some(value)) => Type::Hash(
                None,
                Some(Box::new(self.resolve_subclass_refs_in_type(value))),
            ),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: self.resolve_subclass_refs_in_type(&field.value),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|t| self.resolve_subclass_refs_in_type(t))
                    .collect(),
            ),
            Type::Proc {
                return_type,
                param_count,
            } => Type::Proc {
                return_type: Box::new(self.resolve_subclass_refs_in_type(return_type)),
                param_count: *param_count,
            },
            Type::PatternIndexRef(subject, index) => {
                let resolved_subject = self.resolve_subclass_refs_in_type(subject);
                if Self::type_contains_subclass_ref_candidate(&resolved_subject) {
                    Type::PatternIndexRef(Box::new(resolved_subject), *index)
                } else {
                    Self::resolve_pattern_index_ref(&resolved_subject, *index)
                }
            }
            Type::PatternRestRef(subject) => {
                let resolved_subject = self.resolve_subclass_refs_in_type(subject);
                if Self::type_contains_subclass_ref_candidate(&resolved_subject) {
                    Type::PatternRestRef(Box::new(resolved_subject))
                } else {
                    Self::resolve_pattern_rest_ref(&resolved_subject)
                }
            }
            Type::PatternTrailingRef(subject, from_end) => {
                let resolved_subject = self.resolve_subclass_refs_in_type(subject);
                if Self::type_contains_subclass_ref_candidate(&resolved_subject) {
                    Type::PatternTrailingRef(Box::new(resolved_subject), *from_end)
                } else {
                    Self::resolve_pattern_trailing_ref(&resolved_subject, *from_end)
                }
            }
            Type::PatternKeyRef(subject, key) => {
                let resolved_subject = self.resolve_subclass_refs_in_type(subject);
                if Self::type_contains_subclass_ref_candidate(&resolved_subject) {
                    Type::PatternKeyRef(Box::new(resolved_subject), key.clone())
                } else {
                    Self::resolve_pattern_key_ref(&resolved_subject, key)
                }
            }
            Type::PatternKeyRestRef(subject, matched_keys) => {
                let resolved_subject = self.resolve_subclass_refs_in_type(subject);
                if Self::type_contains_subclass_ref_candidate(&resolved_subject) {
                    Type::PatternKeyRestRef(Box::new(resolved_subject), matched_keys.clone())
                } else {
                    Self::resolve_pattern_key_rest_ref(&resolved_subject, matched_keys)
                }
            }
            _ => ty.clone(),
        }
    }

    pub fn resolve_method_return_refs_global(&mut self) {
        let class_names = self.user_defined_class_names_unsorted();
        self.resolve_method_return_refs_for_classes(&class_names);
    }

    fn resolve_method_return_refs_for_classes(&mut self, class_names: &[Sym]) {
        use rayon::prelude::*;

        if class_names.is_empty() {
            return;
        }

        // Worklist of method slots whose return type still carries symbolic
        // refs, in deterministic (class list, method index) order.
        let mut pending: Vec<(Sym, usize)> = Vec::new();
        for &class_name in class_names {
            let Some(data) = self.class_data.get(&class_name) else {
                continue;
            };
            for (method_idx, method) in data.methods.iter().enumerate() {
                if Self::global_type_contains_ref(&method.raw_return_type) {
                    pending.push((class_name, method_idx));
                }
            }
        }

        // slot wake: only wakes on the (owner class, method name) return types read by the previous evaluation (only the return type is mutable, everything else is frozen).
        struct SlotWake {
            alive: bool,
            reads: Vec<(Sym, Sym)>,
        }

        let slot_alive = |registry: &Self, class_name: Sym, method_idx: usize| -> bool {
            registry
                .class_data
                .get(&class_name)
                .and_then(|data| data.methods.get(method_idx))
                .is_some_and(|method| Self::global_type_contains_ref(&method.raw_return_type))
        };

        let mut wake: Vec<SlotWake> = pending
            .iter()
            .map(|_| SlotWake {
                alive: true,
                reads: Vec::new(),
            })
            .collect();
        let mut eval_set: Vec<usize> = (0..pending.len()).collect();

        // Jacobi round: evaluate all slots against the registry frozen at round start, then apply in bulk (order-independent fixpoint; the old 8-round cap left deep chains unresolved).
        const ROUND_BACKSTOP: usize = 1024;
        // Below this many slots the rayon fan-out costs more than it saves
        // (LSP display requests resolve a handful of neighborhood classes).
        const PARALLEL_THRESHOLD: usize = 8;
        let round_timing = std::env::var_os("TYDA_RESOLUTION_TIMING").is_some();
        for round in 0..=ROUND_BACKSTOP {
            if round == ROUND_BACKSTOP {
                eprintln!(
                    "tyda: method-return-ref resolution did not settle after {ROUND_BACKSTOP} rounds; leaving remaining refs unresolved"
                );
                break;
            }
            if eval_set.is_empty() {
                break;
            }
            let round_start = std::time::Instant::now();
            type SlotResult = (usize, Option<(Sym, bool, Type)>, Vec<(Sym, Sym)>);
            let evaluate = |&slot: &usize| -> SlotResult {
                let (class_name, method_idx) = pending[slot];
                let Some(method) = self
                    .class_data
                    .get(&class_name)
                    .and_then(|data| data.methods.get(method_idx))
                else {
                    return (slot, None, Vec::new());
                };
                if !Self::global_type_contains_ref(&method.raw_return_type) {
                    return (slot, None, Vec::new());
                }
                let mut visiting = FxHashSet::default();
                let mut memo = FxHashMap::default();
                visiting.insert((class_name, Sym::new(method.name)));
                RETURN_TYPE_READS.with(|cell| {
                    *cell.borrow_mut() = Some(FxHashSet::default());
                });
                let mut resolve_budget = GlobalResolveBudget::new();
                let resolved = self.resolve_global_refs(
                    class_name.as_str(),
                    &method.raw_return_type,
                    &mut visiting,
                    &mut memo,
                    0,
                    &mut resolve_budget,
                );
                let reads: Vec<(Sym, Sym)> = RETURN_TYPE_READS
                    .with(|cell| cell.borrow_mut().take())
                    .map(|set| set.into_iter().collect())
                    .unwrap_or_default();
                let update = (resolved != method.raw_return_type).then_some((
                    method.name,
                    method.is_singleton,
                    resolved,
                ));
                (slot, update, reads)
            };
            // the registry is frozen for the whole round, so attr-reader returns memoize
            // across slots; the next round's writes make the entries stale, hence the reset.
            self.attr_reader_return_cache.clear();
            self.attr_reader_return_cache_enabled
                .store(true, std::sync::atomic::Ordering::Release);
            let results: Vec<SlotResult> = if eval_set.len() >= PARALLEL_THRESHOLD {
                eval_set.par_iter().map(evaluate).collect()
            } else {
                eval_set.iter().map(evaluate).collect()
            };
            self.attr_reader_return_cache_enabled
                .store(false, std::sync::atomic::Ordering::Release);
            let update_count = results
                .iter()
                .filter(|(_, update, _)| update.is_some())
                .count();
            if round_timing {
                eprintln!(
                    "method-return-refs round {round}: evaluated={} updates={} {:.3}s",
                    eval_set.len(),
                    update_count,
                    round_start.elapsed().as_secs_f64()
                );
            }
            let mut updated_keys: FxHashSet<(Sym, Sym)> = FxHashSet::default();
            let mut changed_slots: Vec<usize> = Vec::new();
            for (slot, update, reads) in results {
                wake[slot].reads = reads;
                let Some((method_name, is_singleton, resolved)) = update else {
                    continue;
                };
                let class_name = pending[slot].0;
                updated_keys.insert((class_name, method_name));
                changed_slots.push(slot);
                self.update_method_return_type_variant(
                    class_name.as_str(),
                    &method_name,
                    is_singleton,
                    resolved,
                );
            }
            if updated_keys.is_empty() {
                break;
            }
            for &slot in &changed_slots {
                let (class_name, method_idx) = pending[slot];
                wake[slot].alive = slot_alive(self, class_name, method_idx);
            }
            eval_set = (0..pending.len())
                .filter(|&slot| {
                    let meta = &wake[slot];
                    meta.alive && meta.reads.iter().any(|key| updated_keys.contains(key))
                })
                .collect();
        }
        self.attr_reader_return_cache = AttrReaderReturnCache::default();
    }

    fn global_type_contains_ref(ty: &Type) -> bool {
        match ty {
            Type::MethodReturnRef(..)
            | Type::ReceiverMethodRef(..)
            | Type::IvarRef(..)
            | Type::GlobalVariableRef(..) => true,
            Type::Union(parts) | Type::Intersection(parts) => {
                parts.iter().any(Self::global_type_contains_ref)
            }
            Type::Array(Some(inner)) => Self::global_type_contains_ref(inner),
            Type::Tuple(elems) => elems.iter().any(Self::global_type_contains_ref),
            Type::Hash(Some(k), Some(v)) => {
                Self::global_type_contains_ref(k) || Self::global_type_contains_ref(v)
            }
            Type::Hash(Some(k), None) => Self::global_type_contains_ref(k),
            Type::Hash(None, Some(v)) => Self::global_type_contains_ref(v),
            Type::Record(fields) => fields
                .iter()
                .any(|field| Self::global_type_contains_ref(&field.value)),
            Type::Proc { return_type, .. } => Self::global_type_contains_ref(return_type),
            Type::PatternIndexRef(subject, _)
            | Type::PatternRestRef(subject)
            | Type::PatternTrailingRef(subject, _)
            | Type::PatternKeyRef(subject, _)
            | Type::PatternKeyRestRef(subject, _) => Self::global_type_contains_ref(subject),
            _ => false,
        }
    }

    /// determines deferred ref types: used to resolve `IvarRef` rendering for synthesized Struct/Data `initialize`/`with`.
    fn type_is_deferred_ref(ty: &Type) -> bool {
        matches!(
            ty,
            Type::IvarRef(_)
                | Type::MethodReturnRef(..)
                | Type::ReceiverMethodRef(..)
                | Type::ParamRef(_)
                | Type::KeywordParamRef(_)
        )
    }

    fn is_concrete_for_global_resolve(ty: &Type) -> bool {
        match ty {
            Type::Integer
            | Type::Float
            | Type::String
            | Type::Symbol
            | Type::Bool
            | Type::True
            | Type::False
            | Type::Nil
            | Type::Void
            | Type::Top
            | Type::Bot
            | Type::LiteralInteger(_)
            | Type::LiteralFloat(_)
            | Type::LiteralString(_)
            | Type::LiteralSymbol(_)
            | Type::Class(_)
            | Type::Singleton(_)
            | Type::SelfType
            | Type::InstanceType => true,
            // structured Generics are atomically concrete (recursing into args would break compatibility with the old `Class("Base[args]")`).
            Type::Generic { .. } => true,
            Type::Array(None) | Type::Hash(None, None) => true,
            Type::Array(Some(inner)) => Self::is_concrete_for_global_resolve(inner),
            Type::Hash(Some(k), Some(v)) => {
                Self::is_concrete_for_global_resolve(k) && Self::is_concrete_for_global_resolve(v)
            }
            Type::Hash(Some(k), None) => Self::is_concrete_for_global_resolve(k),
            Type::Hash(None, Some(v)) => Self::is_concrete_for_global_resolve(v),
            Type::Union(parts) => parts.iter().all(Self::is_concrete_for_global_resolve),
            Type::Intersection(parts) => parts.iter().all(Self::is_concrete_for_global_resolve),
            Type::Tuple(elems) => elems.iter().all(Self::is_concrete_for_global_resolve),
            Type::Record(fields) => fields
                .iter()
                .all(|field| Self::is_concrete_for_global_resolve(&field.value)),
            Type::Proc { return_type, .. } => Self::is_concrete_for_global_resolve(return_type),
            _ => false,
        }
    }

    fn deferred_receiver_method_ref(
        original_receiver: &Type,
        resolved_receiver: Type,
        method_name: Sym,
    ) -> Type {
        let receiver = if Self::global_type_contains_ref(original_receiver) {
            original_receiver.clone()
        } else {
            resolved_receiver
        };
        Type::ReceiverMethodRef(Box::new(receiver), method_name)
    }

    fn resolve_global_refs(
        &self,
        context_class: &str,
        ty: &Type,
        visiting: &mut FxHashSet<(Sym, Sym)>,
        memo: &mut FxHashMap<Sym, FxHashMap<Type, Type>>,
        depth: usize,
        budget: &mut GlobalResolveBudget,
    ) -> Type {
        if budget.is_exhausted() {
            return Type::Untyped;
        }
        if depth >= 12 {
            return ty.clone();
        }
        if !Self::global_type_contains_ref(ty) {
            return ty.clone();
        }
        if !budget.consume() {
            return Type::Untyped;
        }
        // keyed per context so the type probes by reference: building a `(Sym, Type)`
        // key would deep-clone `ty` on every lookup, which outweighs the memo on wide unions.
        let context_key = Sym::new(context_class);
        if let Some(cached) = memo.get(&context_key).and_then(|slot| slot.get(ty)) {
            return cached.clone();
        }
        let resolved = match ty {
            Type::GlobalVariableRef(name) => {
                // globals don't depend on class context. Resolves to the type accumulated program-wide.
                self.lookup_global_variable_type(name)
                    .unwrap_or(Type::Untyped)
            }
            Type::IvarRef(ivar_name) => {
                let key = (Sym::new(context_class), *ivar_name);
                if visiting.contains(&key) {
                    ty.clone()
                } else {
                    visiting.insert(key);
                    let ivar_type =
                        self.lookup_ivar_type_through_ancestors(context_class, ivar_name);
                    let result = match ivar_type {
                        Some(Type::Untyped)
                        | None
                        | Some(Type::ParamRef(_))
                        | Some(Type::KeywordParamRef(_)) => {
                            let from_init =
                                self.infer_attr_type_from_initialize(context_class, ivar_name);
                            match from_init {
                                Some(ref t) if Self::is_concrete_for_global_resolve(t) => t.clone(),
                                _ => self
                                    .infer_attr_type_from_initialize_camel_for_class(
                                        context_class,
                                        ivar_name,
                                    )
                                    .unwrap_or_else(|| ty.clone()),
                            }
                        }
                        Some(ref resolved) if Self::is_concrete_for_global_resolve(resolved) => {
                            resolved.clone()
                        }
                        Some(resolved) => {
                            let deep = self.resolve_global_refs(
                                context_class,
                                &resolved,
                                visiting,
                                memo,
                                depth + 1,
                                budget,
                            );
                            if Self::is_concrete_for_global_resolve(&deep) {
                                deep
                            } else {
                                self.infer_attr_type_from_initialize_camel_for_class(
                                    context_class,
                                    ivar_name,
                                )
                                .unwrap_or(deep)
                            }
                        }
                    };
                    visiting.remove(&key);
                    result
                }
            }
            Type::MethodReturnRef(class_name, method_name) => {
                if visiting.contains(&(*class_name, *method_name)) {
                    // Record the cycle partner: if it resolves through some
                    // other path later, this slot must be re-evaluated.
                    note_return_type_read(*class_name, *method_name);
                    ty.clone()
                } else {
                    let ret = self
                        .lookup_method_return_type_direct_sym(*class_name, *method_name)
                        .or_else(|| self.lookup_method_return_type(class_name, method_name));
                    if let Some(ret) = ret {
                        visiting.insert((*class_name, *method_name));
                        let resolved = self.resolve_global_refs(
                            class_name,
                            &ret,
                            visiting,
                            memo,
                            depth + 1,
                            budget,
                        );
                        visiting.remove(&(*class_name, *method_name));
                        if Self::is_concrete_for_global_resolve(&resolved) {
                            resolved
                        } else {
                            ty.clone()
                        }
                    } else {
                        ty.clone()
                    }
                }
            }
            Type::ReceiverMethodRef(receiver_type, method_name) => {
                let resolved_receiver = self.resolve_global_refs(
                    context_class,
                    receiver_type,
                    visiting,
                    memo,
                    depth + 1,
                    budget,
                );
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                // union receiver: only produce a union return type once all members are concretized, otherwise keep the ref (prevents incorrect concretization).
                if let Type::Union(members) = &resolved_receiver {
                    let mut resolved_members: Vec<Type> = Vec::with_capacity(members.len());
                    let mut all_concrete = true;
                    for member in members.iter() {
                        let member_ref =
                            Type::ReceiverMethodRef(Box::new(member.clone()), *method_name);
                        let resolved_member = self.resolve_global_refs(
                            context_class,
                            &member_ref,
                            visiting,
                            memo,
                            depth + 1,
                            budget,
                        );
                        if budget.is_exhausted() {
                            return Type::Untyped;
                        }
                        if Self::is_concrete_for_global_resolve(&resolved_member) {
                            resolved_members.push(resolved_member);
                        } else {
                            all_concrete = false;
                            break;
                        }
                    }
                    if all_concrete {
                        Type::from_type_vec(resolved_members)
                    } else {
                        Self::deferred_receiver_method_ref(
                            receiver_type,
                            resolved_receiver,
                            *method_name,
                        )
                    }
                } else if let Some(receiver_class) = Self::type_to_class_name(&resolved_receiver) {
                    let prefer_singleton = matches!(resolved_receiver, Type::Singleton(_));
                    let ret = self
                        .lookup_method_return_type_direct(&receiver_class, method_name)
                        .or_else(|| self.lookup_method_return_type(&receiver_class, method_name));
                    if let Some(ret) = ret {
                        let resolved = self.resolve_global_refs(
                            &receiver_class,
                            &ret,
                            visiting,
                            memo,
                            depth + 1,
                            budget,
                        );
                        if budget.is_exhausted() {
                            return Type::Untyped;
                        }
                        // even on the deferred path, resolve `instance` to the receiver's instance type (prevents collapsing to the owner).
                        let resolved = resolved.replace_instance_type(
                            &Self::instance_type_for_receiver(&resolved_receiver),
                        );
                        if Self::is_concrete_for_global_resolve(&resolved) {
                            resolved
                        } else if let Some(ivar_ty) = self.resolve_attr_reader_return_type(
                            &receiver_class,
                            method_name,
                            prefer_singleton,
                        ) {
                            ivar_ty
                        } else {
                            Self::deferred_receiver_method_ref(
                                receiver_type,
                                resolved_receiver,
                                *method_name,
                            )
                        }
                    } else if let Some(ivar_ty) = self.resolve_attr_reader_return_type(
                        &receiver_class,
                        method_name,
                        prefer_singleton,
                    ) {
                        ivar_ty
                    } else if let Some(stdlib_ret) = stdlib_returns::stdlib_receiver_method_return(
                        &resolved_receiver,
                        method_name.as_str(),
                    ) {
                        // stdlib on the deferred path: a pure table of return-invariant entries only (no lazy loader).
                        stdlib_ret
                    } else {
                        Self::deferred_receiver_method_ref(
                            receiver_type,
                            resolved_receiver,
                            *method_name,
                        )
                    }
                } else {
                    Self::deferred_receiver_method_ref(
                        receiver_type,
                        resolved_receiver,
                        *method_name,
                    )
                }
            }
            // a no-progress container is returned as-is without re-normalizing (avoids sort/dedup CPU cost dominating every round for blocked slots).
            Type::Union(parts) => {
                let mut resolved = Vec::with_capacity(parts.len());
                for part in parts {
                    resolved.push(self.resolve_global_refs(
                        context_class,
                        part,
                        visiting,
                        memo,
                        depth + 1,
                        budget,
                    ));
                    if budget.is_exhausted() {
                        return Type::Untyped;
                    }
                }
                if resolved == *parts {
                    ty.clone()
                } else if resolved
                    .iter()
                    .zip(parts.iter())
                    .any(|(r, p)| *r == Type::Untyped && *p != Type::Untyped)
                {
                    // a member resolved to untyped counts as no-progress (`from_type_vec` drops untyped, which would incorrectly narrow `untyped|nil`->`nil`).
                    ty.clone()
                } else {
                    Type::from_type_vec(resolved)
                }
            }
            Type::Intersection(parts) => {
                let mut resolved = Vec::with_capacity(parts.len());
                for part in parts {
                    resolved.push(self.resolve_global_refs(
                        context_class,
                        part,
                        visiting,
                        memo,
                        depth + 1,
                        budget,
                    ));
                    if budget.is_exhausted() {
                        return Type::Untyped;
                    }
                }
                if resolved == *parts {
                    ty.clone()
                } else {
                    Type::Intersection(resolved)
                }
            }
            Type::Array(Some(inner)) => {
                let resolved = self.resolve_global_refs(
                    context_class,
                    inner,
                    visiting,
                    memo,
                    depth + 1,
                    budget,
                );
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                if resolved == **inner {
                    ty.clone()
                } else {
                    Type::Array(Some(Box::new(resolved)))
                }
            }
            Type::Hash(Some(key), Some(value)) => {
                let resolved_key =
                    self.resolve_global_refs(context_class, key, visiting, memo, depth + 1, budget);
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                let resolved_value = self.resolve_global_refs(
                    context_class,
                    value,
                    visiting,
                    memo,
                    depth + 1,
                    budget,
                );
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                if resolved_key == **key && resolved_value == **value {
                    ty.clone()
                } else {
                    Type::Hash(Some(Box::new(resolved_key)), Some(Box::new(resolved_value)))
                }
            }
            Type::Hash(Some(key), None) => {
                let resolved =
                    self.resolve_global_refs(context_class, key, visiting, memo, depth + 1, budget);
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                if resolved == **key {
                    ty.clone()
                } else {
                    Type::Hash(Some(Box::new(resolved)), None)
                }
            }
            Type::Hash(None, Some(value)) => {
                let resolved = self.resolve_global_refs(
                    context_class,
                    value,
                    visiting,
                    memo,
                    depth + 1,
                    budget,
                );
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                if resolved == **value {
                    ty.clone()
                } else {
                    Type::Hash(None, Some(Box::new(resolved)))
                }
            }
            Type::Record(fields) => {
                let mut resolved = Vec::with_capacity(fields.len());
                for field in fields {
                    resolved.push(RecordField {
                        key: field.key.clone(),
                        value: self.resolve_global_refs(
                            context_class,
                            &field.value,
                            visiting,
                            memo,
                            depth + 1,
                            budget,
                        ),
                        optional: field.optional,
                    });
                    if budget.is_exhausted() {
                        return Type::Untyped;
                    }
                }
                if resolved == *fields {
                    ty.clone()
                } else {
                    Type::Record(resolved)
                }
            }
            Type::Tuple(elems) => {
                let mut resolved = Vec::with_capacity(elems.len());
                for elem in elems {
                    resolved.push(self.resolve_global_refs(
                        context_class,
                        elem,
                        visiting,
                        memo,
                        depth + 1,
                        budget,
                    ));
                    if budget.is_exhausted() {
                        return Type::Untyped;
                    }
                }
                if resolved == *elems {
                    ty.clone()
                } else {
                    Type::Tuple(resolved)
                }
            }
            Type::Proc {
                return_type,
                param_count,
            } => Type::Proc {
                return_type: Box::new(self.resolve_global_refs(
                    context_class,
                    return_type,
                    visiting,
                    memo,
                    depth + 1,
                    budget,
                )),
                param_count: *param_count,
            },
            Type::PatternIndexRef(subject, index) => {
                let resolved_subject = self.resolve_global_refs(
                    context_class,
                    subject,
                    visiting,
                    memo,
                    depth + 1,
                    budget,
                );
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                if Self::global_type_contains_ref(&resolved_subject) {
                    Type::PatternIndexRef(Box::new(resolved_subject), *index)
                } else {
                    Self::resolve_pattern_index_ref(&resolved_subject, *index)
                }
            }
            Type::PatternRestRef(subject) => {
                let resolved_subject = self.resolve_global_refs(
                    context_class,
                    subject,
                    visiting,
                    memo,
                    depth + 1,
                    budget,
                );
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                if Self::global_type_contains_ref(&resolved_subject) {
                    Type::PatternRestRef(Box::new(resolved_subject))
                } else {
                    Self::resolve_pattern_rest_ref(&resolved_subject)
                }
            }
            Type::PatternTrailingRef(subject, from_end) => {
                let resolved_subject = self.resolve_global_refs(
                    context_class,
                    subject,
                    visiting,
                    memo,
                    depth + 1,
                    budget,
                );
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                if Self::global_type_contains_ref(&resolved_subject) {
                    Type::PatternTrailingRef(Box::new(resolved_subject), *from_end)
                } else {
                    Self::resolve_pattern_trailing_ref(&resolved_subject, *from_end)
                }
            }
            Type::PatternKeyRef(subject, key) => {
                let resolved_subject = self.resolve_global_refs(
                    context_class,
                    subject,
                    visiting,
                    memo,
                    depth + 1,
                    budget,
                );
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                if Self::global_type_contains_ref(&resolved_subject) {
                    Type::PatternKeyRef(Box::new(resolved_subject), key.clone())
                } else {
                    Self::resolve_pattern_key_ref(&resolved_subject, key)
                }
            }
            Type::PatternKeyRestRef(subject, matched_keys) => {
                let resolved_subject = self.resolve_global_refs(
                    context_class,
                    subject,
                    visiting,
                    memo,
                    depth + 1,
                    budget,
                );
                if budget.is_exhausted() {
                    return Type::Untyped;
                }
                if Self::global_type_contains_ref(&resolved_subject) {
                    Type::PatternKeyRestRef(Box::new(resolved_subject), matched_keys.clone())
                } else {
                    Self::resolve_pattern_key_rest_ref(&resolved_subject, matched_keys)
                }
            }
            _ => ty.clone(),
        };
        if budget.is_exhausted() {
            return Type::Untyped;
        }
        if Self::is_concrete_for_global_resolve(&resolved) {
            memo.entry(context_key)
                .or_default()
                .insert(ty.clone(), resolved.clone());
        }
        resolved
    }

    fn resolve_param_refs_from_resolved(&self, ty: &Type, params: &[Param]) -> Type {
        let Some(_guard) = ResolveDepthGuard::enter() else {
            return Type::Untyped;
        };
        match ty {
            Type::KeywordParamRef(name) => params
                .iter()
                .find(|p| &p.name == name)
                .map(|p| p.param_type.clone().widen())
                .unwrap_or(Type::Untyped),
            Type::ParamRef(idx) => {
                let mut pos_idx = 0;
                for p in params {
                    if matches!(
                        p.kind,
                        ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                    ) {
                        if pos_idx == *idx {
                            return p.param_type.clone().widen();
                        }
                        pos_idx += 1;
                    }
                }
                Type::Untyped
            }
            Type::ReceiverMethodRef(receiver_type, method_name) => {
                let resolved_receiver =
                    self.resolve_param_refs_from_resolved(receiver_type, params);
                if let Some(cls) = Self::type_to_class_name(&resolved_receiver) {
                    let prefer_singleton = matches!(resolved_receiver, Type::Singleton(_));
                    let ret = self
                        .lookup_method_return_type_with_hint(&cls, method_name, prefer_singleton)
                        .unwrap_or(Type::Untyped);
                    let ret = Self::substitute_self_type(&ret, &resolved_receiver);
                    self.resolve_deferred_refs(&cls, &ret)
                } else if let Type::Union(parts) = &resolved_receiver {
                    let mut resolved_parts: Vec<Type> = Vec::new();
                    for part in parts {
                        if let Some(cls) = Self::type_to_class_name(part) {
                            let prefer_singleton = matches!(part, Type::Singleton(_));
                            let ret = self
                                .lookup_method_return_type_with_hint(
                                    &cls,
                                    method_name,
                                    prefer_singleton,
                                )
                                .unwrap_or(Type::Untyped);
                            let ret = Self::substitute_self_type(&ret, part);
                            let resolved = self.resolve_deferred_refs(&cls, &ret);
                            if resolved != Type::Untyped {
                                resolved_parts.push(resolved);
                            }
                        }
                    }
                    if resolved_parts.is_empty() {
                        Type::Untyped
                    } else {
                        // don't drop untyped contained in a member's resolution result
                        // (otherwise `nil | untyped` narrows to `nil`, giving the wrong concrete type).
                        Type::from_type_vec_preserve_untyped(resolved_parts)
                    }
                } else {
                    Type::Untyped
                }
            }
            Type::PatternIndexRef(subject, index) => {
                let resolved_subject = self.resolve_param_refs_from_resolved(subject, params);
                Self::resolve_pattern_index_ref(&resolved_subject, *index)
            }
            Type::PatternRestRef(subject) => {
                let resolved_subject = self.resolve_param_refs_from_resolved(subject, params);
                Self::resolve_pattern_rest_ref(&resolved_subject)
            }
            Type::PatternTrailingRef(subject, from_end) => {
                let resolved_subject = self.resolve_param_refs_from_resolved(subject, params);
                Self::resolve_pattern_trailing_ref(&resolved_subject, *from_end)
            }
            Type::PatternKeyRef(subject, key) => {
                let resolved_subject = self.resolve_param_refs_from_resolved(subject, params);
                Self::resolve_pattern_key_ref(&resolved_subject, key)
            }
            Type::PatternKeyRestRef(subject, matched_keys) => {
                let resolved_subject = self.resolve_param_refs_from_resolved(subject, params);
                Self::resolve_pattern_key_rest_ref(&resolved_subject, matched_keys)
            }
            Type::Union(parts) => {
                let resolved: Vec<Type> = parts
                    .iter()
                    .map(|t| self.resolve_param_refs_from_resolved(t, params))
                    .collect();
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Intersection(parts) => Type::Intersection(
                parts
                    .iter()
                    .map(|t| self.resolve_param_refs_from_resolved(t, params))
                    .collect(),
            ),
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(
                self.resolve_param_refs_from_resolved(inner, params),
            ))),
            Type::Hash(Some(k), Some(v)) => Type::Hash(
                Some(Box::new(self.resolve_param_refs_from_resolved(k, params))),
                Some(Box::new(self.resolve_param_refs_from_resolved(v, params))),
            ),
            Type::Hash(Some(k), None) => Type::Hash(
                Some(Box::new(self.resolve_param_refs_from_resolved(k, params))),
                None,
            ),
            Type::Hash(None, Some(v)) => Type::Hash(
                None,
                Some(Box::new(self.resolve_param_refs_from_resolved(v, params))),
            ),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|t| self.resolve_param_refs_from_resolved(t, params))
                    .collect(),
            ),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: self.resolve_param_refs_from_resolved(&field.value, params),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Proc {
                return_type,
                param_count,
            } => Type::Proc {
                return_type: Box::new(self.resolve_param_refs_from_resolved(return_type, params)),
                param_count: *param_count,
            },
            _ => ty.clone(),
        }
    }

    fn resolve_deferred_refs(&self, class_name: &str, ty: &Type) -> Type {
        self.resolve_deferred_refs_for_context(class_name, false, ty)
    }

    /// Same result as `resolve_deferred_refs_for_context`, but a ref-free input is handed
    /// straight back instead of being deep-cloned (the common case on the render path).
    pub(super) fn resolve_deferred_refs_for_context_owned(
        &self,
        class_name: &str,
        singleton_context: bool,
        ty: Type,
    ) -> Type {
        if !Self::type_needs_deferred_resolution(&ty) {
            return ty;
        }
        self.resolve_deferred_refs_for_context(class_name, singleton_context, &ty)
    }

    fn type_needs_deferred_resolution(ty: &Type) -> bool {
        Self::type_needs_deferred_resolution_depth(ty, 0)
    }

    fn type_needs_deferred_resolution_depth(ty: &Type, depth: usize) -> bool {
        if depth >= Self::DEFERRED_REF_MAX_DEPTH {
            return true;
        }
        match ty {
            Type::IvarRef(_)
            | Type::GlobalVariableRef(_)
            | Type::MethodReturnRef(..)
            | Type::ReceiverMethodRef(..)
            | Type::PatternIndexRef(..)
            | Type::PatternRestRef(..)
            | Type::PatternTrailingRef(..)
            | Type::PatternKeyRef(..)
            | Type::PatternKeyRestRef(..) => true,
            Type::Union(parts) | Type::Intersection(parts) => parts
                .iter()
                .any(|part| Self::type_needs_deferred_resolution_depth(part, depth + 1)),
            Type::Array(Some(inner)) => {
                Self::type_needs_deferred_resolution_depth(inner, depth + 1)
            }
            Type::Hash(Some(k), Some(v)) => {
                Self::type_needs_deferred_resolution_depth(k, depth + 1)
                    || Self::type_needs_deferred_resolution_depth(v, depth + 1)
            }
            Type::Hash(Some(k), None) => Self::type_needs_deferred_resolution_depth(k, depth + 1),
            Type::Hash(None, Some(v)) => Self::type_needs_deferred_resolution_depth(v, depth + 1),
            Type::Record(fields) => fields
                .iter()
                .any(|field| Self::type_needs_deferred_resolution_depth(&field.value, depth + 1)),
            Type::Proc { return_type, .. } => {
                Self::type_needs_deferred_resolution_depth(return_type, depth + 1)
            }
            Type::Tuple(elems) => elems
                .iter()
                .any(|elem| Self::type_needs_deferred_resolution_depth(elem, depth + 1)),
            _ => false,
        }
    }

    /// substitute `self`/`[self]` with the receiver (turns a concern's `Relation[self]` into the includer).
    pub fn substitute_self_type_pub(ty: &Type, receiver_type: &Type) -> Type {
        Self::substitute_self_type(ty, receiver_type)
    }

    pub(super) fn resolve_deferred_refs_for_context(
        &self,
        class_name: &str,
        singleton_context: bool,
        ty: &Type,
    ) -> Type {
        if !Self::type_needs_deferred_resolution(ty) {
            return ty.clone();
        }
        let Some(_guard) = ResolveDepthGuard::enter() else {
            return Type::Untyped;
        };
        let hop_key = DEFERRED_HOP_MEMO.with(|cell| {
            cell.borrow().is_some().then(|| {
                Arc::new(DeferredKey::new(
                    self.shared_name(class_name),
                    singleton_context,
                    ty.clone(),
                ))
            })
        });
        if let Some(key) = &hop_key {
            let hit = DEFERRED_HOP_MEMO.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .and_then(|memo| memo.get(key).cloned())
            });
            if let Some(hit) = hit {
                return hit;
            }
        }
        let mut memo = FxHashMap::default();
        let mut visiting = FxHashSet::default();
        let resolved = self.resolve_deferred_refs_depth(
            class_name,
            singleton_context,
            ty,
            0,
            &mut memo,
            &mut visiting,
        );
        if let Some(key) = hop_key {
            DEFERRED_HOP_MEMO.with(|cell| {
                if let Some(memo) = cell.borrow_mut().as_mut() {
                    memo.insert(key, resolved.clone());
                }
            });
        }
        resolved
    }

    /// deferred-ref memo gate: huge unions are unsuitable as memo keys (expensive clone/hash); cycles always go through leaf refs, so containers can skip `visiting`.
    fn type_is_memo_small(ty: &Type, budget: &mut usize) -> bool {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        match ty {
            Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => parts
                .iter()
                .all(|part| Self::type_is_memo_small(part, budget)),
            Type::Array(Some(inner)) => Self::type_is_memo_small(inner, budget),
            Type::Hash(key, value) => {
                key.as_deref()
                    .is_none_or(|key| Self::type_is_memo_small(key, budget))
                    && value
                        .as_deref()
                        .is_none_or(|value| Self::type_is_memo_small(value, budget))
            }
            Type::Record(fields) => fields
                .iter()
                .all(|field| Self::type_is_memo_small(&field.value, budget)),
            Type::Proc { return_type, .. } => Self::type_is_memo_small(return_type, budget),
            Type::PatternIndexRef(subject, _)
            | Type::PatternRestRef(subject)
            | Type::PatternTrailingRef(subject, _)
            | Type::PatternKeyRef(subject, _)
            | Type::PatternKeyRestRef(subject, _) => Self::type_is_memo_small(subject, budget),
            _ => true,
        }
    }

    fn resolve_deferred_refs_depth(
        &self,
        class_name: &str,
        singleton_context: bool,
        ty: &Type,
        depth: usize,
        memo: &mut DeferredMemo,
        visiting: &mut DeferredVisiting,
    ) -> Type {
        if depth >= Self::DEFERRED_REF_MAX_DEPTH {
            return Type::Untyped;
        }
        if !Self::type_needs_deferred_resolution_depth(ty, depth) {
            return ty.clone();
        }
        let mut memo_budget = 48usize;
        let key = Self::type_is_memo_small(ty, &mut memo_budget).then(|| {
            Arc::new(DeferredKey::new(
                self.shared_name(class_name),
                singleton_context,
                ty.clone(),
            ))
        });
        if let Some(key) = &key {
            if let Some(resolved) = memo.get(key) {
                return resolved.clone();
            }
            if !visiting.insert(Arc::clone(key)) {
                return Type::Untyped;
            }
        }
        let resolved = match ty {
            Type::GlobalVariableRef(name) => self
                .lookup_global_variable_type(name)
                .unwrap_or(Type::Untyped),
            Type::IvarRef(ivar_name) => {
                let ivar_type = if singleton_context {
                    self.lookup_singleton_ivar_type(class_name, ivar_name)
                } else {
                    self.lookup_ivar_type_through_ancestors(class_name, ivar_name)
                };
                match ivar_type {
                    Some(Type::Untyped)
                    | None
                    | Some(Type::ParamRef(_))
                    | Some(Type::KeywordParamRef(_)) => {
                        if singleton_context {
                            Type::Untyped
                        } else {
                            self.infer_attr_type_from_initialize(class_name, ivar_name)
                                .unwrap_or(Type::Untyped)
                        }
                    }
                    Some(ty) => ty,
                }
            }
            Type::KeywordParamRef(_) => ty.clone(),
            Type::MethodReturnRef(ref_class, method_name) => {
                for singleton in [false, true] {
                    if let Some((owner, is_singleton)) =
                        self.first_method_call_owner_cached(ref_class, method_name, singleton)
                    {
                        let Some(data) = self.class_data.get(owner.as_ref()) else {
                            continue;
                        };
                        let Some(method) =
                            Self::method_for_lookup_kind(data, method_name, Some(is_singleton))
                        else {
                            continue;
                        };
                        let raw_with_block = self.resolve_block_return_refs(
                            owner.as_ref(),
                            method,
                            &method.raw_return_type,
                        );
                        let raw = self.resolve_deferred_refs_depth(
                            owner.as_ref(),
                            is_singleton,
                            &raw_with_block,
                            depth + 1,
                            memo,
                            visiting,
                        );
                        // `resolve_param_refs_from_resolved` reads `params` only at a reachable
                        // `ParamRef`/`KeywordParamRef`, which is exactly what
                        // `type_contains_param_ref` detects, so resolving the owner's params is
                        // pure waste otherwise.
                        let params = if Self::type_contains_param_ref(&raw) {
                            self.resolve_params(owner.as_ref(), method)
                        } else {
                            Vec::new()
                        };
                        let resolved = self.resolve_param_refs_from_resolved(&raw, &params);
                        if resolved != Type::Untyped {
                            return resolved;
                        }
                        if let Some(ivar_ty) = self.resolve_attr_reader_return_type(
                            owner.as_ref(),
                            method_name,
                            is_singleton,
                        ) {
                            return ivar_ty;
                        }
                    }
                }
                if let Some(ret) =
                    self.lookup_method_return_type_in_subclasses(ref_class, method_name)
                    && !matches!(ret, Type::MethodReturnRef(..))
                {
                    let resolved = self.resolve_deferred_refs_depth(
                        ref_class,
                        false,
                        &ret,
                        depth + 1,
                        memo,
                        visiting,
                    );
                    if resolved != Type::Untyped {
                        return resolved;
                    }
                }
                Type::Untyped
            }
            Type::ReceiverMethodRef(receiver_type, method_name) => {
                let resolved_receiver = self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    receiver_type,
                    depth + 1,
                    memo,
                    visiting,
                );
                if Self::type_contains_param_ref(&resolved_receiver) {
                    Type::ReceiverMethodRef(Box::new(resolved_receiver), *method_name)
                } else {
                    self.resolve_method_on_resolved_receiver(
                        class_name,
                        &resolved_receiver,
                        method_name,
                        depth,
                        memo,
                        visiting,
                    )
                }
            }
            Type::PatternIndexRef(subject, index) => {
                let resolved_subject = self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    subject,
                    depth + 1,
                    memo,
                    visiting,
                );
                Self::resolve_pattern_index_ref(&resolved_subject, *index)
            }
            Type::PatternRestRef(subject) => {
                let resolved_subject = self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    subject,
                    depth + 1,
                    memo,
                    visiting,
                );
                Self::resolve_pattern_rest_ref(&resolved_subject)
            }
            Type::PatternTrailingRef(subject, from_end) => {
                let resolved_subject = self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    subject,
                    depth + 1,
                    memo,
                    visiting,
                );
                Self::resolve_pattern_trailing_ref(&resolved_subject, *from_end)
            }
            Type::PatternKeyRef(subject, key) => {
                let resolved_subject = self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    subject,
                    depth + 1,
                    memo,
                    visiting,
                );
                Self::resolve_pattern_key_ref(&resolved_subject, key)
            }
            Type::PatternKeyRestRef(subject, matched_keys) => {
                let resolved_subject = self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    subject,
                    depth + 1,
                    memo,
                    visiting,
                );
                Self::resolve_pattern_key_rest_ref(&resolved_subject, matched_keys)
            }
            // children with no deferred ref pass through as-is; skip re-normalizing if nothing changed (avoids sorting huge param unions on every wake).
            Type::Union(parts) => {
                let resolved: Vec<Type> = parts
                    .iter()
                    .map(|t| {
                        if !Self::type_needs_deferred_resolution(t) {
                            t.clone()
                        } else {
                            self.resolve_deferred_refs_depth(
                                class_name,
                                singleton_context,
                                t,
                                depth + 1,
                                memo,
                                visiting,
                            )
                        }
                    })
                    .collect();
                if resolved == *parts {
                    ty.clone()
                } else {
                    Type::from_type_vec_preserve_untyped(resolved)
                }
            }
            Type::Intersection(parts) => {
                let resolved: Vec<Type> = parts
                    .iter()
                    .map(|t| {
                        if !Self::type_needs_deferred_resolution(t) {
                            t.clone()
                        } else {
                            self.resolve_deferred_refs_depth(
                                class_name,
                                singleton_context,
                                t,
                                depth + 1,
                                memo,
                                visiting,
                            )
                        }
                    })
                    .collect();
                if resolved == *parts {
                    ty.clone()
                } else {
                    Type::Intersection(resolved)
                }
            }
            Type::Array(Some(inner)) => {
                Type::Array(Some(Box::new(self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    inner,
                    depth + 1,
                    memo,
                    visiting,
                ))))
            }
            Type::Hash(Some(k), Some(v)) => Type::Hash(
                Some(Box::new(self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    k,
                    depth + 1,
                    memo,
                    visiting,
                ))),
                Some(Box::new(self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    v,
                    depth + 1,
                    memo,
                    visiting,
                ))),
            ),
            Type::Hash(Some(k), None) => Type::Hash(
                Some(Box::new(self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    k,
                    depth + 1,
                    memo,
                    visiting,
                ))),
                None,
            ),
            Type::Hash(None, Some(v)) => Type::Hash(
                None,
                Some(Box::new(self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    v,
                    depth + 1,
                    memo,
                    visiting,
                ))),
            ),
            Type::Record(fields) => {
                let resolved: Vec<RecordField> = fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: self.resolve_deferred_refs_depth(
                            class_name,
                            singleton_context,
                            &field.value,
                            depth + 1,
                            memo,
                            visiting,
                        ),
                        optional: field.optional,
                    })
                    .collect();
                Type::Record(resolved)
            }
            Type::Proc {
                return_type,
                param_count,
            } => Type::Proc {
                return_type: Box::new(self.resolve_deferred_refs_depth(
                    class_name,
                    singleton_context,
                    return_type,
                    depth + 1,
                    memo,
                    visiting,
                )),
                param_count: *param_count,
            },
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|t| {
                        self.resolve_deferred_refs_depth(
                            class_name,
                            singleton_context,
                            t,
                            depth + 1,
                            memo,
                            visiting,
                        )
                    })
                    .collect(),
            ),
            _ => ty.clone(),
        };
        if let Some(key) = key {
            visiting.remove(&key);
            memo.insert(key, resolved.clone());
        }
        resolved
    }

    fn resolve_method_on_resolved_receiver(
        &self,
        _class_name: &str,
        resolved_receiver: &Type,
        method_name: &str,
        depth: usize,
        memo: &mut DeferredMemo,
        visiting: &mut DeferredVisiting,
    ) -> Type {
        if let Some(ivar_name) = Self::receiver_ivar_reflection_name(method_name) {
            return self.resolve_receiver_ivar_reflection(
                resolved_receiver,
                ivar_name,
                depth,
                memo,
                visiting,
            );
        }
        if let Some(variable_name) = Self::receiver_class_variable_reflection_name(method_name) {
            return self.resolve_receiver_class_variable_reflection(
                resolved_receiver,
                variable_name,
                depth,
                memo,
                visiting,
            );
        }
        if let Some(result) =
            Self::active_record_relation_method_return(resolved_receiver, method_name)
        {
            return result;
        }
        if let Some(cls) = Self::type_to_class_name(resolved_receiver) {
            let prefer_singleton = matches!(resolved_receiver, Type::Singleton(_));
            let ret = self
                .lookup_method_return_type_with_hint(&cls, method_name, prefer_singleton)
                .unwrap_or(Type::Untyped);
            let ret = Self::substitute_self_type(&ret, resolved_receiver);
            let resolved = self.resolve_deferred_refs_depth(
                &cls,
                prefer_singleton,
                &ret,
                depth + 1,
                memo,
                visiting,
            );
            if resolved != Type::Untyped {
                return resolved;
            }
            if let Some(ivar_ty) =
                self.resolve_attr_reader_return_type(&cls, method_name, prefer_singleton)
            {
                return ivar_ty;
            }
            return resolved;
        }
        if let Type::Union(parts) = resolved_receiver {
            let mut resolved_parts: Vec<Type> = Vec::new();
            for part in parts {
                if let Some(cls) = Self::type_to_class_name(part) {
                    let prefer_singleton = matches!(part, Type::Singleton(_));
                    let ret = self
                        .lookup_method_return_type_with_hint(&cls, method_name, prefer_singleton)
                        .unwrap_or(Type::Untyped);
                    let ret = Self::substitute_self_type(&ret, part);
                    let resolved = self.resolve_deferred_refs_depth(
                        &cls,
                        prefer_singleton,
                        &ret,
                        depth + 1,
                        memo,
                        visiting,
                    );
                    if resolved != Type::Untyped {
                        resolved_parts.push(resolved);
                    } else if let Some(ivar_ty) =
                        self.resolve_attr_reader_return_type(&cls, method_name, prefer_singleton)
                    {
                        resolved_parts.push(ivar_ty);
                    }
                }
            }
            if !resolved_parts.is_empty() {
                // don't drop untyped contained in a member's resolution result
                // (otherwise `nil | untyped` narrows to `nil`, giving the wrong concrete type).
                return Type::from_type_vec_preserve_untyped(resolved_parts);
            }
        }
        if method_name == "!" || method_name.ends_with('?') {
            return Type::Bool;
        }
        match method_name {
            "to_s" | "to_str" | "inspect" => return Type::String,
            "to_i" | "to_int" | "count" | "size" | "length" | "hash" | "object_id" => {
                return Type::Integer;
            }
            "to_f" => return Type::Float,
            "to_a" => return Type::Array(None),
            "to_h" => return Type::Hash(None, None),
            _ => {}
        }
        Type::Untyped
    }

    /// Keep the common Active Record relation chain alive while resolving a
    /// deferred receiver outside the inference engine. A source file may
    /// return a relation from a model method defined in another file; the
    /// registry has no AST call node at that point, but these methods preserve
    /// the relation shape independently of their arguments.
    fn active_record_relation_method_return(
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let elem_type = match receiver_type {
            Type::Generic { base, args }
                if matches!(
                    base.as_str(),
                    "ActiveRecord::Relation"
                        | "ActiveRecord::AssociationRelation"
                        | "ActiveRecord::CollectionProxy"
                ) =>
            {
                args.first()?.clone()
            }
            _ => return None,
        };

        match method_name {
            "all" | "where" | "rewhere" | "not" | "or" | "and" | "merge" | "order" | "reorder"
            | "limit" | "offset" | "reselect" | "group" | "regroup" | "having" | "joins"
            | "left_outer_joins" | "includes" | "eager_load" | "preload" | "references"
            | "unscope" | "extending" | "optimizer_hints" | "annotate" | "excluding"
            | "create_with" | "distinct" | "distinct!" | "readonly" | "lock" | "strict_loading"
            | "skip_query_cache" | "without" | "load" | "load_async" | "async" | "none"
            | "invert_where" | "reverse_order" => Some(receiver_type.clone()),
            "ids" => Some(Type::Array(Some(Box::new(Type::Integer)))),
            "count" | "size" | "length" => Some(Type::Integer),
            "any?" | "many?" | "none?" | "one?" | "exists?" | "include?" => Some(Type::Bool),
            "to_a" => Some(Type::Array(Some(Box::new(elem_type)))),
            _ => None,
        }
    }

    fn receiver_ivar_reflection_name(method_name: &str) -> Option<&str> {
        method_name.strip_prefix("__tyda_instance_variable_get__:")
    }

    fn receiver_class_variable_reflection_name(method_name: &str) -> Option<&str> {
        method_name.strip_prefix("__tyda_class_variable_get__:")
    }

    fn resolve_receiver_ivar_reflection(
        &self,
        resolved_receiver: &Type,
        ivar_name: &str,
        depth: usize,
        memo: &mut DeferredMemo,
        visiting: &mut DeferredVisiting,
    ) -> Type {
        if let Some(cls) = Self::type_to_class_name(resolved_receiver) {
            let singleton_context = matches!(resolved_receiver, Type::Singleton(_));
            let raw = if singleton_context {
                self.lookup_singleton_ivar_type(&cls, ivar_name)
            } else {
                self.lookup_ivar_type(&cls, ivar_name)
            };
            return match raw {
                Some(Type::Untyped)
                | Some(Type::ParamRef(_))
                | Some(Type::KeywordParamRef(_))
                | None
                    if !singleton_context =>
                {
                    self.infer_attr_type_from_initialize(&cls, ivar_name)
                        .unwrap_or(Type::Untyped)
                }
                Some(ty) => self.resolve_deferred_refs_depth(
                    &cls,
                    singleton_context,
                    &ty,
                    depth + 1,
                    memo,
                    visiting,
                ),
                None => Type::Untyped,
            };
        }

        if let Type::Union(parts) = resolved_receiver {
            let resolved_parts: Vec<Type> = parts
                .iter()
                .map(|part| {
                    self.resolve_receiver_ivar_reflection(part, ivar_name, depth, memo, visiting)
                })
                .filter(|ty| *ty != Type::Untyped)
                .collect();
            if !resolved_parts.is_empty() {
                return Type::from_type_vec(resolved_parts);
            }
        }

        Type::Untyped
    }

    fn resolve_receiver_class_variable_reflection(
        &self,
        resolved_receiver: &Type,
        variable_name: &str,
        depth: usize,
        memo: &mut DeferredMemo,
        visiting: &mut DeferredVisiting,
    ) -> Type {
        if let Type::Singleton(cls) = resolved_receiver {
            return self
                .lookup_class_variable_type(cls, variable_name)
                .map(|ty| {
                    self.resolve_deferred_refs_depth(cls, true, &ty, depth + 1, memo, visiting)
                })
                .unwrap_or(Type::Untyped);
        }

        if let Type::Union(parts) = resolved_receiver {
            let resolved_parts: Vec<Type> = parts
                .iter()
                .map(|part| {
                    self.resolve_receiver_class_variable_reflection(
                        part,
                        variable_name,
                        depth,
                        memo,
                        visiting,
                    )
                })
                .filter(|ty| *ty != Type::Untyped)
                .collect();
            if !resolved_parts.is_empty() {
                return Type::from_type_vec(resolved_parts);
            }
        }

        Type::Untyped
    }

    pub fn type_to_class_name_pub(ty: &Type) -> Option<String> {
        Self::type_to_class_name(ty)
    }

    fn type_to_class_name(ty: &Type) -> Option<String> {
        match ty {
            Type::Integer | Type::LiteralInteger(_) => Some("Integer".to_string()),
            Type::Float | Type::LiteralFloat(_) => Some("Float".to_string()),
            Type::String | Type::LiteralString(_) => Some("String".to_string()),
            Type::Symbol | Type::LiteralSymbol(_) => Some("Symbol".to_string()),
            Type::Bool => Some("bool".to_string()),
            Type::True => Some("TrueClass".to_string()),
            Type::False => Some("FalseClass".to_string()),
            Type::Nil => Some("NilClass".to_string()),
            Type::Array(_) | Type::Tuple(_) => Some("Array".to_string()),
            Type::Hash(_, _) | Type::Record(_) => Some("Hash".to_string()),
            Type::Class(name) => Some(Self::strip_type_arguments(name).to_string()),
            Type::Generic { base, .. } => Some(base.as_str().to_string()),
            Type::Singleton(name) => Some((name.clone()).to_string()),
            _ => None,
        }
    }

    /// singleton receiver->instance type, used to resolve `instance` returns at the call site.
    fn instance_type_for_receiver(receiver_type: &Type) -> Type {
        match receiver_type {
            Type::Singleton(name) => Type::Class(*name),
            Type::Union(parts) => Type::from_type_vec_preserve_untyped(
                parts.iter().map(Self::instance_type_for_receiver).collect(),
            ),
            other => other.clone(),
        }
    }

    fn substitute_self_type(ty: &Type, receiver_type: &Type) -> Type {
        match ty {
            Type::SelfType => receiver_type.clone(),
            Type::InstanceType => Self::instance_type_for_receiver(receiver_type),
            // `self` inside a Generic resolves to the receiver's instance type (even for a singleton, it's `X` not `singleton(X)`, for compatibility with the old flat form).
            Type::Generic { base, args } => {
                let arg_receiver = Self::type_to_class_name(receiver_type)
                    .map(|name| Type::Class(name.into()))
                    .unwrap_or_else(|| receiver_type.clone());
                Type::Generic {
                    base: *base,
                    args: args
                        .iter()
                        .map(|arg| Self::substitute_self_type(arg, &arg_receiver))
                        .collect(),
                }
            }
            Type::Union(parts) => {
                let resolved: Vec<Type> = parts
                    .iter()
                    .map(|t| Self::substitute_self_type(t, receiver_type))
                    .collect();
                Type::from_type_vec(resolved)
            }
            Type::Intersection(parts) => Type::Intersection(
                parts
                    .iter()
                    .map(|t| Self::substitute_self_type(t, receiver_type))
                    .collect(),
            ),
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(Self::substitute_self_type(
                inner,
                receiver_type,
            )))),
            Type::Hash(Some(k), Some(v)) => Type::Hash(
                Some(Box::new(Self::substitute_self_type(k, receiver_type))),
                Some(Box::new(Self::substitute_self_type(v, receiver_type))),
            ),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|t| Self::substitute_self_type(t, receiver_type))
                    .collect(),
            ),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: Self::substitute_self_type(&field.value, receiver_type),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    /// The caller-context walk only rewrites `ParamRef` / `KeywordParamRef`. Every other
    /// node is rebuilt from its children, which reproduces the input exactly — except
    /// `Type::Union`, which goes back through `from_type_vec_preserve_untyped` and can
    /// come out re-normalized. A subtree with neither is handed straight back.
    fn call_site_walk_is_identity(ty: &Type) -> bool {
        match ty {
            Type::ParamRef(_) | Type::KeywordParamRef(_) | Type::Union(_) => false,
            Type::Intersection(parts) | Type::Tuple(parts) => {
                parts.iter().all(Self::call_site_walk_is_identity)
            }
            Type::Array(Some(inner)) => Self::call_site_walk_is_identity(inner),
            Type::Hash(key, value) => {
                key.as_deref().is_none_or(Self::call_site_walk_is_identity)
                    && value
                        .as_deref()
                        .is_none_or(Self::call_site_walk_is_identity)
            }
            Type::Record(fields) => fields
                .iter()
                .all(|field| Self::call_site_walk_is_identity(&field.value)),
            Type::PatternIndexRef(subject, _)
            | Type::PatternRestRef(subject)
            | Type::PatternTrailingRef(subject, _)
            | Type::PatternKeyRef(subject, _)
            | Type::PatternKeyRestRef(subject, _)
            | Type::ReceiverMethodRef(subject, _) => Self::call_site_walk_is_identity(subject),
            Type::Proc { return_type, .. } => Self::call_site_walk_is_identity(return_type),
            _ => true,
        }
    }

    fn resolve_call_site_type_from_caller_context(
        &self,
        call_site: &CallSite,
        ty: &Type,
        visiting: &mut FxHashSet<(String, String, bool)>,
    ) -> Type {
        if Self::call_site_walk_is_identity(ty) {
            return ty.clone();
        }
        let memo_key = call_site.caller_context.as_deref().map(|ctx| {
            (
                ctx.class_name.clone(),
                ctx.method_name.clone(),
                ctx.method_is_singleton,
                ty.clone(),
            )
        });
        if let Some(key) = &memo_key {
            let hit = CALLER_CTX_MEMO.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .and_then(|memo| memo.get(key).cloned())
            });
            if let Some(hit) = hit {
                return hit;
            }
        }
        let resolved =
            self.resolve_call_site_type_from_caller_context_uncached(call_site, ty, visiting);
        if let Some(key) = memo_key {
            CALLER_CTX_MEMO.with(|cell| {
                if let Some(memo) = cell.borrow_mut().as_mut() {
                    memo.insert(key, resolved.clone());
                }
            });
        }
        resolved
    }

    fn resolve_call_site_type_from_caller_context_uncached(
        &self,
        call_site: &CallSite,
        ty: &Type,
        visiting: &mut FxHashSet<(String, String, bool)>,
    ) -> Type {
        match ty {
            Type::ParamRef(idx) => self
                .resolve_caller_positional_param_type(call_site, *idx, visiting)
                .unwrap_or(Type::Untyped),
            Type::KeywordParamRef(name) => self
                .resolve_caller_keyword_param_type(call_site, name, visiting)
                .unwrap_or(Type::Untyped),
            Type::Union(parts) => Type::from_type_vec_preserve_untyped(
                parts
                    .iter()
                    .map(|part| {
                        self.resolve_call_site_type_from_caller_context(call_site, part, visiting)
                    })
                    .collect(),
            ),
            Type::Intersection(parts) => Type::Intersection(
                parts
                    .iter()
                    .map(|part| {
                        self.resolve_call_site_type_from_caller_context(call_site, part, visiting)
                    })
                    .collect(),
            ),
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(
                self.resolve_call_site_type_from_caller_context(call_site, inner, visiting),
            ))),
            Type::Hash(Some(key), Some(value)) => Type::Hash(
                Some(Box::new(self.resolve_call_site_type_from_caller_context(
                    call_site, key, visiting,
                ))),
                Some(Box::new(self.resolve_call_site_type_from_caller_context(
                    call_site, value, visiting,
                ))),
            ),
            Type::Hash(Some(key), None) => Type::Hash(
                Some(Box::new(self.resolve_call_site_type_from_caller_context(
                    call_site, key, visiting,
                ))),
                None,
            ),
            Type::Hash(None, Some(value)) => Type::Hash(
                None,
                Some(Box::new(self.resolve_call_site_type_from_caller_context(
                    call_site, value, visiting,
                ))),
            ),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|elem| {
                        self.resolve_call_site_type_from_caller_context(call_site, elem, visiting)
                    })
                    .collect(),
            ),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: self.resolve_call_site_type_from_caller_context(
                            call_site,
                            &field.value,
                            visiting,
                        ),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::PatternIndexRef(subject, index) => Type::PatternIndexRef(
                Box::new(
                    self.resolve_call_site_type_from_caller_context(call_site, subject, visiting),
                ),
                *index,
            ),
            Type::PatternRestRef(subject) => Type::PatternRestRef(Box::new(
                self.resolve_call_site_type_from_caller_context(call_site, subject, visiting),
            )),
            Type::PatternTrailingRef(subject, from_end) => Type::PatternTrailingRef(
                Box::new(
                    self.resolve_call_site_type_from_caller_context(call_site, subject, visiting),
                ),
                *from_end,
            ),
            Type::PatternKeyRef(subject, key) => Type::PatternKeyRef(
                Box::new(
                    self.resolve_call_site_type_from_caller_context(call_site, subject, visiting),
                ),
                key.clone(),
            ),
            Type::PatternKeyRestRef(subject, keys) => Type::PatternKeyRestRef(
                Box::new(
                    self.resolve_call_site_type_from_caller_context(call_site, subject, visiting),
                ),
                keys.clone(),
            ),
            Type::ReceiverMethodRef(receiver_type, method_name) => Type::ReceiverMethodRef(
                Box::new(self.resolve_call_site_type_from_caller_context(
                    call_site,
                    receiver_type,
                    visiting,
                )),
                *method_name,
            ),
            Type::Proc {
                return_type,
                param_count,
            } => Type::Proc {
                return_type: Box::new(self.resolve_call_site_type_from_caller_context(
                    call_site,
                    return_type,
                    visiting,
                )),
                param_count: *param_count,
            },
            _ => ty.clone(),
        }
    }

    fn resolve_caller_positional_param_type(
        &self,
        call_site: &CallSite,
        index: usize,
        visiting: &mut FxHashSet<(String, String, bool)>,
    ) -> Option<Type> {
        let context = call_site.caller_context.as_ref()?;
        let params = self.resolve_method_params_with_caller_context(
            context.class_name.as_ref(),
            context.method_name.as_ref(),
            context.method_is_singleton,
            visiting,
        )?;
        params
            .into_iter()
            .filter(|param| {
                matches!(
                    param.kind,
                    ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                )
            })
            .nth(index)
            .map(|param| param.param_type)
    }

    fn resolve_caller_keyword_param_type(
        &self,
        call_site: &CallSite,
        name: &str,
        visiting: &mut FxHashSet<(String, String, bool)>,
    ) -> Option<Type> {
        let context = call_site.caller_context.as_ref()?;
        let params = self.resolve_method_params_with_caller_context(
            context.class_name.as_ref(),
            context.method_name.as_ref(),
            context.method_is_singleton,
            visiting,
        )?;
        params
            .into_iter()
            .find(|param| {
                param.name == name
                    && matches!(
                        param.kind,
                        ParamKind::KeywordRequired | ParamKind::KeywordOptional
                    )
            })
            .map(|param| param.param_type)
    }

    fn resolve_method_params_with_caller_context(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
        visiting: &mut FxHashSet<(String, String, bool)>,
    ) -> Option<Vec<Param>> {
        let _guard = ResolveDepthGuard::enter()?;
        if PARAM_TABLE_MODE.with(|cell| cell.get()) {
            let table_key = (
                self.shared_name(class_name),
                self.shared_name(method_name),
                is_singleton,
            );
            note_param_table_read(table_key.clone());
            return self.resolve_params_cache.get(&table_key);
        }
        let key = (
            class_name.to_string(),
            method_name.to_string(),
            is_singleton,
        );
        if !visiting.insert(key.clone()) {
            return None;
        }
        let params = self
            .lookup_method_def(class_name, method_name, is_singleton)
            .map(|method| self.resolve_params_inner(class_name, method, visiting));
        visiting.remove(&key);
        params
    }

    /// consults the call-site index (classes below the threshold return `None`->linear scan).
    fn matching_call_site_indices(
        &self,
        class_name: &str,
        data: &ClassData,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<Vec<u32>> {
        if data.call_sites.len() < CallSiteIndexCache::LINEAR_SCAN_THRESHOLD {
            return None;
        }
        let shard = self.call_site_index.shard(class_name);
        if let Ok(guard) = shard.read()
            && let Some(entry) = guard.get(class_name)
            && entry.0 == data.call_sites_revision
        {
            return Some(
                entry
                    .1
                    .get(&(self.shared_name(method_name), is_singleton))
                    .cloned()
                    .unwrap_or_default(),
            );
        }
        let mut grouped: FxHashMap<(SharedName, bool), Vec<u32>> = FxHashMap::default();
        for (idx, site) in data.call_sites.iter().enumerate() {
            grouped
                .entry((site.method_name.clone(), site.method_is_singleton))
                .or_default()
                .push(idx as u32);
        }
        let result = grouped
            .get(&(self.shared_name(method_name), is_singleton))
            .cloned()
            .unwrap_or_default();
        if let Ok(mut guard) = shard.write() {
            guard.insert(
                self.shared_name(class_name),
                std::sync::Arc::new((data.call_sites_revision, grouped)),
            );
        }
        Some(result)
    }

    fn resolve_params(&self, class_name: &str, method: &MethodDef) -> Vec<Param> {
        let Some(_guard) = ResolveDepthGuard::enter() else {
            return Vec::new();
        };
        if PARAM_TABLE_MODE.with(|cell| cell.get()) {
            let table_key = (
                self.shared_name(class_name),
                self.shared_name(method.name.as_str()),
                method.is_singleton,
            );
            note_param_table_read(table_key.clone());
            // table mode: the solver writes in bulk at round end; misses wait for a wake.
            return self
                .resolve_params_cache
                .get(&table_key)
                .unwrap_or_default();
        }
        let _memo_scope = CallerCtxMemoScope::enter();
        let cache_key = (
            self.shared_name(class_name),
            self.shared_name(method.name.as_str()),
            method.is_singleton,
        );
        if let Some(cached) = self.resolve_params_cache.get(&cache_key) {
            return cached;
        }
        let mut visiting = FxHashSet::default();
        let key = (
            class_name.to_string(),
            method.name.to_string(),
            method.is_singleton,
        );
        visiting.insert(key.clone());
        let params = self.resolve_params_inner(class_name, method, &mut visiting);
        visiting.remove(&key);
        // writing to the frozen cache would insert in a scheduling-dependent order, making parallel render nondeterministic.
        if !self
            .resolve_params_cache_frozen
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            // LSP long-lived cache cap: deep `Vec<Param>`s are the dominant driver of unbounded growth.
            self.resolve_params_cache.insert_capped(
                cache_key,
                params.clone(),
                Self::RESOLVE_PARAMS_CACHE_CAP / 16,
            );
        }
        params
    }

    // CLI batch: prewarm the param cache in a pure context, then freeze it (prevents nondeterminism in parallel render).
    pub fn prewarm_and_freeze_resolve_params(&self) {
        use rayon::prelude::*;

        self.resolve_params_cache_frozen
            .store(false, std::sync::atomic::Ordering::Relaxed);
        // cache from an early phase reflects pre-global-resolution state -> recomputing is fresher.
        self.resolve_params_cache.clear();
        // prewarm jobs are granular at (class, method) (avoids one huge class becoming a serial tail as a single task).
        let prewarm_start = std::time::Instant::now();
        // every class gets a table entry (nested resolution may query any method; prevents permanent placeholders).
        let mut jobs: Vec<(&Sym, &MethodDef, usize)> = self
            .class_data
            .iter()
            .flat_map(|(class_name, data)| {
                data.methods
                    .iter()
                    .map(move |method| (class_name, method.as_ref(), data.call_sites.len()))
            })
            .collect();
        // prewarm LPT scheduling: avoids adjacent jobs for the same class being serialized onto one rayon worker (values are unaffected).
        jobs.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| a.0.cmp(b.0))
                .then_with(|| a.1.name.cmp(&b.1.name))
                .then_with(|| a.1.is_singleton.cmp(&b.1.is_singleton))
        });
        let job_timing = std::env::var_os("TYDA_PREWARM_JOB_TIMING").is_some();
        let round_timing = std::env::var_os("TYDA_RESOLUTION_TIMING").is_some();

        // param table Jacobi: evaluate in table mode, write in bulk at round end, re-evaluate only when a read entry changed.
        type SolverResult = (
            usize,
            Option<Vec<Param>>,
            Vec<(SharedName, SharedName, bool)>,
        );
        let mut wake_keys: Vec<Vec<(SharedName, SharedName, bool)>> = vec![Vec::new(); jobs.len()];
        let mut eval_set: Vec<usize> = (0..jobs.len()).collect();
        // cyclic param feedback causes union growth -> the refinement cap converges deterministically, matching legacy behavior.
        const MAX_KEY_REFINEMENTS: u32 = 8;
        let mut refinement_counts: FxHashMap<(SharedName, SharedName, bool), u32> =
            FxHashMap::default();
        let mut frozen_keys: FxHashSet<(SharedName, SharedName, bool)> = FxHashSet::default();
        // defer hub methods (thousands of call sites) until the light tier converges (avoids re-running every round; convergence is unaffected).
        const HUB_CALL_SITE_THRESHOLD: usize = 1000;
        let mut deferred_hubs: FxHashSet<usize> = FxHashSet::default();
        const ROUND_BACKSTOP: usize = 64;
        for round in 0..=ROUND_BACKSTOP {
            if round == ROUND_BACKSTOP {
                eprintln!(
                    "tyda: param fixpoint did not settle after {ROUND_BACKSTOP} rounds; freezing current table"
                );
                break;
            }
            if round > 0 {
                let light: Vec<usize> = eval_set
                    .iter()
                    .copied()
                    .filter(|&job_idx| jobs[job_idx].2 < HUB_CALL_SITE_THRESHOLD)
                    .collect();
                if light.is_empty() {
                    // Only hubs are awake: run them all now.
                    eval_set.extend(deferred_hubs.drain());
                    eval_set.sort_unstable();
                    eval_set.dedup();
                } else {
                    for &job_idx in &eval_set {
                        if jobs[job_idx].2 >= HUB_CALL_SITE_THRESHOLD {
                            deferred_hubs.insert(job_idx);
                        }
                    }
                    eval_set = light;
                }
            }
            if eval_set.is_empty() {
                break;
            }
            let round_start = std::time::Instant::now();
            let results: Vec<SolverResult> = eval_set
                .par_iter()
                .with_max_len(1)
                .map(|&job_idx| {
                    let (class_name, method, _) = jobs[job_idx];
                    let Some(_guard) = ResolveDepthGuard::enter() else {
                        return (job_idx, None, Vec::new());
                    };
                    let job_start = job_timing.then(std::time::Instant::now);
                    let table_scope = ParamTableScope::enter();
                    let _memo_scope = CallerCtxMemoScope::enter();
                    let mut visiting = FxHashSet::default();
                    visiting.insert((
                        class_name.to_string(),
                        method.name.to_string(),
                        method.is_singleton,
                    ));
                    let params = self.resolve_params_inner(class_name, method, &mut visiting);
                    let reads = ParamTableScope::take_reads();
                    drop(table_scope);
                    if let Some(job_start) = job_start {
                        let elapsed = job_start.elapsed();
                        if elapsed.as_millis() >= 100 {
                            eprintln!(
                                "prewarm-job {class_name}#{} ({}): {:.3}s",
                                method.name,
                                if method.is_singleton { "s" } else { "i" },
                                elapsed.as_secs_f64()
                            );
                        }
                    }
                    (job_idx, Some(params), reads)
                })
                .collect();
            let mut changed_keys: FxHashSet<(SharedName, SharedName, bool)> = FxHashSet::default();
            let mut update_count = 0usize;
            for (job_idx, params, reads) in results {
                wake_keys[job_idx] = reads;
                let Some(params) = params else {
                    continue;
                };
                let (class_name, method, _) = jobs[job_idx];
                let key = (
                    self.shared_name(class_name),
                    self.shared_name(method.name.as_str()),
                    method.is_singleton,
                );
                if frozen_keys.contains(&key) {
                    continue;
                }
                let previous = self.resolve_params_cache.get(&key);
                if previous.as_deref() != Some(params.as_slice()) {
                    self.resolve_params_cache
                        .insert_uncapped(key.clone(), params);
                    let count = refinement_counts.entry(key.clone()).or_insert(0);
                    *count += 1;
                    if *count >= MAX_KEY_REFINEMENTS {
                        frozen_keys.insert(key.clone());
                    }
                    changed_keys.insert(key);
                    update_count += 1;
                }
            }
            if round_timing {
                eprintln!(
                    "param-fixpoint round {round}: evaluated={} updates={update_count} {:.3}s",
                    eval_set.len(),
                    round_start.elapsed().as_secs_f64()
                );
            }
            if changed_keys.is_empty() && deferred_hubs.is_empty() {
                break;
            }
            eval_set = (0..jobs.len())
                .filter(|&job_idx| {
                    let (class_name, method, _) = jobs[job_idx];
                    let own_key = (
                        self.shared_name(class_name),
                        self.shared_name(method.name.as_str()),
                        method.is_singleton,
                    );
                    if frozen_keys.contains(&own_key) {
                        return false;
                    }
                    wake_keys[job_idx]
                        .iter()
                        .any(|key| changed_keys.contains(key))
                })
                .collect();
        }
        self.resolve_params_cache_frozen
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if round_timing {
            eprintln!(
                "param-prewarm: {} entries {:.3}s",
                jobs.len(),
                prewarm_start.elapsed().as_secs_f64()
            );
        }
    }

    fn resolve_params_inner(
        &self,
        class_name: &str,
        method: &MethodDef,
        visiting: &mut FxHashSet<(String, String, bool)>,
    ) -> Vec<Param> {
        if method.param_infos.is_empty() {
            return Vec::new();
        }

        let Some(data) = self.class_data.get(class_name) else {
            return method
                .param_infos
                .iter()
                .map(|pi| Param {
                    name: pi.name.clone(),
                    param_type: pi.default_type.clone().unwrap_or(Type::Untyped),
                    kind: pi.kind,
                })
                .collect();
        };

        if method.has_annotation() {
            let param_names = method.effective_param_names();
            return method
                .param_infos
                .iter()
                .enumerate()
                .map(|(i, param_info)| {
                    let mut ty = data
                        .cold()
                        .annotated_params
                        .get(&(method.name, method.is_singleton))
                        .and_then(|params| params.get(&i))
                        .cloned()
                        .unwrap_or(Type::Untyped);
                    if ty == Type::Untyped
                        && let Some(default_ty) = method
                            .param_infos
                            .get(i)
                            .and_then(|pi| pi.default_type.clone())
                    {
                        // Struct/Data synthesized `initialize`/`with`: resolve `default_type`'s ivar deferred ref symmetrically to the reader.
                        ty = if Self::type_is_deferred_ref(&default_ty) {
                            let resolved = self.resolve_deferred_refs_for_context(
                                class_name,
                                method.is_singleton,
                                &default_ty,
                            );
                            if Self::is_concrete_for_global_resolve(&resolved) {
                                resolved
                            } else {
                                Type::Untyped
                            }
                        } else {
                            default_ty
                        };
                    }
                    let kind = method
                        .param_infos
                        .get(i)
                        .map(|pi| pi.kind)
                        .unwrap_or(ParamKind::Required);
                    Param {
                        name: param_names
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| param_info.name.clone()),
                        param_type: ty,
                        kind,
                    }
                })
                .collect();
        }

        let positional_count = Self::positional_param_count(&method.param_infos);
        let mut param_types: Vec<Vec<Type>> = vec![Vec::new(); positional_count];
        let indexed = self.matching_call_site_indices(
            class_name,
            data,
            method.name.as_str(),
            method.is_singleton,
        );
        let site_iter: Box<dyn Iterator<Item = &CallSite>> = match &indexed {
            Some(indices) => Box::new(indices.iter().map(|&idx| data.call_sites.get(idx as usize))),
            None => Box::new(data.call_sites.iter()),
        };
        for call_site in site_iter {
            if call_site.method_name.as_ref() == method.name.as_str()
                && call_site.method_is_singleton == method.is_singleton
            {
                // skip resolving remaining call sites once the union cap saturates (avoids wasted CPU after a hub method is already pinned to untyped by the cap).
                if positional_count > 0
                    && param_types
                        .iter()
                        .all(|types| Type::union_parts_saturated(types))
                {
                    break;
                }
                // skip self-referential call sites (re-entering params into the same method would recurse infinitely, e.g. with duplicate `def self.`/`class_methods`).
                if let Some(ctx) = call_site.caller_context.as_ref()
                    && ctx.class_name.as_ref() == class_name
                    && ctx.method_name.as_ref() == method.name.as_str()
                    && ctx.method_is_singleton == method.is_singleton
                {
                    continue;
                }
                let resolved_arg_types =
                    Self::synthesize_keyword_hash_arg(call_site, &method.param_infos)
                        .into_iter()
                        .map(|ty| {
                            self.resolve_call_site_type_from_caller_context(
                                call_site, &ty, visiting,
                            )
                        })
                        .collect::<Vec<_>>();
                Self::merge_resolved_positional_arg_types(
                    &mut param_types,
                    &resolved_arg_types,
                    &method.param_infos,
                );
            }
        }

        let mut positional_idx = 0;
        let mut resolved_positional_types = Vec::new();
        method
            .param_infos
            .iter()
            .map(|pi| {
                let passthrough_type = (method.name == "initialize" && !method.is_singleton)
                    .then(|| self.resolve_initialize_param_passthrough_type(class_name, &pi.name))
                    .flatten();
                let resolved_default_type = pi
                    .default_type
                    .as_ref()
                    .map(|ty| Self::substitute_param_refs_static(ty, &resolved_positional_types));
                let (ty, kind) = match pi.kind {
                    ParamKind::Required => {
                        let mut ty = if positional_idx < param_types.len()
                            && !param_types[positional_idx].is_empty()
                        {
                            Type::merge_param_arg_vec(param_types[positional_idx].clone())
                                .widen_arg_for_param()
                        } else {
                            Type::Untyped
                        };
                        if ty == Type::Untyped
                            && let Some(ref passthrough_ty) = passthrough_type
                        {
                            ty = passthrough_ty.clone();
                        }
                        if ty == Type::Untyped
                            && let Some(ref default_ty) = pi.default_type
                        {
                            ty = default_ty.clone();
                        }
                        positional_idx += 1;
                        (ty, ParamKind::Required)
                    }
                    ParamKind::Optional => {
                        let mut ty = if positional_idx < param_types.len()
                            && !param_types[positional_idx].is_empty()
                        {
                            Type::merge_param_arg_vec(param_types[positional_idx].clone())
                                .widen_arg_for_param()
                        } else {
                            Type::Untyped
                        };
                        positional_idx += 1;
                        if ty == Type::Untyped
                            && let Some(ref passthrough_ty) = passthrough_type
                        {
                            ty = passthrough_ty.clone();
                        }
                        if let Some(ref default_ty) = resolved_default_type {
                            let default_ty = default_ty.clone().widen_arg_for_param();
                            if ty == Type::Untyped {
                                ty = default_ty;
                            } else {
                                ty = ty.union_with(default_ty);
                            }
                        }
                        (ty, ParamKind::Optional)
                    }
                    ParamKind::Rest => {
                        let ty = if positional_idx < param_types.len()
                            && !param_types[positional_idx].is_empty()
                        {
                            Type::merge_param_arg_vec(param_types[positional_idx].clone())
                                .widen_arg_for_param()
                        } else {
                            passthrough_type.clone().unwrap_or(Type::Untyped)
                        };
                        positional_idx += 1;
                        (ty, ParamKind::Rest)
                    }
                    ParamKind::KeywordRequired => {
                        let mut ty = self.resolve_keyword_param_type(
                            class_name,
                            data,
                            &method.name,
                            method.is_singleton,
                            &pi.name,
                            visiting,
                        );
                        if ty == Type::Untyped
                            && let Some(ref passthrough_ty) = passthrough_type
                        {
                            ty = passthrough_ty.clone();
                        }
                        (ty, ParamKind::KeywordRequired)
                    }
                    ParamKind::KeywordOptional => {
                        let call_ty = self.resolve_keyword_param_type(
                            class_name,
                            data,
                            &method.name,
                            method.is_singleton,
                            &pi.name,
                            visiting,
                        );
                        let ty = if call_ty == Type::Untyped {
                            let passthrough_ty = passthrough_type.clone().unwrap_or(Type::Untyped);
                            if passthrough_ty == Type::Untyped {
                                resolved_default_type.clone().unwrap_or(Type::Untyped)
                            } else if let Some(ref default_ty) = resolved_default_type {
                                passthrough_ty.union_with(default_ty.clone())
                            } else {
                                passthrough_ty
                            }
                        } else if let Some(ref default_ty) = resolved_default_type {
                            call_ty.union_with(default_ty.clone())
                        } else {
                            call_ty
                        };
                        (ty, ParamKind::KeywordOptional)
                    }
                    ParamKind::DoubleRest => {
                        let named: HashSet<&str> = method
                            .param_infos
                            .iter()
                            .filter(|p| {
                                matches!(
                                    p.kind,
                                    ParamKind::KeywordRequired | ParamKind::KeywordOptional
                                )
                            })
                            .map(|p| p.name.as_str())
                            .collect();
                        let mut values: Vec<Type> = Vec::new();
                        for site in data.call_sites.iter().filter(|site| {
                            site.method_name.as_ref() == method.name.as_str()
                                && site.method_is_singleton == method.is_singleton
                        }) {
                            for (name, ty) in &site.keyword_arg_types {
                                if named.contains(name.as_ref()) {
                                    continue;
                                }
                                values.push(ty.clone());
                            }
                        }
                        let ty = if values.is_empty() {
                            Type::Untyped
                        } else {
                            Type::from_type_vec(values).widen_arg_for_param()
                        };
                        (ty, ParamKind::DoubleRest)
                    }
                    ParamKind::Block => (Type::Untyped, ParamKind::Block),
                };
                if matches!(
                    kind,
                    ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                ) {
                    let resolved_ty = self.resolve_param_signature_type(
                        class_name,
                        &ty,
                        resolved_default_type.is_some(),
                    );
                    resolved_positional_types.push(resolved_ty.clone());
                    return Param {
                        name: pi.name.clone(),
                        param_type: resolved_ty,
                        kind,
                    };
                }
                Param {
                    name: pi.name.clone(),
                    param_type: self.resolve_param_signature_type(
                        class_name,
                        &ty,
                        resolved_default_type.is_some(),
                    ),
                    kind,
                }
            })
            .collect()
    }

    fn resolve_param_signature_type(
        &self,
        class_name: &str,
        ty: &Type,
        force_top_level: bool,
    ) -> Type {
        if force_top_level {
            return self.resolve_deferred_refs(class_name, ty);
        }
        ty.clone()
    }

    fn resolve_keyword_param_type(
        &self,
        class_name: &str,
        data: &ClassData,
        method_name: &str,
        method_is_singleton: bool,
        kw_name: &str,
        visiting: &mut FxHashSet<(String, String, bool)>,
    ) -> Type {
        let mut types = Vec::new();
        let indexed =
            self.matching_call_site_indices(class_name, data, method_name, method_is_singleton);
        let site_iter: Box<dyn Iterator<Item = &CallSite>> = match &indexed {
            Some(indices) => Box::new(indices.iter().map(|&idx| data.call_sites.get(idx as usize))),
            None => Box::new(data.call_sites.iter()),
        };
        for call_site in site_iter {
            if call_site.method_name.as_ref() == method_name
                && call_site.method_is_singleton == method_is_singleton
                && let Some(ty) = call_site.keyword_arg_types.get(kw_name)
            {
                let resolved =
                    self.resolve_call_site_type_from_caller_context(call_site, ty, visiting);
                Type::append_union_parts(&mut types, resolved);
            }
        }
        if types.is_empty() {
            Type::Untyped
        } else {
            Type::from_type_vec(types)
        }
    }

    pub fn set_class_type_params(&mut self, class_name: &str, params: Vec<String>) {
        self.class_data_mut(class_name).cold_mut().class_type_params = params;
    }

    pub fn get_class_type_params(&self, class_name: &str) -> &[String] {
        self.class_data
            .get(class_name)
            .map(|d| d.cold().class_type_params.as_slice())
            .unwrap_or(&[])
    }

    pub fn set_class_type_param_bounds(
        &mut self,
        class_name: &str,
        bounds: Vec<(String, rbs_ir::RbsType)>,
    ) {
        self.class_data_mut(class_name)
            .cold_mut()
            .class_type_param_bounds = bounds;
    }

    pub fn get_class_type_param_bounds(&self, class_name: &str) -> &[(String, rbs_ir::RbsType)] {
        self.class_data
            .get(class_name)
            .map(|d| d.cold().class_type_param_bounds.as_slice())
            .unwrap_or(&[])
    }

    pub fn set_class_type_param_defaults(
        &mut self,
        class_name: &str,
        defaults: Vec<(String, Type)>,
    ) {
        self.class_data_mut(class_name)
            .cold_mut()
            .class_type_param_defaults = defaults;
    }

    pub fn get_class_type_param_defaults(&self, class_name: &str) -> &[(String, Type)] {
        self.class_data
            .get(class_name)
            .map(|d| d.cold().class_type_param_defaults.as_slice())
            .unwrap_or(&[])
    }

    pub fn add_class_type_param(&mut self, class_name: &str, param_name: String) {
        let cold = self.class_data_mut(class_name).cold_mut();
        if !cold.class_type_params.contains(&param_name) {
            cold.class_type_params.push(param_name);
        }
    }

    pub fn lookup_rbs_method_types(
        &self,
        class_name: &str,
        method_name: &str,
    ) -> &[rbs_ir::MethodType] {
        self.lookup_rbs_method_types_with_hint(class_name, method_name, false)
    }
    pub fn lookup_rbs_method_types_with_hint(
        &self,
        class_name: &str,
        method_name: &str,
        prefer_singleton: bool,
    ) -> &[rbs_ir::MethodType] {
        let order = if prefer_singleton {
            [true, false]
        } else {
            [false, true]
        };
        // RBS overload pass 1: only match the caller's singleton intent (prevents the opposite overload from hijacking instance/singleton).
        if let Some((owner, _)) =
            self.resolve_first_method_call_owner_ref(class_name, method_name, order[0])
            && let Some(data) = self.class_data.get(owner)
            && let Some(m) = Self::method_for_lookup_kind(data, method_name, Some(order[0]))
            && !m.rbs_method_types.is_empty()
        {
            return &m.rbs_method_types;
        }
        // RBS pass 2: don't fall through to the opposite side if the primary already resolved an owner (including an empty RBS).
        if self
            .resolve_first_method_call_owner_ref(class_name, method_name, order[0])
            .is_none()
            && let Some((owner, _)) =
                self.resolve_first_method_call_owner_ref(class_name, method_name, order[1])
            && let Some(data) = self.class_data.get(owner)
            && let Some(m) = Self::method_for_lookup_kind(data, method_name, Some(order[1]))
            && !m.rbs_method_types.is_empty()
        {
            return &m.rbs_method_types;
        }
        &[]
    }

    pub fn set_class_location(&mut self, class_name: &str, loc: SourceLocation) {
        let data = self.class_data_mut(class_name);
        if data.loc.is_none() {
            data.loc = Some(loc);
        }
    }

    pub fn set_file_path(&mut self, class_name: &str, file_path: &str) {
        self.file_contribution_names.insert(class_name.to_string());
        let data = self.class_data_mut(class_name);
        if data.file_path.is_none() {
            data.file_path = Some(SharedPath::from(file_path));
        }
    }

    pub(crate) fn file_contribution_names(&self) -> &HashSet<String> {
        &self.file_contribution_names
    }

    pub(crate) fn file_contribution_method_names(&self) -> &HashSet<Sym> {
        &self.file_contribution_method_names
    }

    pub fn set_superclass(&mut self, class_name: &str, superclass: &str) {
        let superclass = self.intern_name(superclass.strip_prefix("::").unwrap_or(superclass));
        let data = self.class_data_mut(class_name);
        data.superclass = Some(superclass);
        data.user_defined = true;
    }

    pub fn set_superclass_type_args(&mut self, class_name: &str, type_args: Vec<rbs_ir::RbsType>) {
        self.class_data_mut(class_name)
            .cold_mut()
            .superclass_type_args = type_args;
    }

    pub fn get_superclass_type_args(&self, class_name: &str) -> &[rbs_ir::RbsType] {
        self.class_data
            .get(class_name)
            .map(|data| data.cold().superclass_type_args.as_slice())
            .unwrap_or(&[])
    }

    pub fn set_is_module(&mut self, class_name: &str, is_module: bool) {
        let data = self.class_data_mut(class_name);
        data.is_module = is_module;
        data.user_defined = true;
    }

    /// record a method's visibility override. Removed from the map for `Public` since it's the default
    /// (covers the case of reverting from non-public via a `public` section / `public :sym`).
    pub fn set_method_visibility(
        &mut self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
        visibility: Option<Visibility>,
    ) {
        let key = (Sym::new(method_name), is_singleton);
        let cold = self.class_data_mut(class_name).cold_mut();
        match visibility {
            Some(visibility) => {
                cold.method_visibility.insert(key, visibility);
            }
            None => {
                cold.method_visibility.remove(&key);
            }
        }
    }

    /// record a hand-written bare ivar reader (`def x = @x` / `def x; @x; end`) into the cold set.
    /// called during collection (def walk); feeds the pure-reader check for self-fact narrowing.
    pub fn mark_bare_ivar_reader(
        &mut self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) {
        let key = (Sym::new(method_name), is_singleton);
        self.class_data_mut(class_name)
            .cold_mut()
            .bare_ivar_readers
            .insert(key);
    }

    /// look up a method's visibility override. Treated as `Public` (returns `None`) if there's no entry.
    pub fn method_visibility(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<Visibility> {
        self.class_data
            .get(class_name)
            .and_then(|data| {
                data.cold()
                    .method_visibility
                    .get(&(Sym::new(method_name), is_singleton))
            })
            .copied()
    }

    pub fn add_mixin(&mut self, class_name: &str, module_name: &str, kind: MixinKind) -> bool {
        self.add_mixin_with_source(class_name, module_name, kind, Vec::new(), false)
    }

    pub fn add_external_mixin(
        &mut self,
        class_name: &str,
        module_name: &str,
        kind: MixinKind,
    ) -> bool {
        self.add_mixin_with_source(class_name, module_name, kind, Vec::new(), true)
    }

    pub fn add_external_mixin_with_type_args(
        &mut self,
        class_name: &str,
        module_name: &str,
        kind: MixinKind,
        type_args: Vec<rbs_ir::RbsType>,
    ) -> bool {
        self.add_mixin_with_source(class_name, module_name, kind, type_args, true)
    }

    fn add_mixin_with_source(
        &mut self,
        class_name: &str,
        module_name: &str,
        kind: MixinKind,
        type_args: Vec<rbs_ir::RbsType>,
        external_source: bool,
    ) -> bool {
        let module_name = self.intern_name(module_name);
        let data = self.class_data_mut(class_name);
        if let Some(existing) = data
            .mixins
            .iter_mut()
            .find(|m| m.module_name == module_name && m.kind == kind)
        {
            if existing.type_args.is_empty() && !type_args.is_empty() {
                existing.type_args = type_args;
                self.invalidate_reverse_indexes();
                self.mixin_hook_mixins_applied = false;
                self.includer_bound_dsl_applied = false;
                return true;
            }
            if !external_source && existing.external_source {
                let module_name = existing.module_name.clone();
                data.mixins.retain(|m| {
                    !(m.module_name == module_name && m.kind == kind && m.external_source)
                });
                data.mixins.push(Mixin {
                    module_name,
                    type_args,
                    kind,
                    external_source,
                });
                self.invalidate_reverse_indexes();
                self.mixin_hook_mixins_applied = false;
                self.includer_bound_dsl_applied = false;
                return true;
            }
        } else {
            data.mixins.push(Mixin {
                module_name,
                type_args,
                kind,
                external_source,
            });
            self.invalidate_reverse_indexes();
            self.mixin_hook_mixins_applied = false;
            self.includer_bound_dsl_applied = false;
            return true;
        }
        false
    }

    pub fn add_mixin_hook_mixin(
        &mut self,
        module_name: &str,
        hook_kind: MixinKind,
        target_module_name: &str,
        kind: MixinKind,
    ) {
        let target_module_name = self.intern_name(target_module_name);
        let data = self.class_data_mut(module_name);
        let hook_mixins = data.hook_mixins_mut().by_kind_mut(&hook_kind);
        if hook_mixins
            .iter()
            .any(|existing| existing.module_name == target_module_name && existing.kind == kind)
        {
            return;
        }
        hook_mixins.push(Mixin {
            module_name: target_module_name,
            type_args: Vec::new(),
            kind,
            external_source: false,
        });
        self.has_mixin_hook_mixins = true;
        self.invalidate_reverse_indexes();
        self.mixin_hook_mixins_applied = false;
        self.includer_bound_dsl_applied = false;
    }

    fn resolve_mixin_hook_mixin_target(&self, hook_owner: &str, raw_name: &str) -> String {
        raw_name
            .strip_prefix("::")
            .map(ToString::to_string)
            .unwrap_or_else(|| self.resolve_scoped_class_ref(hook_owner, raw_name))
    }

    fn is_mixin_hook_method(method_name: &str, is_singleton: bool) -> bool {
        is_singleton && matches!(method_name, "included" | "extended" | "prepended")
    }

    fn method_needs_mixin_hook_call_site(method: &MethodDef) -> bool {
        Self::is_mixin_hook_method(method.name.as_str(), method.is_singleton)
            && !method.has_annotation()
            && !method.is_external_rbs_source()
            && !method.synthetic_dsl_source
            && method.param_infos.iter().any(|param| {
                matches!(
                    param.kind,
                    ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                )
            })
    }

    fn refresh_mixin_hook_method_flag(&mut self) {
        self.has_mixin_hook_methods = self.class_data.values().any(|data| {
            data.methods
                .iter()
                .any(|method| Self::method_needs_mixin_hook_call_site(method))
        });
    }

    fn add_mixin_hook_call_sites(&mut self) {
        if !self.has_mixin_hook_methods {
            return;
        }

        let mut callbacks: Vec<(String, &'static str, Sym)> = Vec::new();
        for (target_class, data) in &self.class_data {
            for mixin in &data.mixins {
                let hook_owner = self
                    .resolve_scoped_class_ref(target_class.as_str(), mixin.module_name.as_ref());
                let Some(hook_owner_data) = self.class_data.get(hook_owner.as_str()) else {
                    continue;
                };
                if !hook_owner_data.is_module {
                    continue;
                }
                let hook_method_name = mixin.kind.hook_method_name();
                let Some(hook_method_idx) = hook_owner_data
                    .method_index
                    .get(hook_method_name)
                    .and_then(|slots| slots.singleton)
                else {
                    continue;
                };
                let Some(hook_method) = hook_owner_data.methods.get(hook_method_idx) else {
                    continue;
                };
                if !Self::method_needs_mixin_hook_call_site(hook_method) {
                    continue;
                }
                callbacks.push((hook_owner, hook_method_name, *target_class));
            }
        }
        callbacks.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(left.1.cmp(right.1))
                .then(left.2.cmp(&right.2))
        });

        let generated_callback_count = callbacks.len();
        for (hook_owner, hook_method_name, target_class) in callbacks {
            self.add_call_site(
                &hook_owner,
                CallSite {
                    method_name: hook_method_name.into(),
                    method_is_singleton: true,
                    arg_types: vec![Type::Singleton(target_class)],
                    keyword_arg_types: KeywordArgTypes::new(),
                    block: None,
                    caller_context: None,
                },
            );
        }
        if generated_callback_count > 0 {
            self.invalidate_resolve_cache();
        }
    }

    pub fn apply_mixin_hook_mixins(&mut self) {
        if !self.mixin_hook_mixins_applied {
            if self.has_mixin_hook_mixins {
                for _ in 0..8 {
                    let mut additions: Vec<(String, String, MixinKind)> = Vec::new();
                    for (class_name, data) in &self.class_data {
                        for mixin in &data.mixins {
                            let hook_owner = self
                                .resolve_scoped_class_ref(class_name, mixin.module_name.as_ref());
                            let Some(hook_owner_data) = self.class_data.get(hook_owner.as_str())
                            else {
                                continue;
                            };
                            let hook_mixins = hook_owner_data.hook_mixins_by_kind(&mixin.kind);
                            for hook_mixin in hook_mixins {
                                let target = self.resolve_mixin_hook_mixin_target(
                                    &hook_owner,
                                    hook_mixin.module_name.as_ref(),
                                );
                                additions.push((
                                    class_name.to_string(),
                                    target,
                                    hook_mixin.kind.clone(),
                                ));
                            }
                        }
                    }
                    let mut changed = false;
                    for (class_name, target, kind) in additions {
                        changed |= self.add_mixin(&class_name, &target, kind);
                    }
                    if !changed {
                        break;
                    }
                }
            }
            self.add_mixin_hook_call_sites();
            self.mixin_hook_mixins_applied = true;
        }
        self.apply_includer_bound_dsl();
    }

    pub fn add_required_ancestor(&mut self, class_name: &str, ancestor_name: &str) {
        self.add_required_ancestor_with_type_args(class_name, ancestor_name, Vec::new());
    }

    pub fn add_required_ancestor_with_type_args(
        &mut self,
        class_name: &str,
        ancestor_name: &str,
        type_args: Vec<rbs_ir::RbsType>,
    ) {
        let ancestor_name =
            self.intern_name(ancestor_name.strip_prefix("::").unwrap_or(ancestor_name));
        let cold = self.class_data_mut(class_name).cold_mut();
        if !cold
            .required_ancestors
            .iter()
            .any(|existing| existing == &ancestor_name)
        {
            cold.required_ancestors.push(ancestor_name.clone());
        }
        if !type_args.is_empty() {
            if let Some((_, existing_args)) = cold
                .required_ancestor_type_args
                .iter_mut()
                .find(|(name, _)| name == &ancestor_name)
            {
                if existing_args.is_empty() {
                    *existing_args = type_args;
                }
            } else {
                cold.required_ancestor_type_args
                    .push((ancestor_name, type_args));
            }
        }
    }

    pub fn set_class_modifier_comments(&mut self, class_name: &str, comments: Vec<String>) {
        if comments.is_empty() {
            return;
        }
        let cold = self.class_data_mut(class_name).cold_mut();
        for comment in comments {
            if !cold.sorbet_modifier_comments.contains(&comment) {
                cold.sorbet_modifier_comments.push(comment);
            }
        }
    }

    pub fn class_data_for(&self, class_name: &str) -> Option<&ClassData> {
        self.class_data.get(class_name).map(|data| &**data)
    }

    pub fn has_class(&self, class_name: &str) -> bool {
        self.class_data.contains_key(class_name)
    }

    fn is_concern_module(&self, module_name: &str) -> bool {
        let module_name = module_name.trim_scope_prefix();
        if let Some(data) = self.class_data.get(module_name) {
            data.mixins.iter().any(|m| {
                m.kind == MixinKind::Extend && m.module_name.as_ref() == "ActiveSupport::Concern"
            })
        } else {
            false
        }
    }

    /// a concern's `ClassMethods` module name (resolved as class methods on the including class's singleton).
    fn concern_class_methods_owner(&self, module_name: &str) -> Option<&str> {
        let candidate = crate::sym::join_scope(module_name.trim_scope_prefix(), "ClassMethods");
        self.class_data
            .get_key_value(candidate.as_str())
            .map(|(key, _)| key.as_str())
    }

    fn class_data_mut(&mut self, class_name: &str) -> &mut ClassData {
        self.intern_name(class_name);
        self.class_data.entry(Sym::new(class_name)).or_default()
    }

    /// At most one owner: every arm either propagates a nested result or
    /// returns the class it matched in, so the walk never accumulates.
    fn resolve_method_call_owners_inner_refs<'a>(
        &'a self,
        class_name: &'a str,
        method_name: &str,
        method_is_singleton: bool,
        seen: &mut Vec<(&'a str, bool)>,
    ) -> Option<(&'a str, bool)> {
        if seen.contains(&(class_name, method_is_singleton)) {
            return None;
        }
        seen.push((class_name, method_is_singleton));

        let data = self.class_data.get(class_name)?;

        // Ruby applies the last mixin first during instance method lookup.
        if !method_is_singleton {
            for mixin in data.mixins.iter().rev() {
                if mixin.kind != MixinKind::Prepend {
                    continue;
                }
                let mixin_ref =
                    self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
                if let Some(owners) =
                    self.resolve_method_call_owners_inner_refs(mixin_ref, method_name, false, seen)
                {
                    return Some(owners);
                }
            }
        }

        if Self::method_is_undefined_for_lookup(data, method_name, method_is_singleton) {
            return None;
        }

        if Self::method_for_lookup_kind(data, method_name, Some(method_is_singleton)).is_some() {
            return Some((class_name, method_is_singleton));
        }

        if method_is_singleton {
            // `extend` and Concern class methods follow the same reverse
            // application order as the singleton ancestor chain.
            for mixin in data.mixins.iter().rev() {
                let mixin_ref =
                    self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
                if mixin.kind == MixinKind::Extend {
                    if let Some(owners) = self.resolve_method_call_owners_inner_refs(
                        mixin_ref,
                        method_name,
                        false,
                        seen,
                    ) {
                        return Some(owners);
                    }
                } else {
                    // a Concern's `class_methods do` is collected as the module's singleton -> chained into the singleton on include.
                    if self.is_concern_module(mixin_ref)
                        && let Some(owners) = self.resolve_method_call_owners_inner_refs(
                            mixin_ref,
                            method_name,
                            true,
                            seen,
                        )
                    {
                        return Some(owners);
                    }
                    // `M::ClassMethods` instance method -> includer class method (the `self.included` convention, independent of Concern).
                    if let Some(class_methods) = self.concern_class_methods_owner(mixin_ref)
                        && let Some(owners) = self.resolve_method_call_owners_inner_refs(
                            class_methods,
                            method_name,
                            false,
                            seen,
                        )
                    {
                        return Some(owners);
                    }
                }
            }
            if let Some(superclass) = &data.superclass {
                let super_ref =
                    self.resolve_scoped_class_ref_borrow(class_name, superclass.as_ref());
                if let Some(owners) =
                    self.resolve_method_call_owners_inner_refs(super_ref, method_name, true, seen)
                {
                    return Some(owners);
                }
            }
            // end of the singleton chain: instance methods of Class/Module/Object become singleton methods (e.g. `new`/`send`).
            for fallback in ["Class", "Module", "Object"] {
                if class_name == fallback {
                    continue;
                }
                if let Some(owners) =
                    self.resolve_method_call_owners_inner_refs(fallback, method_name, false, seen)
                {
                    return Some(owners);
                }
            }
        } else {
            for mixin in data.mixins.iter().rev() {
                // Prepend was already walked before checking the class
                // itself; Extend lives on the singleton side.
                if mixin.kind != MixinKind::Include {
                    continue;
                }
                let mixin_ref =
                    self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
                if let Some(owners) =
                    self.resolve_method_call_owners_inner_refs(mixin_ref, method_name, false, seen)
                {
                    return Some(owners);
                }
            }

            if let Some(superclass) = &data.superclass {
                let super_ref =
                    self.resolve_scoped_class_ref_borrow(class_name, superclass.as_ref());
                if let Some(owners) =
                    self.resolve_method_call_owners_inner_refs(super_ref, method_name, false, seen)
                {
                    return Some(owners);
                }
            }

            for ancestor in &data.cold().required_ancestors {
                if let Some(owners) = self.resolve_method_call_owners_inner_refs(
                    ancestor.as_ref(),
                    method_name,
                    false,
                    seen,
                ) {
                    return Some(owners);
                }
            }

            if class_name != "Object"
                && let Some(owners) =
                    self.resolve_method_call_owners_inner_refs("Object", method_name, false, seen)
            {
                return Some(owners);
            }

            for fallback in ["Kernel", "Comparable", "BasicObject"] {
                if let Some(owners) =
                    self.resolve_method_call_owners_inner_refs(fallback, method_name, false, seen)
                {
                    return Some(owners);
                }
            }
        }

        None
    }
    pub fn ancestor_knowledge_complete(&self, class_name: &str) -> bool {
        let mut visited: Vec<String> = Vec::new();
        let mut touched_framework_base = false;
        self.ancestor_knowledge_complete_inner(
            class_name.trim_scope_prefix(),
            &mut visited,
            &mut touched_framework_base,
        )
    }

    // determines method-surface completeness: dynamic surfaces like AR and `method_missing` can't be proven absent.
    pub fn method_surface_knowledge_complete(
        &self,
        class_name: &str,
        generated_artifacts_present: bool,
    ) -> bool {
        let mut visited: Vec<String> = Vec::new();
        let mut touched_framework_base = false;
        if !self.ancestor_knowledge_complete_inner(
            class_name.trim_scope_prefix(),
            &mut visited,
            &mut touched_framework_base,
        ) {
            return false;
        }
        if self.walked_closure_defines_method_missing(&visited) {
            return false;
        }
        if !touched_framework_base {
            return true;
        }
        !generated_artifacts_present || self.walked_closure_has_declaration_backed_methods(&visited)
    }

    fn walked_closure_defines_method_missing(&self, classes: &[String]) -> bool {
        classes
            .iter()
            .filter_map(|name| self.class_data.get(name.as_str()))
            .any(|data| data.method_index.contains_key("method_missing"))
    }

    fn walked_closure_has_declaration_backed_methods(&self, classes: &[String]) -> bool {
        classes
            .iter()
            .filter_map(|name| self.class_data.get(name.as_str()))
            .any(|data| {
                data.method_file_paths
                    .values()
                    .any(|path| path.ends_with(".rbi"))
                    || data
                        .methods
                        .iter()
                        .any(|method| method.rbs_file_source && !method.synthetic_dsl_source)
            })
    }

    fn ancestor_knowledge_complete_inner(
        &self,
        class_name: &str,
        visited: &mut Vec<String>,
        touched_framework_base: &mut bool,
    ) -> bool {
        // the chain is complete at the universal root / modeled framework base (method-surface completeness is left to the caller).
        if Self::is_universal_ancestor_root(class_name) {
            return true;
        }
        if Self::is_modeled_framework_base(class_name) {
            *touched_framework_base = true;
            return true;
        }
        if visited.iter().any(|seen| seen == class_name) {
            // Cyclic edge (`class A < A`): treat as terminated rather than
            // unknown so a genuine cycle does not blanket-mute diagnostics.
            return true;
        }
        visited.push(class_name.to_string());

        let Some(data) = self.class_data.get(class_name) else {
            // Referenced but never declared -> unknowable surface.
            return false;
        };
        // An empty speculative stub (no loc / methods / superclass / mixins)
        // carries no method surface; a genuine empty declaration keeps a loc.
        if !data.has_type_substance() {
            return false;
        }

        if let Some(superclass) = &data.superclass {
            let super_ref = self.resolve_scoped_class_ref_borrow(class_name, superclass.as_ref());
            let super_ref = super_ref.trim_scope_prefix().to_string();
            if !self.ancestor_knowledge_complete_inner(&super_ref, visited, touched_framework_base)
            {
                return false;
            }
        }

        for mixin in &data.mixins {
            if !matches!(mixin.kind, MixinKind::Include | MixinKind::Prepend) {
                continue;
            }
            let mixin_ref =
                self.resolve_scoped_class_ref_borrow(class_name, mixin.module_name.as_ref());
            let mixin_ref = mixin_ref.trim_scope_prefix().to_string();
            if !self.ancestor_knowledge_complete_inner(&mixin_ref, visited, touched_framework_base)
            {
                return false;
            }
        }

        for ancestor in &data.cold().required_ancestors {
            let ancestor_ref = ancestor.trim_scope_prefix().to_string();
            if !self.ancestor_knowledge_complete_inner(
                &ancestor_ref,
                visited,
                touched_framework_base,
            ) {
                return false;
            }
        }

        true
    }

    /// method surface is complete once a known stdlib ancestor root is reached (even in an isolated snapshot).
    fn is_universal_ancestor_root(name: &str) -> bool {
        matches!(
            name,
            "Object"
                | "BasicObject"
                | "Kernel"
                | "Comparable"
                | "Class"
                | "Module"
                | "Struct"
                | "Data"
        )
    }

    pub(crate) fn is_modeled_framework_base(name: &str) -> bool {
        let bare = name.trim_scope_prefix();
        // Rails convention base classes: even absent from an isolated snapshot, subclasses are usually AR/controller/mailer.
        if matches!(
            bare,
            "ApplicationRecord"
                | "ApplicationController"
                | "ApplicationMailer"
                | "ApplicationJob"
                | "ApplicationCable::Channel"
                | "ApplicationCable::Connection"
        ) {
            return true;
        }
        let root = bare.split("::").next().unwrap_or(bare);
        matches!(
            root,
            "ActiveRecord"
                | "ActionController"
                | "ActionMailer"
                | "ActionView"
                | "ActiveJob"
                | "ActionCable"
                | "ActiveStorage"
                | "ActiveModel"
                | "AbstractController"
                | "ActionDispatch"
                | "ActionText"
        )
    }

    fn type_contains_param_ref(ty: &Type) -> bool {
        match ty {
            Type::ParamRef(_) | Type::KeywordParamRef(_) => true,
            Type::Union(parts) | Type::Intersection(parts) => {
                parts.iter().any(Self::type_contains_param_ref)
            }
            Type::Array(Some(inner)) => Self::type_contains_param_ref(inner),
            Type::Hash(Some(key), Some(value)) => {
                Self::type_contains_param_ref(key) || Self::type_contains_param_ref(value)
            }
            Type::Hash(Some(key), None) => Self::type_contains_param_ref(key),
            Type::Hash(None, Some(value)) => Self::type_contains_param_ref(value),
            Type::Tuple(elems) => elems.iter().any(Self::type_contains_param_ref),
            Type::Record(fields) => fields
                .iter()
                .any(|field| Self::type_contains_param_ref(&field.value)),
            Type::PatternIndexRef(subject, _)
            | Type::PatternRestRef(subject)
            | Type::PatternTrailingRef(subject, _) => Self::type_contains_param_ref(subject),
            Type::PatternKeyRef(subject, _) | Type::PatternKeyRestRef(subject, _) => {
                Self::type_contains_param_ref(subject)
            }
            Type::ReceiverMethodRef(receiver_type, _) => {
                Self::type_contains_param_ref(receiver_type)
            }
            Type::Proc { return_type, .. } => Self::type_contains_param_ref(return_type),
            _ => false,
        }
    }

    fn lookup_method_sig_in_class_kind(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<MethodSig> {
        let data = self.class_data.get(class_name)?;
        let method = Self::method_for_lookup_kind(data, method_name, Some(is_singleton))?;
        Some(self.build_method_sig(class_name, method))
    }

    fn method_for_lookup_kind<'a>(
        data: &'a ClassData,
        method_name: &str,
        is_singleton: Option<bool>,
    ) -> Option<&'a MethodDef> {
        let slots = data.method_index.get(method_name)?;
        match is_singleton {
            Some(true) if !slots.has(true) => return None,
            Some(false) if !slots.has(false) => return None,
            _ => {}
        }
        let mut best: Option<&'a MethodDef> = None;
        let mut best_priority: u8 = 4;
        for method in data.methods.iter().rev() {
            if method.name != method_name {
                continue;
            }
            if let Some(flag) = is_singleton
                && method.is_singleton != flag
            {
                continue;
            }
            let priority = if !method.rbs_file_source && method.has_annotation() {
                1
            } else if method.rbs_file_source {
                2
            } else {
                3
            };
            if priority < best_priority {
                best = Some(method);
                best_priority = priority;
                if priority == 1 {
                    break;
                }
            }
        }
        best
    }

    fn method_is_undefined_for_lookup(
        data: &ClassData,
        method_name: &str,
        is_singleton: bool,
    ) -> bool {
        data.cold()
            .undefined_methods
            .iter()
            .any(|(name, singleton)| name.as_ref() == method_name && *singleton == is_singleton)
    }

    fn set_method_slot(data: &mut ClassData, method_name: Sym, is_singleton: bool, idx: usize) {
        data.method_index.set_slot(method_name, is_singleton, idx);
    }

    fn index_method_if_absent(
        data: &mut ClassData,
        method_name: Sym,
        is_singleton: bool,
        idx: usize,
    ) {
        data.method_index
            .set_slot_if_absent(method_name, is_singleton, idx);
    }

    fn rebuild_method_index(data: &mut ClassData) {
        data.method_index.clear();
        let methods: Vec<(Sym, bool, bool)> = data
            .methods
            .iter()
            .map(|method| (method.name, method.is_singleton, method.rbs_file_source))
            .collect();
        // rebuild the method index: user definitions take priority, RBS/synthetic only fill empty keys (matches `add_method_def` shadowing order).
        for (idx, (method_name, is_singleton, rbs_file_source)) in methods.iter().enumerate() {
            if !rbs_file_source {
                Self::index_method_if_absent(data, *method_name, *is_singleton, idx);
            }
        }
        for (idx, (method_name, is_singleton, rbs_file_source)) in methods.into_iter().enumerate() {
            if rbs_file_source {
                Self::index_method_if_absent(data, method_name, is_singleton, idx);
            }
        }
    }

    pub fn class_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.class_data.keys().map(|k| k.to_string()).collect();
        names.sort();
        names
    }

    pub fn class_count(&self) -> usize {
        self.class_data.len()
    }

    /// per-class holder breakdown for `--memory-breakdown`.
    pub fn breakdown_totals(&self) -> RegistryBreakdownTotals {
        let mut totals = RegistryBreakdownTotals {
            class_count: self.class_data.len(),
            ..Default::default()
        };
        for data in self.class_data.values() {
            totals.method_count += data.methods.len();
            totals.method_index_count += data.method_index.len();
            totals.constant_count += data.constants.len();
            totals.ivar_count += data.ivars.len();
            totals.singleton_ivar_count += data.cold().singleton_ivars.len();
            totals.class_variable_count += data.cold().class_variables.len();
            totals.call_site_count += data.call_sites.len();
            totals.mixin_count += data.mixins.len();
            totals.undefined_method_count += data.cold().undefined_methods.len();
            totals.annotated_param_count += data.cold().annotated_params.len();
            for method in &data.methods {
                totals.param_count += method.param_infos.len();
                totals.rbs_overload_count += method.rbs_method_types.len();
            }
        }
        totals.method_block_meta_count = self.tail().method_block_meta.len();
        totals.name_pool_count = self.tail().name_pool.len();
        totals.type_alias_count = self.tail().type_aliases.len();
        totals.global_variable_count = self.tail().global_variables.len();
        totals
    }

    pub fn class_names_unsorted(&self) -> Vec<Sym> {
        self.class_data.keys().copied().collect()
    }

    pub fn user_defined_class_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .class_data
            .iter()
            .filter_map(|(name, data)| data.user_defined.then_some(name.to_string()))
            .collect();
        names.sort();
        names
    }

    pub fn user_defined_class_names_unsorted(&self) -> Vec<Sym> {
        self.class_data
            .iter()
            .filter_map(|(name, data)| data.user_defined.then_some(*name))
            .collect()
    }

    pub fn is_user_defined_class(&self, class_name: &str) -> bool {
        self.class_data
            .get(class_name)
            .is_some_and(|data| data.user_defined)
    }

    pub fn get_call_sites(&self, class_name: &str) -> &CallSiteStore {
        static EMPTY: CallSiteStore = CallSiteStore {
            head: None,
            tail: Vec::new(),
        };
        self.class_data
            .get(class_name)
            .map(|d| &d.call_sites)
            .unwrap_or(&EMPTY)
    }

    pub fn get_methods(&self, class_name: &str) -> &[Arc<MethodDef>] {
        self.class_data
            .get(class_name)
            .map(|d| d.methods.as_slice())
            .unwrap_or(&[])
    }

    pub fn lookup_method_def(
        &self,
        class_name: &str,
        method_name: &str,
        is_singleton: bool,
    ) -> Option<&MethodDef> {
        self.class_data.get(class_name).and_then(|data| {
            data.method_index
                .get(method_name)
                .and_then(|slots| slots.get(is_singleton))
                .and_then(|idx| data.methods.get(idx))
                .map(|method| method.as_ref())
        })
    }

    pub fn get_ivar_names(&self, class_name: &str) -> Vec<String> {
        self.class_data
            .get(class_name)
            .map(|d| d.ivars.keys().map(|name| name.to_string()).collect())
            .unwrap_or_default()
    }

    /// Resolve ParamRef types in an ivar's stored types using the given param_types.
    pub fn resolve_ivar_param_refs(
        &mut self,
        class_name: &str,
        ivar_name: &str,
        param_types: &[Type],
    ) {
        if let Some(data) = self.class_data.get_mut(class_name)
            && let Some(types) = data.ivars.get_mut(ivar_name)
        {
            let resolved: Vec<Type> = types
                .iter()
                .map(|t| Self::substitute_param_refs_static(t, param_types))
                .collect();
            *types = resolved;
        }
    }

    fn substitute_param_refs_static(ty: &Type, param_types: &[Type]) -> Type {
        Self::substitute_param_refs_static_with_keywords(ty, param_types, &HashMap::new())
    }

    fn substitute_param_refs_static_with_keywords(
        ty: &Type,
        param_types: &[Type],
        keyword_types: &HashMap<String, Type>,
    ) -> Type {
        let recur = |t: &Type| {
            Self::substitute_param_refs_static_with_keywords(t, param_types, keyword_types)
        };
        match ty {
            Type::ParamRef(idx) => param_types.get(*idx).cloned().unwrap_or(Type::Untyped),
            Type::KeywordParamRef(name) => keyword_types
                .get(name.as_str())
                .cloned()
                .unwrap_or(Type::Untyped),
            Type::Union(parts) => {
                let resolved: Vec<Type> = parts.iter().map(&recur).collect();
                // untyped coming from a `ParamRef` is kept in the union (not dropped by a concrete sibling).
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Intersection(parts) => Type::Intersection(parts.iter().map(&recur).collect()),
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(recur(inner)))),
            Type::Hash(Some(k), Some(v)) => {
                Type::Hash(Some(Box::new(recur(k))), Some(Box::new(recur(v))))
            }
            Type::Hash(Some(k), None) => Type::Hash(Some(Box::new(recur(k))), None),
            Type::Hash(None, Some(v)) => Type::Hash(None, Some(Box::new(recur(v)))),
            Type::Record(fields) => {
                let resolved: Vec<RecordField> = fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: recur(&field.value),
                        optional: field.optional,
                    })
                    .collect();
                Type::Record(resolved)
            }
            Type::Tuple(elems) => Type::Tuple(elems.iter().map(&recur).collect()),
            Type::ReceiverMethodRef(receiver_type, method_name) => {
                let resolved_receiver = recur(receiver_type);
                Type::ReceiverMethodRef(Box::new(resolved_receiver), *method_name)
            }
            Type::Proc {
                return_type,
                param_count,
            } => Type::Proc {
                return_type: Box::new(recur(return_type)),
                param_count: *param_count,
            },
            // don't eagerly resolve while a param marker remains (`_ => Untyped` would destroy the marker -> leave it for call-site substitution).
            Type::PatternIndexRef(subject, index) => {
                let resolved_subject = recur(subject);
                if Self::type_contains_param_ref_static(&resolved_subject) {
                    Type::PatternIndexRef(Box::new(resolved_subject), *index)
                } else {
                    Self::resolve_pattern_index_ref(&resolved_subject, *index)
                }
            }
            Type::PatternRestRef(subject) => {
                let resolved_subject = recur(subject);
                if Self::type_contains_param_ref_static(&resolved_subject) {
                    Type::PatternRestRef(Box::new(resolved_subject))
                } else {
                    Self::resolve_pattern_rest_ref(&resolved_subject)
                }
            }
            Type::PatternTrailingRef(subject, from_end) => {
                let resolved_subject = recur(subject);
                if Self::type_contains_param_ref_static(&resolved_subject) {
                    Type::PatternTrailingRef(Box::new(resolved_subject), *from_end)
                } else {
                    Self::resolve_pattern_trailing_ref(&resolved_subject, *from_end)
                }
            }
            Type::PatternKeyRef(subject, key) => {
                let resolved_subject = recur(subject);
                if Self::type_contains_param_ref_static(&resolved_subject) {
                    Type::PatternKeyRef(Box::new(resolved_subject), key.clone())
                } else {
                    Self::resolve_pattern_key_ref(&resolved_subject, key)
                }
            }
            Type::PatternKeyRestRef(subject, matched_keys) => {
                let resolved_subject = recur(subject);
                if Self::type_contains_param_ref_static(&resolved_subject) {
                    Type::PatternKeyRestRef(Box::new(resolved_subject), matched_keys.clone())
                } else {
                    Self::resolve_pattern_key_rest_ref(&resolved_subject, matched_keys)
                }
            }
            _ => ty.clone(),
        }
    }

    pub(crate) fn resolve_pattern_index_ref(subject: &Type, index: usize) -> Type {
        match subject {
            Type::Union(parts) => {
                let mut resolved: Vec<Type> = parts
                    .iter()
                    .map(|part| Self::resolve_pattern_index_ref(part, index))
                    .collect();
                if resolved.iter().any(|ty| *ty != Type::Untyped) {
                    resolved.retain(|ty| *ty != Type::Untyped);
                }
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Tuple(elems) => elems.get(index).cloned().unwrap_or(Type::Untyped),
            Type::Array(Some(elem)) => *elem.clone(),
            _ => Type::Untyped,
        }
    }

    pub(crate) fn resolve_pattern_rest_ref(subject: &Type) -> Type {
        match subject {
            Type::Union(parts) => {
                let mut resolved: Vec<Type> =
                    parts.iter().map(Self::resolve_pattern_rest_ref).collect();
                if resolved
                    .iter()
                    .any(|ty| !Self::is_generic_pattern_placeholder(ty))
                {
                    resolved.retain(|ty| !Self::is_generic_pattern_placeholder(ty));
                }
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Tuple(elems) => Type::Array(Some(Box::new(Type::from_type_vec(elems.clone())))),
            Type::Array(Some(elem)) => Type::Array(Some(Box::new(*elem.clone()))),
            _ => Type::Array(Some(Box::new(Type::Untyped))),
        }
    }

    pub(crate) fn resolve_pattern_trailing_ref(subject: &Type, from_end: usize) -> Type {
        match subject {
            Type::Union(parts) => {
                let mut resolved: Vec<Type> = parts
                    .iter()
                    .map(|part| Self::resolve_pattern_trailing_ref(part, from_end))
                    .collect();
                if resolved.iter().any(|ty| *ty != Type::Untyped) {
                    resolved.retain(|ty| *ty != Type::Untyped);
                }
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Tuple(elems) => elems
                .len()
                .checked_sub(from_end + 1)
                .and_then(|index| elems.get(index).cloned())
                .unwrap_or(Type::Untyped),
            Type::Array(Some(elem)) => *elem.clone(),
            _ => Type::Untyped,
        }
    }

    pub(crate) fn resolve_pattern_key_ref(subject: &Type, key: &RecordKey) -> Type {
        match subject {
            Type::Union(parts) => {
                let mut resolved: Vec<Type> = parts
                    .iter()
                    .map(|part| Self::resolve_pattern_key_ref(part, key))
                    .collect();
                if resolved.iter().any(|ty| *ty != Type::Untyped) {
                    resolved.retain(|ty| *ty != Type::Untyped);
                }
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Record(fields) => fields
                .iter()
                .find(|field| field.key == *key)
                .map(|field| field.value.clone())
                .unwrap_or(Type::Untyped),
            Type::Hash(key_type, Some(value_type))
                if key_type
                    .as_deref()
                    .map(|key_type| Self::pattern_hash_key_type_matches(key_type, key))
                    .unwrap_or(true) =>
            {
                *value_type.clone()
            }
            _ => Type::Untyped,
        }
    }

    pub(crate) fn resolve_pattern_key_rest_ref(subject: &Type, matched_keys: &[RecordKey]) -> Type {
        match subject {
            Type::Union(parts) => {
                let mut resolved: Vec<Type> = parts
                    .iter()
                    .map(|part| Self::resolve_pattern_key_rest_ref(part, matched_keys))
                    .collect();
                if resolved
                    .iter()
                    .any(|ty| !Self::is_generic_pattern_placeholder(ty))
                {
                    resolved.retain(|ty| !Self::is_generic_pattern_placeholder(ty));
                }
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .filter(|field| !matched_keys.contains(&field.key))
                    .cloned()
                    .collect(),
            ),
            Type::Hash(Some(key), Some(value)) => {
                Type::Hash(Some(Box::new(*key.clone())), Some(Box::new(*value.clone())))
            }
            _ => Type::Hash(Some(Box::new(Type::Untyped)), Some(Box::new(Type::Untyped))),
        }
    }

    fn is_generic_pattern_placeholder(ty: &Type) -> bool {
        matches!(ty, Type::Untyped)
            || matches!(ty, Type::Array(Some(inner)) if **inner == Type::Untyped)
            || matches!(
                ty,
                Type::Hash(Some(key), Some(value))
                    if **key == Type::Untyped && **value == Type::Untyped
            )
    }

    fn pattern_hash_key_type_matches(key_type: &Type, key: &RecordKey) -> bool {
        match (key_type, key) {
            (Type::Untyped | Type::Top, _) => true,
            (Type::Symbol, RecordKey::Symbol(_)) => true,
            (Type::String, RecordKey::String(_)) => true,
            (Type::LiteralSymbol(expected), RecordKey::Symbol(actual)) => expected == actual,
            (Type::LiteralString(expected), RecordKey::String(actual)) => expected == actual,
            (Type::Union(parts), _) => parts
                .iter()
                .any(|part| Self::pattern_hash_key_type_matches(part, key)),
            _ => false,
        }
    }
}

fn to_camel_case(snake: &str) -> String {
    snake
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// pin the `ClassData` shell at 264B (tens of thousands live at once -> directly impacts LSP footprint; evict to cold or a box when exceeded).
    #[test]
    fn class_data_shell_stays_small() {
        assert!(
            std::mem::size_of::<ClassData>() <= 272,
            "ClassData = {}B (> 272B)",
            std::mem::size_of::<ClassData>()
        );
    }

    #[test]
    fn method_index_freeze_round_trip() {
        let mut index = MethodIndex::default();
        index.set_slot(Sym::new("save"), false, 0);
        index.set_slot(Sym::new("save"), true, 1);
        index.set_slot(Sym::new("build"), false, 2);
        index.freeze();
        assert!(matches!(index, MethodIndex::Frozen(_)));
        assert_eq!(index.len(), 2);
        assert_eq!(
            index.get("save").and_then(|slots| slots.get(false)),
            Some(0)
        );
        assert_eq!(index.get("save").and_then(|slots| slots.get(true)), Some(1));
        assert_eq!(index.get("build").and_then(|slots| slots.get(true)), None);
        assert!(index.contains_key("build"));
        assert!(!index.contains_key("missing"));
        assert!(index.get("missing").is_none());

        // a known name updates its slot in place, keeping the packed form.
        index.set_slot(Sym::new("build"), true, 3);
        index.set_slot_if_absent(Sym::new("build"), true, 9);
        assert!(matches!(index, MethodIndex::Frozen(_)));
        assert_eq!(
            index.get("build").and_then(|slots| slots.get(true)),
            Some(3)
        );

        // an unknown name rematerializes the map and keeps the frozen entries.
        index.set_slot(Sym::new("touch"), false, 4);
        assert!(matches!(index, MethodIndex::Live(_)));
        let mut entries: Vec<(String, Option<usize>, Option<usize>)> = index
            .iter()
            .map(|(name, slots)| (name.as_str().to_string(), slots.instance, slots.singleton))
            .collect();
        entries.sort();
        assert_eq!(
            entries,
            vec![
                ("build".to_string(), Some(2), Some(3)),
                ("save".to_string(), Some(0), Some(1)),
                ("touch".to_string(), Some(4), None),
            ]
        );

        index.freeze();
        assert!(matches!(index, MethodIndex::Frozen(_)));
        assert_eq!(
            index.get("touch").and_then(|slots| slots.get(false)),
            Some(4)
        );
        index.clear();
        assert!(index.is_empty());
        assert!(index.get("save").is_none());
    }

    #[test]
    fn method_file_paths_freeze_round_trip() {
        let a: SharedPath = Arc::from("app/a.rb");
        let b: SharedPath = Arc::from("app/b.rb");
        let mut paths = MethodFilePaths::default();
        paths.insert((Sym::new("one"), false), a.clone());
        paths.insert((Sym::new("two"), true), a.clone());
        paths.freeze();
        assert!(matches!(paths, MethodFilePaths::Uniform(_)));
        assert_eq!(paths.len(), 2);
        assert_eq!(paths.get(&(Sym::new("one"), false)), Some(&a));
        assert_eq!(paths.get(&(Sym::new("two"), false)), None);
        assert_eq!(paths.get(&(Sym::new("missing"), false)), None);

        // a differing path materializes the map and keeps the frozen entries.
        paths.insert((Sym::new("three"), false), b.clone());
        assert!(matches!(paths, MethodFilePaths::PerMethod(_)));
        let mut entries: Vec<(String, bool, String)> = paths
            .iter()
            .map(|((name, is_singleton), path)| {
                (name.as_str().to_string(), is_singleton, path.to_string())
            })
            .collect();
        entries.sort();
        assert_eq!(
            entries,
            vec![
                ("one".to_string(), false, "app/a.rb".to_string()),
                ("three".to_string(), false, "app/b.rb".to_string()),
                ("two".to_string(), true, "app/a.rb".to_string()),
            ]
        );

        paths.freeze();
        assert!(matches!(paths, MethodFilePaths::PerMethod(_)));
        paths.retain_paths(|path| path.as_ref() == "app/a.rb");
        paths.freeze();
        assert!(matches!(paths, MethodFilePaths::Uniform(_)));
        paths.remove(&(Sym::new("one"), false));
        assert_eq!(paths.len(), 1);
        paths.retain_paths(|_| false);
        assert!(paths.is_empty());
        assert!(matches!(paths, MethodFilePaths::Empty));
    }

    fn test_method(name: &str, raw_return_type: Type) -> MethodDef {
        MethodDef {
            name: Sym::new(name),
            param_infos: Vec::new(),
            raw_return_type,
            sorbet_modifier_comments: Vec::new(),
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            attr_ivar: None,
            is_singleton: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            rbs_method_types: Default::default(),
            extra_overloads: Vec::new(),
            loc: None,
        }
    }

    fn test_call_site(method_name: &str) -> CallSite {
        CallSite {
            method_name: method_name.into(),
            method_is_singleton: false,
            arg_types: Vec::new(),
            keyword_arg_types: KeywordArgTypes::new(),
            block: None,
            caller_context: None,
        }
    }

    /// finalize also resolves nominals inside compound types in lexical order (unresolved ones stay pending -> cross-file finalize).
    #[test]
    fn finalize_pending_scoped_type_refs_resolves_composite_type_args() {
        let mut registry = TypeRegistry::new();
        let raw_array = Type::Array(Some(Box::new(Type::Class(Sym::new("Inner")))));
        let raw_nilable = Type::Union(vec![Type::Class(Sym::new("Inner")), Type::Nil]);
        registry.mark_user_defined("NS::S");
        registry.add_method_def("NS::S", test_method("xs", raw_array.clone()));
        registry.add_method_def("NS::S", test_method("y", raw_nilable.clone()));
        registry.push_pending_scoped_type_ref(PendingScopedTypeRef {
            owner_class: "NS::S".to_string(),
            method_name: "xs".to_string(),
            is_singleton: false,
            declaration_scope: "NS::S".to_string(),
            raw_type: raw_array.clone(),
        });
        registry.push_pending_scoped_type_ref(PendingScopedTypeRef {
            owner_class: "NS::S".to_string(),
            method_name: "y".to_string(),
            is_singleton: false,
            declaration_scope: "NS::S".to_string(),
            raw_type: raw_nilable,
        });

        // don't write back while the reference target is undefined; keep it pending (stays as the bare fallback).
        registry.finalize_pending_scoped_type_refs();
        assert_eq!(
            registry
                .lookup_method_def("NS::S", "xs", false)
                .unwrap()
                .raw_return_type,
            raw_array
        );

        // once `NS::Inner` is defined, it resolves to the lexical-scope side (`NS::Inner`).
        registry.mark_user_defined("NS::Inner");
        registry.finalize_pending_scoped_type_refs();
        assert_eq!(
            registry
                .lookup_method_def("NS::S", "xs", false)
                .unwrap()
                .raw_return_type,
            Type::Array(Some(Box::new(Type::Class(Sym::new("NS::Inner")))))
        );
        assert_eq!(
            registry
                .lookup_method_def("NS::S", "y", false)
                .unwrap()
                .raw_return_type,
            Type::Union(vec![Type::Class(Sym::new("NS::Inner")), Type::Nil])
        );
        // all references are resolved, so pending is empty.
        assert!(registry.take_pending_scoped_type_refs().is_empty());
    }

    #[test]
    fn method_visibility_setter_removes_entry_when_public() {
        let mut registry = TypeRegistry::new();
        registry.set_method_visibility("A", "foo", false, Some(Visibility::Private));
        assert_eq!(
            registry.method_visibility("A", "foo", false),
            Some(Visibility::Private)
        );
        // specifying `None`, equivalent to `public :foo`, removes it from the map and reverts to Public.
        registry.set_method_visibility("A", "foo", false, None);
        assert_eq!(registry.method_visibility("A", "foo", false), None);
    }

    #[test]
    fn bare_ivar_reader_is_pure_but_unmarked_method_is_not() {
        let mut registry = TypeRegistry::new();
        registry.mark_bare_ivar_reader("A", "corporation", false);
        assert!(registry.is_pure_ivar_reader_method("A", "corporation", false));
        // a method that isn't recorded is not a pure reader.
        assert!(!registry.is_pure_ivar_reader_method("A", "other", false));
        // under singleton dispatch, it doesn't match the instance's bare reader.
        assert!(!registry.is_pure_ivar_reader_method("A", "corporation", true));
    }

    #[test]
    fn schema_column_reader_is_pure_but_writer_and_predicate_are_not() {
        let mut registry = TypeRegistry::new();
        registry.register_dirty_pattern_columns("Post", vec![(Sym::new("title"), Type::String)]);
        // a DB column reader is a pure reader that doesn't write to self.
        assert!(registry.is_pure_ivar_reader_method("Post", "title", false));
        // writer/predicate names aren't in the column-name set, so they aren't pure readers.
        assert!(!registry.is_pure_ivar_reader_method("Post", "title=", false));
        assert!(!registry.is_pure_ivar_reader_method("Post", "title?", false));
        // methods that aren't a different column name are also excluded.
        assert!(!registry.is_pure_ivar_reader_method("Post", "body", false));
    }

    #[test]
    fn method_visibility_merges_add_if_absent() {
        let mut source = TypeRegistry::new();
        source.set_method_visibility("A", "keep", false, Some(Visibility::Private));
        source.set_method_visibility("A", "loses", false, Some(Visibility::Protected));

        let mut target = TypeRegistry::new();
        // existing entries aren't overwritten (`AddIfAbsent`).
        target.set_method_visibility("A", "loses", false, Some(Visibility::Private));

        target.merge_rbs_registry(&source);

        assert_eq!(
            target.method_visibility("A", "keep", false),
            Some(Visibility::Private)
        );
        assert_eq!(
            target.method_visibility("A", "loses", false),
            Some(Visibility::Private)
        );
    }

    #[test]
    fn resolve_deferred_refs_breaks_recursive_cycles() {
        let mut registry = TypeRegistry::new();
        registry.add_method_def(
            "Foo",
            test_method("a", Type::MethodReturnRef("Foo".into(), "b".into())),
        );
        registry.add_method_def(
            "Foo",
            test_method("b", Type::MethodReturnRef("Foo".into(), "a".into())),
        );

        assert_eq!(
            registry
                .resolve_deferred_refs("Foo", &Type::MethodReturnRef("Foo".into(), "a".into()),),
            Type::Untyped
        );
    }

    fn declare_class(registry: &mut TypeRegistry, name: &str) {
        registry.set_class_location(name, SourceLocation { line: 1, column: 0 });
    }

    #[test]
    fn ancestor_knowledge_complete_for_fully_resolved_chain() {
        let mut registry = TypeRegistry::new();
        // Leaf < Mid < Base (all declared), Leaf includes M (declared, empty).
        declare_class(&mut registry, "Base");
        declare_class(&mut registry, "Mid");
        registry.set_superclass("Mid", "Base");
        declare_class(&mut registry, "M");
        registry.set_is_module("M", true);
        declare_class(&mut registry, "Leaf");
        registry.set_superclass("Leaf", "Mid");
        registry.add_mixin("Leaf", "M", MixinKind::Include);

        // Every superclass / mixin edge resolves to a substantive definition;
        // an empty-but-declared module counts as a fully known surface.
        assert!(registry.ancestor_knowledge_complete("Leaf"));
    }

    #[test]
    fn mixin_lookup_and_ancestor_order_follow_latest_application() {
        let mut registry = TypeRegistry::new();

        declare_class(&mut registry, "IncludeFirst");
        registry.set_is_module("IncludeFirst", true);
        registry.add_method_def(
            "IncludeFirst",
            test_method("value", Type::LiteralInteger(1)),
        );
        declare_class(&mut registry, "IncludeSecond");
        registry.set_is_module("IncludeSecond", true);
        registry.add_method_def(
            "IncludeSecond",
            test_method("value", Type::LiteralInteger(2)),
        );
        declare_class(&mut registry, "IncludeHost");
        registry.add_mixin("IncludeHost", "IncludeFirst", MixinKind::Include);
        registry.add_mixin("IncludeHost", "IncludeSecond", MixinKind::Include);
        assert_eq!(
            registry.resolve_instance_method_call_owners("IncludeHost", "value"),
            vec!["IncludeSecond"]
        );

        declare_class(&mut registry, "PrependFirst");
        registry.set_is_module("PrependFirst", true);
        registry.add_method_def(
            "PrependFirst",
            test_method("value", Type::LiteralInteger(1)),
        );
        declare_class(&mut registry, "PrependSecond");
        registry.set_is_module("PrependSecond", true);
        registry.add_method_def(
            "PrependSecond",
            test_method("value", Type::LiteralInteger(2)),
        );
        declare_class(&mut registry, "PrependHost");
        registry.add_mixin("PrependHost", "PrependFirst", MixinKind::Prepend);
        registry.add_mixin("PrependHost", "PrependSecond", MixinKind::Prepend);
        assert_eq!(
            registry.resolve_instance_method_call_owners("PrependHost", "value"),
            vec!["PrependSecond"]
        );

        declare_class(&mut registry, "ExtendFirst");
        registry.set_is_module("ExtendFirst", true);
        registry.add_method_def("ExtendFirst", test_method("value", Type::LiteralInteger(1)));
        declare_class(&mut registry, "ExtendSecond");
        registry.set_is_module("ExtendSecond", true);
        registry.add_method_def(
            "ExtendSecond",
            test_method("value", Type::LiteralInteger(2)),
        );
        declare_class(&mut registry, "ExtendHost");
        registry.add_mixin("ExtendHost", "ExtendFirst", MixinKind::Extend);
        registry.add_mixin("ExtendHost", "ExtendSecond", MixinKind::Extend);
        assert_eq!(
            registry.resolve_method_call_owners("ExtendHost", "value", true),
            vec![("ExtendSecond".to_string(), false)]
        );

        declare_class(&mut registry, "BasicObject");
        declare_class(&mut registry, "Kernel");
        registry.set_is_module("Kernel", true);
        declare_class(&mut registry, "Object");
        registry.set_superclass("Object", "BasicObject");
        registry.add_mixin("Object", "Kernel", MixinKind::Include);
        declare_class(&mut registry, "AncestorHost");
        registry.add_mixin("AncestorHost", "IncludeFirst", MixinKind::Include);
        registry.add_mixin("AncestorHost", "IncludeSecond", MixinKind::Include);
        registry.add_mixin("AncestorHost", "PrependFirst", MixinKind::Prepend);
        registry.add_mixin("AncestorHost", "PrependSecond", MixinKind::Prepend);

        assert_eq!(
            registry
                .ordered_ancestor_names("AncestorHost")
                .expect("complete ancestor chain")
                .into_iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>(),
            vec![
                "PrependSecond",
                "PrependFirst",
                "AncestorHost",
                "IncludeSecond",
                "IncludeFirst",
                "Object",
                "Kernel",
                "BasicObject",
            ]
        );
    }

    #[test]
    fn mixin_hook_call_sites_use_singleton_targets() {
        let mut registry = TypeRegistry::new();
        registry.set_is_module("Hook", true);

        let mut hook = test_method("included", Type::Untyped);
        hook.is_singleton = true;
        hook.param_infos = vec![ParamInfo {
            name: "base".to_string(),
            kind: ParamKind::Required,
            default_type: None,
        }];
        registry.add_method_def("Hook", hook);
        registry.add_mixin("First", "Hook", MixinKind::Include);
        registry.add_mixin("Second", "Hook", MixinKind::Include);

        registry.apply_mixin_hook_mixins();

        let method = registry
            .lookup_method_def("Hook", "included", true)
            .expect("hook method should be present");
        let params = registry.resolve_params("Hook", method);
        assert_eq!(
            params[0].param_type,
            Type::from_type_vec(vec![
                Type::Singleton(Sym::new("First")),
                Type::Singleton(Sym::new("Second")),
            ])
        );
        assert_eq!(
            registry
                .get_call_sites("Hook")
                .iter()
                .filter(|site| site.method_name.as_ref() == "included")
                .count(),
            2
        );
    }

    #[test]
    fn ancestor_knowledge_incomplete_when_mixin_is_empty_stub() {
        let mut registry = TypeRegistry::new();
        declare_class(&mut registry, "Host");
        // A stub with no loc / methods / superclass / mixins (as synthesized for
        // an undefined constant) has no knowable method surface.
        registry.add_mixin("Host", "PhantomMixin", MixinKind::Include);
        let data = registry
            .class_data
            .entry(Sym::new("PhantomMixin"))
            .or_default();
        assert!(!data.has_type_substance());

        assert!(!registry.ancestor_knowledge_complete("Host"));
    }

    #[test]
    fn ancestor_knowledge_incomplete_when_superclass_unresolved() {
        let mut registry = TypeRegistry::new();
        declare_class(&mut registry, "Sub");
        // Superclass name is recorded but never declared -> unknowable surface.
        registry.set_superclass("Sub", "NeverDeclaredBase");

        assert!(!registry.ancestor_knowledge_complete("Sub"));
    }

    #[test]
    fn ancestor_knowledge_complete_terminates_on_cycle() {
        let mut registry = TypeRegistry::new();
        // `class A < A` and mutual includes must terminate rather than recurse
        // forever; a cycle is treated as terminated, not unknown.
        declare_class(&mut registry, "A");
        registry.set_superclass("A", "A");
        declare_class(&mut registry, "B");
        registry.set_is_module("B", true);
        declare_class(&mut registry, "C");
        registry.set_is_module("C", true);
        registry.add_mixin("B", "C", MixinKind::Include);
        registry.add_mixin("C", "B", MixinKind::Include);

        assert!(registry.ancestor_knowledge_complete("A"));
        assert!(registry.ancestor_knowledge_complete("B"));
    }

    #[test]
    fn ancestor_knowledge_complete_for_modeled_framework_base() {
        let mut registry = TypeRegistry::new();
        // modeled framework base subclass: the constant chain is known; the method surface is complete after generated-artifact merge.
        declare_class(&mut registry, "Item");
        registry.set_superclass("Item", "ApplicationRecord");

        assert!(registry.ancestor_knowledge_complete("Item"));
        assert!(registry.method_surface_knowledge_complete("Item", false));
        assert!(!registry.method_surface_knowledge_complete("Item", true));

        // Merged generated declarations (a method sourced from an `.rbi`)
        // make the surface provable again.
        registry
            .class_data_mut("Item")
            .method_file_paths
            .insert((Sym::new("person_id"), false), Arc::from("dsl/item.rbi"));
        assert!(registry.method_surface_knowledge_complete("Item", true));
    }

    #[test]
    fn method_surface_open_when_chain_defines_method_missing() {
        let mut registry = TypeRegistry::new();
        // `method_missing` present on the chain -> the method surface is open (responds to any call; constant lookup is unaffected).
        declare_class(&mut registry, "Config");
        declare_class(&mut registry, "DynamicBase");
        registry.set_superclass("Config", "DynamicBase");
        registry.add_method_def("DynamicBase", test_method("method_missing", Type::Untyped));

        assert!(registry.ancestor_knowledge_complete("Config"));
        assert!(!registry.method_surface_knowledge_complete("Config", false));
        assert!(!registry.method_surface_knowledge_complete("Config", true));
    }

    #[test]
    fn resolve_deferred_refs_caps_deep_structural_types() {
        let registry = TypeRegistry::new();
        assert!(!TypeRegistry::type_needs_deferred_resolution(&Type::Array(
            Some(Box::new(Type::String))
        )));
        let mut ty = Type::Tuple(Vec::new());
        for _ in 0..25 {
            ty = Type::Array(Some(Box::new(ty)));
        }

        assert!(TypeRegistry::type_needs_deferred_resolution(&ty));
        let resolved = registry.resolve_deferred_refs_for_context("Object", false, &ty);

        let mut expected = Type::Untyped;
        for _ in 0..16 {
            expected = Type::Array(Some(Box::new(expected)));
        }
        assert_eq!(resolved, expected);

        let mut ty = Type::Tuple(Vec::new());
        for _ in 0..25 {
            ty = Type::Tuple(vec![ty]);
        }

        let resolved = registry.resolve_deferred_refs_for_context("Object", false, &ty);

        let mut expected = Type::Untyped;
        for _ in 0..16 {
            expected = Type::Tuple(vec![expected]);
        }
        assert_eq!(resolved, expected);
    }

    #[test]
    fn add_method_def_stores_param_names_in_param_infos() {
        let mut registry = TypeRegistry::new();
        registry.add_method_def(
            "Foo",
            MethodDef {
                name: Sym::new("call"),
                param_infos: vec![ParamInfo {
                    name: "value".to_string(),
                    kind: ParamKind::Required,
                    default_type: None,
                }],
                raw_return_type: Type::Untyped,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: false,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: None,
            },
        );

        let method = registry
            .lookup_method_def("Foo", "call", false)
            .expect("method should be stored");
        assert_eq!(method.param_name_at(0).as_deref(), Some("value"));
        assert_eq!(method.effective_param_names(), vec!["value".to_string()]);
    }

    #[test]
    fn take_method_body_summary_keeps_local_method_block_meta() {
        let mut registry = TypeRegistry::new();
        registry.set_file_path("Foo", "foo.rb");
        registry.set_file_path("Bar", "bar.rb");
        registry.set_method_block_meta(
            "Foo",
            "call",
            false,
            MethodBlockMeta {
                yield_param_types: vec![Type::Integer],
                return_type: None,
                forwarded_block: None,
                yields: true,
            },
        );
        registry.set_method_block_meta(
            "Foo",
            "build",
            true,
            MethodBlockMeta {
                yield_param_types: vec![Type::String],
                return_type: None,
                forwarded_block: None,
                yields: true,
            },
        );
        registry.set_method_block_meta(
            "Bar",
            "skip",
            false,
            MethodBlockMeta {
                yield_param_types: vec![Type::Symbol],
                return_type: None,
                forwarded_block: None,
                yields: true,
            },
        );

        let summary = registry.take_method_body_summary("foo.rb");
        let foo_meta = summary
            .method_block_meta_by_class
            .iter()
            .find(|(name, _)| name.as_ref() == "Foo")
            .map(|(_, meta)| meta)
            .expect("Foo method block meta should be summarized");
        assert_eq!(summary.method_block_meta_by_class.len(), 1);
        assert_eq!(
            foo_meta
                .instance
                .get("call")
                .map(|meta| meta.yield_param_types.clone()),
            Some(vec![Type::Integer])
        );
        assert_eq!(
            foo_meta
                .singleton
                .get("build")
                .map(|meta| meta.yield_param_types.clone()),
            Some(vec![Type::String])
        );
    }

    #[test]
    fn call_site_store_preserves_insertion_order_across_chunks() {
        // pins order-equivalence with the old representation that appended to a single Vec.
        // the only safe invariant is "the same elements are iterated in the same order".
        let mut store = CallSiteStore::default();
        store.push(test_call_site("t0"));
        store.push_chunk(Arc::from(vec![test_call_site("c0"), test_call_site("c1")]));
        store.push(test_call_site("t1"));
        store.push_chunk(Arc::from(vec![test_call_site("c2")]));
        store.extend([test_call_site("t2"), test_call_site("t3")]);

        let expected = ["t0", "c0", "c1", "t1", "c2", "t2", "t3"];
        assert_eq!(store.len(), expected.len());
        let flat: Vec<&str> = store.iter().map(|s| s.method_name.as_ref()).collect();
        assert_eq!(flat, expected);
        let by_index: Vec<&str> = (0..store.len())
            .map(|i| store.get(i).method_name.as_ref())
            .collect();
        assert_eq!(by_index, expected);
        let taken: Vec<String> = store
            .take_all()
            .into_iter()
            .map(|s| s.method_name.to_string())
            .collect();
        assert_eq!(taken, expected);
        assert!(store.is_empty());
    }

    #[test]
    fn merge_rbs_registry_keeps_method_block_meta_lookupable() {
        let mut rbs = TypeRegistry::new();
        rbs.set_method_block_meta(
            "Foo",
            "call",
            false,
            MethodBlockMeta {
                yield_param_types: vec![Type::Integer],
                return_type: None,
                forwarded_block: None,
                yields: true,
            },
        );

        let mut registry = TypeRegistry::new();
        registry.merge_rbs_registry(&rbs);

        let meta = registry
            .lookup_method_block_meta("Foo", "call", false)
            .expect("merged method block meta should be available");
        assert_eq!(meta.yield_param_types, vec![Type::Integer]);
    }

    #[test]
    fn class_name_pool_reuses_superclass_mixin_and_summary_names() {
        let mut registry = TypeRegistry::new_pooled();
        registry.set_superclass("Child", "Parent");
        registry.add_mixin("Child", "Enumerable", MixinKind::Include);
        registry.add_call_site(
            "Child",
            CallSite {
                method_name: "call".into(),
                method_is_singleton: false,
                arg_types: Vec::new(),
                keyword_arg_types: KeywordArgTypes::new(),
                block: None,
                caller_context: None,
            },
        );
        registry.set_file_path("Child", "child.rb");

        let parent_name = registry.shared_name("Parent");
        let enumerable_name = registry.shared_name("Enumerable");
        let child_name = registry.shared_name("Child");

        let data = registry.class_data_for("Child").expect("child data");
        assert!(std::sync::Arc::ptr_eq(
            data.superclass.as_ref().expect("superclass"),
            &parent_name
        ));
        assert!(std::sync::Arc::ptr_eq(
            &data.mixins[0].module_name,
            &enumerable_name
        ));

        let summary = registry.take_method_body_summary("child.rb");
        let (summary_child_name, _) = summary
            .call_sites_by_class
            .first()
            .expect("call site summary should exist");
        assert!(std::sync::Arc::ptr_eq(summary_child_name, &child_name));
    }

    #[test]
    fn finalize_pending_call_site_summaries_merges_only_pending_classes() {
        let mut registry = TypeRegistry::new();
        registry.add_call_site("Foo", test_call_site("call"));
        registry.add_call_site("Foo", test_call_site("call"));

        let summary = MethodBodySummary {
            call_sites_by_class: vec![("Bar".into(), vec![test_call_site("run")].into())],
            ..MethodBodySummary::default()
        };
        registry.apply_method_body_summary(&summary);

        assert!(
            registry
                .class_data_for("Foo")
                .expect("foo data")
                .has_pending_call_site_summary
        );
        assert!(
            !registry
                .class_data_for("Bar")
                .expect("bar data")
                .has_pending_call_site_summary
        );

        registry.finalize_pending_call_site_summaries();

        let foo = registry.class_data_for("Foo").expect("foo data");
        assert_eq!(foo.call_sites.len(), 1);
        assert!(!foo.has_pending_call_site_summary);

        let bar = registry.class_data_for("Bar").expect("bar data");
        assert_eq!(bar.call_sites.len(), 1);
        assert!(!bar.has_pending_call_site_summary);
    }

    /// accumulation during grouping (unnormalized union buildup) gets canonicalized on write-back,
    /// guaranteeing the same normal form as per-merge sort+dedup.
    #[test]
    fn finalize_call_site_summaries_canonicalizes_accumulated_unions() {
        let mut registry = TypeRegistry::new();
        let with_arg = |ty: Type| {
            let mut site = test_call_site("call");
            site.arg_types = vec![ty];
            site
        };
        registry.add_call_site("Foo", with_arg(Type::String));
        registry.add_call_site("Foo", with_arg(Type::Integer));
        registry.add_call_site("Foo", with_arg(Type::Nil));
        registry.add_call_site("Foo", with_arg(Type::Integer));
        registry.add_call_site("Foo", with_arg(Type::Untyped));

        registry.finalize_pending_call_site_summaries();

        let foo = registry.class_data_for("Foo").expect("foo data");
        assert_eq!(foo.call_sites.len(), 1);
        let summary = foo.call_sites.iter().next().expect("summary site");
        assert_eq!(
            summary.arg_types[0],
            Type::from_type_vec(vec![Type::String, Type::Integer, Type::Nil]),
            "accumulated slot must be canonical (sorted / deduped / untyped dropped)"
        );
    }

    /// a slot saturated past the union cap settles to untyped regardless of merge order.
    #[test]
    fn saturated_call_site_slot_collapses_to_untyped_deterministically() {
        let mut registry = TypeRegistry::new();
        for i in 0..=(Type::UNION_CARDINALITY_LIMIT) {
            let mut site = test_call_site("call");
            site.arg_types = vec![Type::LiteralInteger(i as i64)];
            registry.add_call_site("Foo", site);
        }

        registry.finalize_pending_call_site_summaries();

        let foo = registry.class_data_for("Foo").expect("foo data");
        assert_eq!(foo.call_sites.len(), 1);
        let summary = foo.call_sites.iter().next().expect("summary site");
        assert_eq!(summary.arg_types[0], Type::Untyped);
    }

    #[test]
    fn drop_transient_collection_state_keeps_lookup_facts() {
        let mut registry = TypeRegistry::new_pooled();
        registry.set_superclass("Child", "Parent");
        registry.add_call_site("Child", test_call_site("call"));
        registry.apply_global_resolution();

        registry.drop_transient_collection_state();

        let data = registry.class_data_for("Child").expect("child data");
        // the call site body stays because hover's attr type inference/propagation reads it.
        assert_eq!(data.call_sites.len(), 1);
        // fingerprint is only used for dedup during collection, so it is dropped.
        assert!(data.call_site_fingerprints.is_none());
        assert_eq!(
            data.superclass.as_deref(),
            Some("Parent"),
            "structural facts must survive the drop pass"
        );
    }

    #[test]
    fn propagate_call_sites_for_target_classes_filters_by_target_method_names() {
        let mut registry = TypeRegistry::new();
        registry.mark_user_defined("Provider");
        registry.mark_user_defined("Child");
        registry.mark_user_defined("Noise");
        registry.set_superclass("Child", "Provider");
        registry.add_method_def("Provider", test_method("greeting", Type::String));
        registry.add_call_site("Child", test_call_site("greeting"));
        registry.add_call_site("Noise", test_call_site("unrelated"));

        let mut targets = HashSet::new();
        targets.insert("Provider".to_string());
        registry.propagate_call_sites_for_target_classes(&targets);

        let provider = registry.class_data_for("Provider").expect("provider");
        assert!(
            provider
                .call_sites
                .iter()
                .any(|site| site.method_name.as_ref() == "greeting"),
            "matching target method names should still propagate"
        );
        let noise = registry.class_data_for("Noise").expect("noise");
        assert_eq!(
            noise.call_sites.len(),
            1,
            "non-target classes should keep only their own call sites"
        );
    }

    #[test]
    fn method_index_distinguishes_instance_and_singleton_variants() {
        let mut registry = TypeRegistry::new();
        registry.add_method_def("Foo", test_method("call", Type::Integer));
        let mut singleton = test_method("call", Type::String);
        singleton.is_singleton = true;
        registry.add_method_def("Foo", singleton);

        let instance = registry
            .lookup_method_def("Foo", "call", false)
            .expect("instance method should exist");
        let singleton = registry
            .lookup_method_def("Foo", "call", true)
            .expect("singleton method should exist");

        assert_eq!(instance.raw_return_type, Type::Integer);
        assert_eq!(singleton.raw_return_type, Type::String);
        assert!(registry.has_method_variant("Foo", "call", false));
        assert!(registry.has_method_variant("Foo", "call", true));
    }

    #[test]
    fn method_completion_for_union_merges_common_method_signature() {
        let mut registry = TypeRegistry::new();
        registry.add_method_def("A", test_method("value", Type::Integer));
        registry.add_method_def("A", test_method("only_a", Type::Integer));
        registry.add_method_def("B", test_method("value", Type::String));

        let candidates = registry.method_completion_candidates_for_type(&Type::Union(vec![
            Type::Class(Sym::new("A")),
            Type::Class(Sym::new("B")),
        ]));

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "value");
        assert_eq!(candidates[0].owner_class, "A | B");
        assert_eq!(
            candidates[0].sig.return_type,
            Type::from_type_vec(vec![Type::Integer, Type::String])
        );
    }

    #[test]
    fn constant_completion_candidates_follow_constant_aliases() {
        let mut registry = TypeRegistry::new();
        registry.set_is_module("Outer", true);
        registry.mark_user_defined("Outer::Inner");
        registry.set_is_module("Outer::Helper", true);
        registry.mark_user_defined("Outer::C");
        registry.set_constant("Outer", "VALUE", Type::Integer, None, None);
        registry.set_constant("Outer::Inner", "NESTED", Type::String, None, None);
        registry.set_constant(
            "Object",
            "Alias",
            Type::Singleton(Sym::new("Outer")),
            None,
            None,
        );

        let candidates = registry.constant_completion_candidates_for_namespace("Alias", "");
        let names: Vec<_> = candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect();

        assert_eq!(names, vec!["C", "Helper", "Inner", "VALUE"]);
        let inner = candidates
            .iter()
            .find(|candidate| candidate.name == "Inner")
            .expect("Inner candidate");
        assert_eq!(inner.full_name, "Outer::Inner");
        assert_eq!(inner.kind, ConstantCompletionKind::Class);
        let helper = candidates
            .iter()
            .find(|candidate| candidate.name == "Helper")
            .expect("Helper candidate");
        assert_eq!(helper.kind, ConstantCompletionKind::Module);
        let value = candidates
            .iter()
            .find(|candidate| candidate.name == "VALUE")
            .expect("VALUE candidate");
        assert_eq!(value.const_type, Some(Type::Integer));

        let relative_candidates =
            registry.constant_completion_candidates_for_namespace("Inner", "Outer::C");
        assert_eq!(relative_candidates.len(), 1);
        assert_eq!(relative_candidates[0].name, "NESTED");
        assert_eq!(relative_candidates[0].full_name, "Outer::Inner::NESTED");
    }

    #[test]
    fn update_method_return_type_variant_keeps_instance_and_singleton_separate() {
        let mut registry = TypeRegistry::new();
        registry.add_method_def("Foo", test_method("call", Type::Integer));
        let mut singleton = test_method("call", Type::String);
        singleton.is_singleton = true;
        registry.add_method_def("Foo", singleton);

        registry.update_method_return_type_variant("Foo", "call", true, Type::Bool);

        let instance = registry
            .lookup_method_def("Foo", "call", false)
            .expect("instance method should exist");
        let singleton = registry
            .lookup_method_def("Foo", "call", true)
            .expect("singleton method should exist");

        assert_eq!(instance.raw_return_type, Type::Integer);
        assert_eq!(singleton.raw_return_type, Type::Bool);
    }

    #[test]
    fn display_resolution_resolves_target_classes_via_external_method_chains() {
        let mut registry = TypeRegistry::new();
        registry.mark_user_defined("Provider");
        registry.mark_user_defined("Consumer");
        registry.add_method_def("Provider", test_method("value", Type::Integer));
        registry.add_method_def(
            "Provider",
            test_method(
                "wrapper",
                Type::MethodReturnRef("Provider".into(), "value".into()),
            ),
        );
        registry.add_method_def(
            "Consumer",
            test_method(
                "call",
                Type::MethodReturnRef("Provider".into(), "wrapper".into()),
            ),
        );

        let target_classes = HashSet::from([String::from("Consumer")]);
        registry.apply_display_resolution_for_targets(&target_classes);

        let consumer = registry
            .lookup_method_def("Consumer", "call", false)
            .expect("consumer method should exist");
        assert_eq!(consumer.raw_return_type, Type::Integer);

        let provider = registry
            .lookup_method_def("Provider", "wrapper", false)
            .expect("provider wrapper should exist");
        assert_eq!(
            provider.raw_return_type,
            Type::MethodReturnRef("Provider".into(), "value".into())
        );
    }

    #[test]
    fn global_resolution_resolves_subclass_method_refs() {
        let mut registry = TypeRegistry::new();
        registry.mark_user_defined("Base");
        registry.mark_user_defined("Child");
        registry.set_superclass("Child", "Base");
        registry.add_method_def("Child", test_method("value", Type::Integer));
        registry.add_method_def(
            "Base",
            test_method(
                "call_value",
                Type::MethodReturnRef("Base".into(), "value".into()),
            ),
        );

        registry.apply_global_resolution();

        let method = registry
            .lookup_method_def("Base", "call_value", false)
            .expect("base method should exist");
        assert_eq!(method.raw_return_type, Type::Integer);
    }

    #[test]
    fn global_resolution_resolves_refs_inside_structural_types() {
        let mut registry = TypeRegistry::new();
        registry.mark_user_defined("Box");
        registry.add_method_def("Box", test_method("value", Type::Integer));
        registry.add_method_def(
            "Box",
            test_method(
                "record",
                Type::Record(vec![RecordField {
                    key: RecordKey::Symbol("value".to_string()),
                    value: Type::MethodReturnRef("Box".into(), "value".into()),
                    optional: false,
                }]),
            ),
        );
        registry.add_method_def(
            "Box",
            test_method(
                "callback",
                Type::Proc {
                    return_type: Box::new(Type::MethodReturnRef("Box".into(), "value".into())),
                    param_count: 0,
                },
            ),
        );
        registry.add_method_def(
            "Box",
            test_method(
                "hash",
                Type::Hash(
                    Some(Box::new(Type::Symbol)),
                    Some(Box::new(Type::MethodReturnRef(
                        "Box".into(),
                        "value".into(),
                    ))),
                ),
            ),
        );

        registry.apply_global_resolution();

        let record = registry
            .lookup_method_def("Box", "record", false)
            .expect("record method");
        assert_eq!(
            record.raw_return_type,
            Type::Record(vec![RecordField {
                key: RecordKey::Symbol("value".to_string()),
                value: Type::Integer,
                optional: false,
            }])
        );
        let callback = registry
            .lookup_method_def("Box", "callback", false)
            .expect("callback method");
        assert_eq!(
            callback.raw_return_type,
            Type::Proc {
                return_type: Box::new(Type::Integer),
                param_count: 0,
            }
        );
        let hash = registry
            .lookup_method_def("Box", "hash", false)
            .expect("hash method");
        assert_eq!(
            hash.raw_return_type,
            Type::Hash(Some(Box::new(Type::Symbol)), Some(Box::new(Type::Integer)))
        );
    }

    #[test]
    fn global_resolution_resolves_pattern_refs_after_subject_refs() {
        let mut registry = TypeRegistry::new();
        registry.mark_user_defined("User");
        registry.add_method_def("User", test_method("name", Type::String));
        registry.add_method_def(
            "User",
            test_method(
                "payload",
                Type::Record(vec![RecordField {
                    key: RecordKey::Symbol("name".to_string()),
                    value: Type::MethodReturnRef("User".into(), "name".into()),
                    optional: false,
                }]),
            ),
        );
        registry.add_method_def(
            "User",
            test_method(
                "extracted_name",
                Type::PatternKeyRef(
                    Box::new(Type::MethodReturnRef("User".into(), "payload".into())),
                    Box::new(RecordKey::Symbol("name".to_string())),
                ),
            ),
        );

        registry.apply_global_resolution();

        let method = registry
            .lookup_method_def("User", "extracted_name", false)
            .expect("extracted method");
        assert_eq!(method.raw_return_type, Type::String);
    }

    #[test]
    fn subclass_resolution_resolves_refs_inside_records() {
        let mut registry = TypeRegistry::new();
        registry.mark_user_defined("Base");
        registry.mark_user_defined("Child");
        registry.set_superclass("Child", "Base");
        registry.add_method_def("Child", test_method("value", Type::Integer));
        registry.add_method_def(
            "Base",
            test_method(
                "snapshot",
                Type::Record(vec![RecordField {
                    key: RecordKey::Symbol("value".to_string()),
                    value: Type::MethodReturnRef("Base".into(), "value".into()),
                    optional: false,
                }]),
            ),
        );

        registry.apply_global_resolution();

        let method = registry
            .lookup_method_def("Base", "snapshot", false)
            .expect("snapshot method");
        assert_eq!(
            method.raw_return_type,
            Type::Record(vec![RecordField {
                key: RecordKey::Symbol("value".to_string()),
                value: Type::Integer,
                optional: false,
            }])
        );
    }
}
