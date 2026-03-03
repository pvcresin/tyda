use crate::sym::Sym;
use crate::types::SharedPath;
use rustc_hash::FxHashMap;
use std::collections::HashSet;
use std::sync::Arc;

type FileId = u32;
type SymbolId = u32;

/// `(kind, file_id)` packed into one word: kind in the high bits keeps the
/// sort order of the unpacked tuple, so `binary_search` still works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TypedRef(u32);

const TYPED_REF_FILE_BITS: u32 = 28;
const TYPED_REF_FILE_MASK: u32 = (1 << TYPED_REF_FILE_BITS) - 1;

impl TypedRef {
    fn new(kind: DepEdgeKind, file_id: FileId) -> Self {
        Self(((kind as u32) << TYPED_REF_FILE_BITS) | (file_id & TYPED_REF_FILE_MASK))
    }

    fn kind(self) -> Option<DepEdgeKind> {
        DepEdgeKind::from_index(self.0 >> TYPED_REF_FILE_BITS)
    }

    fn file_id(self) -> FileId {
        self.0 & TYPED_REF_FILE_MASK
    }
}

/// Separates invalidation granularity for method-body changes vs. inheritance/mixin etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DepEdgeKind {
    Superclass,
    Mixin,
    ConstantLookup,
    MethodCall,
    IvarFlow,
    RBSOverride,
    DslExpansion,
}

impl DepEdgeKind {
    fn from_index(index: u32) -> Option<Self> {
        Some(match index {
            0 => Self::Superclass,
            1 => Self::Mixin,
            2 => Self::ConstantLookup,
            3 => Self::MethodCall,
            4 => Self::IvarFlow,
            5 => Self::RBSOverride,
            6 => Self::DslExpansion,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DepEdge {
    pub symbol: String,
    pub kind: DepEdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StoredDepEdge {
    symbol_id: SymbolId,
    kind: DepEdgeKind,
}

#[derive(Debug, Default)]
pub struct DependencyGraph {
    file_ids: FxHashMap<SharedPath, FileId>,
    paths_by_id: Vec<SharedPath>,
    symbol_ids: FxHashMap<Sym, SymbolId>,
    symbols_by_id: Vec<Sym>,
    file_deps: FxHashMap<FileId, StoredFileDeps>,
    /// Indexed by `SymbolId`; only set-union is needed, so a sorted Vec has a
    /// smaller shell than HashSet, and dense indexing drops the map shell.
    symbol_definers: Vec<Vec<FileId>>,
    symbol_referencers: Vec<Vec<FileId>>,
    typed_symbol_referencers: Vec<Vec<TypedRef>>,
}

#[derive(Debug, Default, Clone)]
pub struct FileDeps {
    pub defined_symbols: HashSet<String>,
    pub referenced_symbols: HashSet<String>,
    pub edges: Vec<DepEdge>,
}

#[derive(Debug, Default, Clone)]
struct StoredFileDeps {
    defined_symbols: HashSet<SymbolId>,
    referenced_symbols: HashSet<SymbolId>,
    edges: Vec<StoredDepEdge>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    fn file_id(&self, file_path: &str) -> Option<FileId> {
        self.file_ids.get(file_path).copied()
    }

    fn get_or_create_file_id(&mut self, file_path: &str) -> FileId {
        if let Some(file_id) = self.file_id(file_path) {
            return file_id;
        }

        // `TypedRef` packs the id into `TYPED_REF_FILE_BITS`, so ids above the mask are rejected.
        let file_id: FileId = u32::try_from(self.paths_by_id.len())
            .ok()
            .filter(|id| *id <= TYPED_REF_FILE_MASK)
            .expect("too many tracked files");
        let shared: SharedPath = Arc::from(file_path);
        self.file_ids.insert(Arc::clone(&shared), file_id);
        self.paths_by_id.push(shared);
        file_id
    }

    fn symbol_id(&self, symbol: &str) -> Option<SymbolId> {
        self.symbol_ids.get(symbol).copied()
    }

    fn get_or_create_symbol_id(&mut self, symbol: &str) -> SymbolId {
        if let Some(symbol_id) = self.symbol_id(symbol) {
            return symbol_id;
        }

        let symbol_id = self
            .symbols_by_id
            .len()
            .try_into()
            .expect("too many tracked symbols");
        let sym = Sym::new(symbol);
        self.symbol_ids.insert(sym, symbol_id);
        self.symbols_by_id.push(sym);
        self.symbol_definers.push(Vec::new());
        self.symbol_referencers.push(Vec::new());
        self.typed_symbol_referencers.push(Vec::new());
        symbol_id
    }

    fn files_for_symbol(index: &[Vec<FileId>], symbol_id: SymbolId) -> &[FileId] {
        index.get(symbol_id as usize).map_or(&[], Vec::as_slice)
    }

    fn path_for_id(&self, file_id: FileId) -> Option<&str> {
        self.paths_by_id.get(file_id as usize).map(|p| &**p)
    }

    fn symbol_for_id(&self, symbol_id: SymbolId) -> Option<&str> {
        self.symbols_by_id.get(symbol_id as usize).map(Sym::as_str)
    }

    fn collect_paths<I>(&self, file_ids: I) -> HashSet<String>
    where
        I: IntoIterator<Item = FileId>,
    {
        let mut result = HashSet::new();
        for file_id in file_ids {
            if let Some(path) = self.path_for_id(file_id) {
                result.insert(path.to_string());
            }
        }
        result
    }

    fn for_each_path<I, F>(&self, file_ids: I, mut f: F)
    where
        I: IntoIterator<Item = FileId>,
        F: FnMut(&str),
    {
        for file_id in file_ids {
            if let Some(path) = self.path_for_id(file_id) {
                f(path);
            }
        }
    }

    fn dependent_file_ids(&self, symbols: &HashSet<String>) -> HashSet<FileId> {
        let mut file_ids = HashSet::new();
        for symbol in symbols {
            let Some(symbol_id) = self.symbol_id(symbol) else {
                continue;
            };
            file_ids.extend(Self::files_for_symbol(&self.symbol_referencers, symbol_id));
        }
        file_ids
    }

    fn dependent_file_ids_by_kinds(
        &self,
        symbols: &HashSet<String>,
        kinds: &[DepEdgeKind],
        include_untyped_fallback: bool,
    ) -> HashSet<FileId> {
        let mut file_ids = HashSet::new();
        for symbol in symbols {
            let Some(symbol_id) = self.symbol_id(symbol) else {
                continue;
            };
            if include_untyped_fallback {
                file_ids.extend(
                    Self::files_for_symbol(&self.symbol_referencers, symbol_id)
                        .iter()
                        .copied()
                        .filter(|file_id| {
                            self.typed_edges_of_id(*file_id).is_none_or(Vec::is_empty)
                        }),
                );
            }
            if let Some(refs) = self.typed_symbol_referencers.get(symbol_id as usize) {
                file_ids.extend(
                    refs.iter()
                        .filter(|typed| typed.kind().is_some_and(|kind| kinds.contains(&kind)))
                        .map(|typed| typed.file_id()),
                );
            }
        }
        file_ids
    }

    fn definer_file_ids(&self, symbols: &HashSet<String>) -> HashSet<FileId> {
        let mut file_ids = HashSet::new();
        for symbol in symbols {
            let Some(symbol_id) = self.symbol_id(symbol) else {
                continue;
            };
            file_ids.extend(Self::files_for_symbol(&self.symbol_definers, symbol_id));
        }
        file_ids
    }

    fn insert_symbol_file(index: &mut [Vec<FileId>], symbol_id: SymbolId, file_id: FileId) {
        let Some(files) = index.get_mut(symbol_id as usize) else {
            return;
        };
        if let Err(pos) = files.binary_search(&file_id) {
            files.insert(pos, file_id);
        }
    }

    fn remove_symbol_file(index: &mut [Vec<FileId>], symbol_id: SymbolId, file_id: FileId) {
        let Some(files) = index.get_mut(symbol_id as usize) else {
            return;
        };
        if let Ok(pos) = files.binary_search(&file_id) {
            files.remove(pos);
        }
        if files.is_empty() {
            files.shrink_to_fit();
        }
    }

    fn insert_typed_symbol_file(
        &mut self,
        symbol_id: SymbolId,
        kind: DepEdgeKind,
        file_id: FileId,
    ) {
        let Some(refs) = self.typed_symbol_referencers.get_mut(symbol_id as usize) else {
            return;
        };
        let typed = TypedRef::new(kind, file_id);
        if let Err(pos) = refs.binary_search(&typed) {
            refs.insert(pos, typed);
        }
    }

    fn remove_typed_symbol_file(
        &mut self,
        symbol_id: SymbolId,
        kind: DepEdgeKind,
        file_id: FileId,
    ) {
        let Some(refs) = self.typed_symbol_referencers.get_mut(symbol_id as usize) else {
            return;
        };
        if let Ok(pos) = refs.binary_search(&TypedRef::new(kind, file_id)) {
            refs.remove(pos);
        }
        if refs.is_empty() {
            refs.shrink_to_fit();
        }
    }

    fn store_file_deps(&mut self, deps: FileDeps) -> StoredFileDeps {
        let mut stored = StoredFileDeps::default();
        for symbol in deps.defined_symbols {
            stored
                .defined_symbols
                .insert(self.get_or_create_symbol_id(&symbol));
        }
        for symbol in deps.referenced_symbols {
            stored
                .referenced_symbols
                .insert(self.get_or_create_symbol_id(&symbol));
        }
        stored.edges = deps
            .edges
            .into_iter()
            .map(|edge| StoredDepEdge {
                symbol_id: self.get_or_create_symbol_id(&edge.symbol),
                kind: edge.kind,
            })
            .collect();
        stored
    }

    fn clear_file(&mut self, file_id: FileId) {
        if let Some(old_deps) = self.file_deps.remove(&file_id) {
            for sym in &old_deps.defined_symbols {
                Self::remove_symbol_file(&mut self.symbol_definers, *sym, file_id);
            }

            for sym in &old_deps.referenced_symbols {
                Self::remove_symbol_file(&mut self.symbol_referencers, *sym, file_id);
            }

            for edge in &old_deps.edges {
                self.remove_typed_symbol_file(edge.symbol_id, edge.kind, file_id);
            }
        }
    }

    pub fn update_file(&mut self, file_path: &str, deps: FileDeps) {
        let file_id = self.get_or_create_file_id(file_path);
        self.clear_file(file_id);
        let deps = self.store_file_deps(deps);

        for sym in &deps.defined_symbols {
            Self::insert_symbol_file(&mut self.symbol_definers, *sym, file_id);
        }

        for sym in &deps.referenced_symbols {
            Self::insert_symbol_file(&mut self.symbol_referencers, *sym, file_id);
        }

        if !deps.edges.is_empty() {
            for edge in &deps.edges {
                self.insert_typed_symbol_file(edge.symbol_id, edge.kind, file_id);
            }
        }

        self.file_deps.insert(file_id, deps);
    }

    pub fn remove_file(&mut self, file_path: &str) {
        if let Some(file_id) = self.file_id(file_path) {
            self.clear_file(file_id);
        }
    }

    pub fn dependents_of(&self, symbols: &HashSet<String>) -> HashSet<String> {
        self.collect_paths(self.dependent_file_ids(symbols))
    }

    pub fn for_each_dependent_path<F>(&self, symbols: &HashSet<String>, f: F)
    where
        F: FnMut(&str),
    {
        self.for_each_path(self.dependent_file_ids(symbols), f);
    }

    /// Falls back to untyped `references` only for files that have no typed edges yet.
    pub fn dependents_of_by_kind(
        &self,
        symbols: &HashSet<String>,
        kinds: &HashSet<DepEdgeKind>,
    ) -> HashSet<String> {
        let kinds: Vec<_> = kinds.iter().copied().collect();
        self.collect_paths(self.dependent_file_ids_by_kinds(symbols, &kinds, true))
    }

    /// Does not treat an untyped ref that is only a namespace reopen as a method-call dependency.
    pub fn dependents_of_by_kind_strict(
        &self,
        symbols: &HashSet<String>,
        kinds: &HashSet<DepEdgeKind>,
    ) -> HashSet<String> {
        let kinds: Vec<_> = kinds.iter().copied().collect();
        self.collect_paths(self.dependent_file_ids_by_kinds(symbols, &kinds, false))
    }

    pub fn for_each_dependent_path_by_kinds_strict<F>(
        &self,
        symbols: &HashSet<String>,
        kinds: &[DepEdgeKind],
        f: F,
    ) where
        F: FnMut(&str),
    {
        self.for_each_path(self.dependent_file_ids_by_kinds(symbols, kinds, false), f);
    }

    fn typed_edges_of_id(&self, file_id: FileId) -> Option<&Vec<StoredDepEdge>> {
        self.file_deps.get(&file_id).map(|deps| &deps.edges)
    }

    pub fn for_each_typed_edge<F>(&self, file_path: &str, mut f: F)
    where
        F: FnMut(&str, DepEdgeKind),
    {
        let Some(file_id) = self.file_id(file_path) else {
            return;
        };
        let Some(edges) = self.typed_edges_of_id(file_id) else {
            return;
        };
        for edge in edges {
            if let Some(symbol) = self.symbol_for_id(edge.symbol_id) {
                f(symbol, edge.kind);
            }
        }
    }

    pub fn typed_edges_of(&self, file_path: &str) -> Option<Vec<DepEdge>> {
        let file_id = self.file_id(file_path)?;
        let edges = self.typed_edges_of_id(file_id)?;
        Some(
            edges
                .iter()
                .filter_map(|edge| {
                    Some(DepEdge {
                        symbol: self.symbol_for_id(edge.symbol_id)?.to_string(),
                        kind: edge.kind,
                    })
                })
                .collect(),
        )
    }

    /// Deep heap estimate used for memory attribution.
    pub fn deep_bytes(&self) -> usize {
        fn shell(len: usize, kv: usize) -> usize {
            if len == 0 {
                0
            } else {
                (len * 8 / 7 + 1).next_power_of_two() * (kv + 1) + 48
            }
        }
        let mut bytes = shell(
            self.file_ids.len(),
            std::mem::size_of::<(SharedPath, FileId)>(),
        );
        bytes += self.paths_by_id.len() * std::mem::size_of::<SharedPath>();
        bytes += self.paths_by_id.iter().map(|p| p.len() + 16).sum::<usize>();
        bytes += shell(
            self.symbol_ids.len(),
            std::mem::size_of::<(Sym, SymbolId)>(),
        );
        bytes += self.symbols_by_id.len() * std::mem::size_of::<Sym>();
        bytes += shell(
            self.file_deps.len(),
            std::mem::size_of::<(FileId, StoredFileDeps)>(),
        );
        for deps in self.file_deps.values() {
            bytes += shell(deps.defined_symbols.len(), std::mem::size_of::<SymbolId>());
            bytes += shell(
                deps.referenced_symbols.len(),
                std::mem::size_of::<SymbolId>(),
            );
            bytes += deps.edges.len() * std::mem::size_of::<StoredDepEdge>();
        }
        for index in [&self.symbol_definers, &self.symbol_referencers] {
            bytes += index.len() * std::mem::size_of::<Vec<FileId>>();
            for files in index {
                bytes += std::mem::size_of_val(files.as_slice());
            }
        }
        bytes += self.typed_symbol_referencers.len() * std::mem::size_of::<Vec<TypedRef>>();
        for refs in &self.typed_symbol_referencers {
            bytes += std::mem::size_of_val(refs.as_slice());
        }
        bytes
    }

    pub fn has_definitions(&self, file_path: &str) -> bool {
        self.file_id(file_path)
            .and_then(|file_id| self.file_deps.get(&file_id))
            .is_some_and(|deps| !deps.defined_symbols.is_empty())
    }

    pub fn has_references(&self, file_path: &str) -> bool {
        self.file_id(file_path)
            .and_then(|file_id| self.file_deps.get(&file_id))
            .is_some_and(|deps| !deps.referenced_symbols.is_empty())
    }

    pub fn for_each_definition_symbol<F>(&self, file_path: &str, mut f: F)
    where
        F: FnMut(&str),
    {
        let Some(file_id) = self.file_id(file_path) else {
            return;
        };
        let Some(deps) = self.file_deps.get(&file_id) else {
            return;
        };
        for symbol_id in &deps.defined_symbols {
            if let Some(symbol) = self.symbol_for_id(*symbol_id) {
                f(symbol);
            }
        }
    }

    pub fn definitions_of(&self, file_path: &str) -> Option<HashSet<String>> {
        let file_id = self.file_id(file_path)?;
        let deps = self.file_deps.get(&file_id)?;
        Some(
            deps.defined_symbols
                .iter()
                .filter_map(|symbol_id| self.symbol_for_id(*symbol_id).map(str::to_string))
                .collect(),
        )
    }

    pub fn for_each_reference_symbol<F>(&self, file_path: &str, mut f: F)
    where
        F: FnMut(&str),
    {
        let Some(file_id) = self.file_id(file_path) else {
            return;
        };
        let Some(deps) = self.file_deps.get(&file_id) else {
            return;
        };
        for symbol_id in &deps.referenced_symbols {
            if let Some(symbol) = self.symbol_for_id(*symbol_id) {
                f(symbol);
            }
        }
    }

    pub fn references_of(&self, file_path: &str) -> Option<HashSet<String>> {
        let file_id = self.file_id(file_path)?;
        let deps = self.file_deps.get(&file_id)?;
        Some(
            deps.referenced_symbols
                .iter()
                .filter_map(|symbol_id| self.symbol_for_id(*symbol_id).map(str::to_string))
                .collect(),
        )
    }

    pub fn definers_of(&self, symbols: &HashSet<String>) -> HashSet<String> {
        self.collect_paths(self.definer_file_ids(symbols))
    }

    pub fn for_each_definer_path<F>(&self, symbols: &HashSet<String>, f: F)
    where
        F: FnMut(&str),
    {
        self.for_each_path(self.definer_file_ids(symbols), f);
    }

    pub fn all_files(&self) -> HashSet<String> {
        self.collect_paths(self.file_deps.keys().copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_KINDS: [DepEdgeKind; 7] = [
        DepEdgeKind::Superclass,
        DepEdgeKind::Mixin,
        DepEdgeKind::ConstantLookup,
        DepEdgeKind::MethodCall,
        DepEdgeKind::IvarFlow,
        DepEdgeKind::RBSOverride,
        DepEdgeKind::DslExpansion,
    ];

    #[test]
    fn test_typed_ref_packing_roundtrip() {
        for kind in ALL_KINDS {
            for file_id in [0, 1, 12_724, TYPED_REF_FILE_MASK] {
                let packed = TypedRef::new(kind, file_id);
                assert_eq!(packed.kind(), Some(kind));
                assert_eq!(packed.file_id(), file_id);
            }
        }
    }

    #[test]
    fn test_typed_ref_packing_keeps_tuple_order() {
        let mut packed: Vec<TypedRef> = ALL_KINDS
            .iter()
            .flat_map(|kind| [0, 7, TYPED_REF_FILE_MASK].map(|file| TypedRef::new(*kind, file)))
            .collect();
        packed.sort();
        let unpacked: Vec<(DepEdgeKind, FileId)> = packed
            .iter()
            .filter_map(|typed| Some((typed.kind()?, typed.file_id())))
            .collect();
        let mut expected = unpacked.clone();
        expected.sort();
        assert_eq!(unpacked, expected, "binary_search relies on this order");
    }

    #[test]
    fn test_basic_dependency_tracking() {
        let mut graph = DependencyGraph::new();

        graph.update_file(
            "user.rb",
            FileDeps {
                defined_symbols: HashSet::from(["User".to_string()]),
                ..Default::default()
            },
        );
        graph.update_file(
            "controller.rb",
            FileDeps {
                defined_symbols: HashSet::from(["UserController".to_string()]),
                referenced_symbols: HashSet::from(["User".to_string()]),
                ..Default::default()
            },
        );
        graph.update_file(
            "admin.rb",
            FileDeps {
                defined_symbols: HashSet::from(["Admin".to_string()]),
                referenced_symbols: HashSet::from(["User".to_string(), "Role".to_string()]),
                ..Default::default()
            },
        );

        let changed = HashSet::from(["User".to_string()]);
        let deps = graph.dependents_of(&changed);
        assert!(deps.contains("controller.rb"));
        assert!(deps.contains("admin.rb"));
        assert!(!deps.contains("user.rb"));
    }

    #[test]
    fn test_update_replaces_old_data() {
        let mut graph = DependencyGraph::new();

        graph.update_file(
            "a.rb",
            FileDeps {
                defined_symbols: HashSet::from(["Foo".to_string()]),
                ..Default::default()
            },
        );
        let expected_foo = HashSet::from(["Foo".to_string()]);
        assert_eq!(graph.definitions_of("a.rb"), Some(expected_foo));

        graph.update_file(
            "a.rb",
            FileDeps {
                defined_symbols: HashSet::from(["Bar".to_string()]),
                ..Default::default()
            },
        );
        let expected_bar = HashSet::from(["Bar".to_string()]);
        assert_eq!(graph.definitions_of("a.rb"), Some(expected_bar));
        let foo_id = graph.symbol_id("Foo").expect("Foo should stay interned");
        assert!(graph.symbol_definers[foo_id as usize].is_empty());
    }

    #[test]
    fn test_remove_file() {
        let mut graph = DependencyGraph::new();
        graph.update_file(
            "a.rb",
            FileDeps {
                defined_symbols: HashSet::from(["X".to_string()]),
                referenced_symbols: HashSet::from(["Y".to_string()]),
                ..Default::default()
            },
        );
        assert!(graph.all_files().contains("a.rb"));

        graph.remove_file("a.rb");
        assert!(!graph.all_files().contains("a.rb"));
        let x_id = graph.symbol_id("X").expect("X should stay interned");
        assert!(graph.symbol_definers[x_id as usize].is_empty());
    }

    #[test]
    fn test_no_false_positives() {
        let mut graph = DependencyGraph::new();
        graph.update_file(
            "a.rb",
            FileDeps {
                defined_symbols: HashSet::from(["A".to_string()]),
                referenced_symbols: HashSet::from(["B".to_string()]),
                ..Default::default()
            },
        );
        graph.update_file(
            "b.rb",
            FileDeps {
                defined_symbols: HashSet::from(["B".to_string()]),
                referenced_symbols: HashSet::from(["C".to_string()]),
                ..Default::default()
            },
        );

        let changed = HashSet::from(["A".to_string()]);
        let deps = graph.dependents_of(&changed);
        assert!(deps.is_empty(), "b.rb does not reference A");
    }

    #[test]
    fn test_typed_edges_basic() {
        let mut graph = DependencyGraph::new();
        graph.update_file(
            "base.rb",
            FileDeps {
                defined_symbols: HashSet::from(["Base".to_string()]),
                ..Default::default()
            },
        );
        graph.update_file(
            "child.rb",
            FileDeps {
                defined_symbols: HashSet::from(["Child".to_string()]),
                referenced_symbols: HashSet::from(["Base".to_string()]),
                edges: vec![DepEdge {
                    symbol: "Base".to_string(),
                    kind: DepEdgeKind::Superclass,
                }],
            },
        );
        graph.update_file(
            "caller.rb",
            FileDeps {
                defined_symbols: HashSet::from(["Caller".to_string()]),
                referenced_symbols: HashSet::from(["Base".to_string()]),
                edges: vec![DepEdge {
                    symbol: "Base".to_string(),
                    kind: DepEdgeKind::MethodCall,
                }],
            },
        );

        let changed = HashSet::from(["Base".to_string()]);

        let all_deps = graph.dependents_of(&changed);
        assert!(all_deps.contains("child.rb"));
        assert!(all_deps.contains("caller.rb"));

        let superclass_only =
            graph.dependents_of_by_kind(&changed, &HashSet::from([DepEdgeKind::Superclass]));
        assert!(superclass_only.contains("child.rb"));
        assert!(!superclass_only.contains("caller.rb"));

        let method_call_only =
            graph.dependents_of_by_kind(&changed, &HashSet::from([DepEdgeKind::MethodCall]));
        assert!(!method_call_only.contains("child.rb"));
        assert!(method_call_only.contains("caller.rb"));
    }

    #[test]
    fn test_strict_kind_filter_ignores_untyped_namespace_refs() {
        let mut graph = DependencyGraph::new();
        graph.update_file(
            "ml.rb",
            FileDeps {
                defined_symbols: HashSet::from(["Ml".to_string()]),
                ..Default::default()
            },
        );
        graph.update_file(
            "ml/model.rb",
            FileDeps {
                defined_symbols: HashSet::from(["Ml::Model".to_string()]),
                referenced_symbols: HashSet::from(["Ml".to_string()]),
                edges: Vec::new(),
            },
        );

        let changed = HashSet::from(["Ml".to_string()]);
        let deps =
            graph.dependents_of_by_kind_strict(&changed, &HashSet::from([DepEdgeKind::MethodCall]));
        assert!(
            deps.is_empty(),
            "namespace-only reopen files should not count as method-call dependents"
        );
    }

    #[test]
    fn test_typed_edges_removed_on_file_removal() {
        let mut graph = DependencyGraph::new();
        graph.update_file(
            "a.rb",
            FileDeps {
                defined_symbols: HashSet::from(["A".to_string()]),
                referenced_symbols: HashSet::from(["B".to_string()]),
                edges: vec![DepEdge {
                    symbol: "B".to_string(),
                    kind: DepEdgeKind::Mixin,
                }],
            },
        );
        assert_eq!(
            graph.typed_edges_of("a.rb").map_or(0, |edges| edges.len()),
            1
        );

        graph.remove_file("a.rb");
        assert!(graph.typed_edges_of("a.rb").is_none());
    }

    #[test]
    fn test_untyped_fallback_clears_when_file_gains_typed_edges() {
        let mut graph = DependencyGraph::new();
        graph.update_file(
            "consumer.rb",
            FileDeps {
                referenced_symbols: HashSet::from(["Base".to_string()]),
                ..Default::default()
            },
        );

        let changed = HashSet::from(["Base".to_string()]);
        let fallback =
            graph.dependents_of_by_kind(&changed, &HashSet::from([DepEdgeKind::MethodCall]));
        assert!(fallback.contains("consumer.rb"));

        graph.update_file(
            "consumer.rb",
            FileDeps {
                referenced_symbols: HashSet::from(["Base".to_string()]),
                edges: vec![DepEdge {
                    symbol: "Base".to_string(),
                    kind: DepEdgeKind::Superclass,
                }],
                ..Default::default()
            },
        );

        let method_only =
            graph.dependents_of_by_kind(&changed, &HashSet::from([DepEdgeKind::MethodCall]));
        assert!(
            !method_only.contains("consumer.rb"),
            "typed edges should stop falling back to untyped references"
        );
    }
}
