mod display_scope;
mod fingerprints;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustc_hash::FxHashMap;

use crate::dep_graph::{DependencyGraph, FileDeps};
use crate::inference::FileAnalysisSnapshot;
use crate::registry::TypeRegistry;

type FileId = u32;
use self::display_scope::DisplayBaseRegistryCache;
// Only the LSP consumes this re-export; gate it so the minimal wasm build
// doesn't flag it as an unused import.
#[cfg(feature = "lsp")]
pub(crate) use self::display_scope::DisplayScopeKey;
pub use self::fingerprints::{
    ExportFingerprint, FileFingerprints, RegistryFingerprint, hash_content,
};
use self::fingerprints::{FingerprintAggregate, hash_u64};

#[derive(Debug, Clone, Copy)]
struct UpsertFileOptions {
    on_disk_stamp: Option<FileStamp>,
    fingerprints: FileFingerprints,
    skip_dependent_invalidation: bool,
}

pub struct WorkspaceFileEntry {
    pub content_hash: u64,
    pub analysis: FileAnalysisSnapshot,
    pub export_fingerprint: ExportFingerprint,
    pub registry_fingerprint: RegistryFingerprint,
    registry_fingerprint_hash: u64,
    pub on_disk_stamp: Option<FileStamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileStamp {
    modified_nanos: u128,
    len: u64,
}

impl FileStamp {
    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        let modified = metadata.modified().ok()?;
        let modified_nanos = modified
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos();
        Some(Self {
            modified_nanos,
            len: metadata.len(),
        })
    }
}

/// Timing for workspace-level operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkspaceTimings {
    pub registry_build: Duration,
    pub propagate: Duration,
    pub current_file_solve: Duration,
    #[cfg(test)]
    pub display_merge: Duration,
    #[cfg(test)]
    pub display_clone: Duration,
    #[cfg(test)]
    pub display_base_cache_hit: bool,
    #[cfg(test)]
    pub analysis_timings: crate::analysis::AnalysisTimings,
    #[cfg(test)]
    pub finalize_call_site_summaries: Duration,
}

/// Registry resolution profile for CLI batch vs. LSP (merge logic is shared).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionProfile {
    // CLI: call-site propagation + freezing the param cache makes parallel render deterministic; long-lived shrink is skipped.
    Batch,
    // LSP: full global resolution plus shrink / transient drop for long-lived Arcs.
    Interactive,
}

pub struct WorkspaceState {
    file_ids: FxHashMap<String, FileId>,
    paths_by_id: Vec<String>,
    files: FxHashMap<FileId, WorkspaceFileEntry>,
    dep_graph: DependencyGraph,
    pending_scan_files: HashSet<FileId>,
    cached_registry: Option<Arc<TypeRegistry>>,
    dirty_files: HashSet<FileId>,
    cached_display_base_registry: Option<DisplayBaseRegistryCache>,
    registry_fingerprint_aggregate: FingerprintAggregate,
    registry_version: u64,
    pub last_timings: WorkspaceTimings,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceState {
    fn workspace_file_id(&self, file_path: &str) -> Option<FileId> {
        self.file_ids.get(file_path).copied()
    }

    fn ensure_workspace_file_id(&mut self, file_path: &str) -> FileId {
        if let Some(file_id) = self.workspace_file_id(file_path) {
            return file_id;
        }

        let file_id = self
            .paths_by_id
            .len()
            .try_into()
            .expect("too many workspace files");
        self.file_ids.insert(file_path.to_string(), file_id);
        self.paths_by_id.push(file_path.to_string());
        file_id
    }

    fn workspace_file_path(&self, file_id: FileId) -> Option<&str> {
        self.paths_by_id.get(file_id as usize).map(String::as_str)
    }

    fn remove_workspace_file_entry_by_id(&mut self, file_id: FileId) -> Option<WorkspaceFileEntry> {
        let entry = self.files.remove(&file_id)?;
        self.registry_fingerprint_aggregate
            .remove_hash(entry.registry_fingerprint_hash);
        self.dirty_files.remove(&file_id);
        Some(entry)
    }

    fn collect_file_paths(&self, file_ids: &HashSet<FileId>) -> HashSet<String> {
        let mut paths = HashSet::new();
        for file_id in file_ids {
            if let Some(path) = self.workspace_file_path(*file_id) {
                paths.insert(path.to_string());
            }
        }
        paths
    }

    fn collect_dependent_file_ids(&self, symbols: &HashSet<String>) -> HashSet<FileId> {
        let mut dependent_ids = HashSet::new();
        self.dep_graph
            .for_each_dependent_path(symbols, |file_path| {
                if let Some(file_id) = self.workspace_file_id(file_path) {
                    dependent_ids.insert(file_id);
                }
            });
        dependent_ids
    }

    fn collect_dependent_file_paths(&self, symbols: &HashSet<String>) -> HashSet<String> {
        let dependent_ids = self.collect_dependent_file_ids(symbols);
        self.collect_file_paths(&dependent_ids)
    }

    fn mark_dependents_dirty_and_collect_paths(
        &mut self,
        symbols: &HashSet<String>,
    ) -> HashSet<String> {
        let dependent_ids = self.collect_dependent_file_ids(symbols);
        self.dirty_files.extend(dependent_ids.iter().copied());
        self.collect_file_paths(&dependent_ids)
    }

    pub fn new() -> Self {
        Self {
            file_ids: FxHashMap::default(),
            paths_by_id: Vec::new(),
            files: FxHashMap::default(),
            dep_graph: DependencyGraph::new(),
            pending_scan_files: HashSet::new(),
            cached_registry: None,
            dirty_files: HashSet::new(),
            cached_display_base_registry: None,
            registry_fingerprint_aggregate: FingerprintAggregate::default(),
            registry_version: 0,
            last_timings: WorkspaceTimings::default(),
        }
    }

    pub fn dep_graph(&self) -> &DependencyGraph {
        &self.dep_graph
    }

    pub fn dep_graph_mut(&mut self) -> &mut DependencyGraph {
        &mut self.dep_graph
    }

    pub fn workspace_file(&self, file_path: &str) -> Option<&WorkspaceFileEntry> {
        self.workspace_file_id(file_path)
            .and_then(|file_id| self.files.get(&file_id))
    }

    pub fn file_paths(&self) -> impl Iterator<Item = &str> {
        self.files
            .keys()
            .filter_map(|file_id| self.workspace_file_path(*file_id))
    }

    pub fn contains_file(&self, file_path: &str) -> bool {
        self.workspace_file_id(file_path)
            .is_some_and(|file_id| self.files.contains_key(&file_id))
    }

    pub fn is_file_dirty(&self, file_path: &str) -> bool {
        self.workspace_file_id(file_path)
            .is_some_and(|file_id| self.dirty_files.contains(&file_id))
    }

    pub fn excluding_fingerprint(&self, exclude_file: &str) -> u64 {
        let excluded_hash = self
            .workspace_file(exclude_file)
            .map(|entry| entry.registry_fingerprint_hash);
        self.registry_fingerprint_aggregate
            .fingerprint_excluding(excluded_hash)
    }

    pub fn upsert_file(
        &mut self,
        file_path: String,
        content_hash: u64,
        analysis: FileAnalysisSnapshot,
        deps: FileDeps,
    ) -> HashSet<String> {
        let fingerprints = FileFingerprints::from_analysis(&analysis);
        self.upsert_file_with_stamp_and_fingerprints(
            file_path,
            content_hash,
            analysis,
            deps,
            None,
            fingerprints,
        )
    }

    pub fn upsert_file_with_stamp(
        &mut self,
        file_path: String,
        content_hash: u64,
        analysis: FileAnalysisSnapshot,
        deps: FileDeps,
        on_disk_stamp: Option<FileStamp>,
    ) -> HashSet<String> {
        let fingerprints = FileFingerprints::from_analysis(&analysis);
        self.upsert_file_with_stamp_and_fingerprints(
            file_path,
            content_hash,
            analysis,
            deps,
            on_disk_stamp,
            fingerprints,
        )
    }

    pub fn upsert_file_with_stamp_and_fingerprints(
        &mut self,
        file_path: String,
        content_hash: u64,
        analysis: FileAnalysisSnapshot,
        deps: FileDeps,
        on_disk_stamp: Option<FileStamp>,
        fingerprints: FileFingerprints,
    ) -> HashSet<String> {
        self.upsert_file_with_stamp_and_fingerprints_impl(
            file_path,
            content_hash,
            analysis,
            deps,
            UpsertFileOptions {
                on_disk_stamp,
                fingerprints,
                skip_dependent_invalidation: false,
            },
        )
    }

    pub fn upsert_scanned_file_with_stamp_and_fingerprints(
        &mut self,
        file_path: String,
        content_hash: u64,
        analysis: FileAnalysisSnapshot,
        deps: FileDeps,
        on_disk_stamp: Option<FileStamp>,
        fingerprints: FileFingerprints,
    ) -> HashSet<String> {
        self.upsert_file_with_stamp_and_fingerprints_impl(
            file_path,
            content_hash,
            analysis,
            deps,
            UpsertFileOptions {
                on_disk_stamp,
                fingerprints,
                skip_dependent_invalidation: true,
            },
        )
    }

    fn upsert_file_with_stamp_and_fingerprints_impl(
        &mut self,
        file_path: String,
        content_hash: u64,
        analysis: FileAnalysisSnapshot,
        deps: FileDeps,
        options: UpsertFileOptions,
    ) -> HashSet<String> {
        let UpsertFileOptions {
            on_disk_stamp,
            fingerprints,
            skip_dependent_invalidation,
        } = options;
        let file_id = self.ensure_workspace_file_id(&file_path);
        let old_fingerprint = self
            .files
            .get(&file_id)
            .map(|entry| entry.export_fingerprint);
        let old_registry_fingerprint = self
            .files
            .get(&file_id)
            .map(|entry| entry.registry_fingerprint);
        let exports_changed = old_fingerprint != Some(fingerprints.export);
        let registry_changed = old_registry_fingerprint != Some(fingerprints.registry);

        self.dep_graph.update_file(&file_path, deps);
        let registry_fingerprint_hash = hash_u64(&fingerprints.registry);
        if let Some(old_entry) = self.files.insert(
            file_id,
            WorkspaceFileEntry {
                content_hash,
                analysis,
                export_fingerprint: fingerprints.export,
                registry_fingerprint: fingerprints.registry,
                registry_fingerprint_hash,
                on_disk_stamp,
            },
        ) {
            self.registry_fingerprint_aggregate
                .remove_hash(old_entry.registry_fingerprint_hash);
        }
        self.registry_fingerprint_aggregate
            .add_hash(registry_fingerprint_hash);
        if registry_changed {
            self.cached_registry = None;
        }

        if exports_changed {
            if skip_dependent_invalidation {
                return HashSet::new();
            }
            self.dirty_files.insert(file_id);
            let changed_symbols = self.dep_graph.definitions_of(&file_path);
            changed_symbols
                .as_ref()
                .map(|symbols| self.mark_dependents_dirty_and_collect_paths(symbols))
                .unwrap_or_default()
        } else {
            self.dirty_files.remove(&file_id);
            HashSet::new()
        }
    }

    // Keeps the stale entry around for export-fingerprint comparison, avoiding a full rebuild on every keystroke.
    pub fn mark_file_dirty(&mut self, file_path: &str) {
        if let Some(file_id) = self.workspace_file_id(file_path)
            && self.files.contains_key(&file_id)
        {
            self.dirty_files.insert(file_id);
        }
    }

    pub fn mark_file_pending_scan(&mut self, file_path: String) {
        let file_id = self.ensure_workspace_file_id(&file_path);
        self.pending_scan_files.insert(file_id);
    }

    pub fn remove_pending_scan_file(&mut self, file_path: &str) {
        if let Some(file_id) = self.workspace_file_id(file_path) {
            self.pending_scan_files.remove(&file_id);
        }
    }

    pub fn pending_scan_files(&self) -> impl Iterator<Item = &str> {
        self.pending_scan_files
            .iter()
            .filter_map(|file_id| self.workspace_file_path(*file_id))
    }

    pub fn clear_pending_scan_files(&mut self) {
        self.pending_scan_files.clear();
    }

    pub fn remove_file(&mut self, file_path: &str) {
        let changed_symbols = self.dep_graph.definitions_of(file_path);
        let dependents = changed_symbols
            .as_ref()
            .map(|symbols| self.collect_dependent_file_paths(symbols))
            .unwrap_or_default();
        let file_id = self.workspace_file_id(file_path);
        self.dep_graph.remove_file(file_path);
        if let Some(file_id) = file_id {
            self.remove_workspace_file_entry_by_id(file_id);
            self.pending_scan_files.remove(&file_id);
        }
        self.invalidate_registry();
        for dep in &dependents {
            if let Some(dep_id) = self.workspace_file_id(dep) {
                self.dirty_files.insert(dep_id);
            }
        }
    }

    pub fn invalidate_registry(&mut self) {
        self.cached_registry = None;
        self.cached_display_base_registry = None;
    }

    pub fn invalidate_all(&mut self) {
        self.cached_registry = None;
        self.cached_display_base_registry = None;
        self.dirty_files = self.files.keys().copied().collect();
    }

    pub fn registry_version(&self) -> u64 {
        self.registry_version
    }

    pub fn workspace_registry(&mut self, user_rbs: &TypeRegistry) -> Arc<TypeRegistry> {
        if let Some(ref cached) = self.cached_registry
            && self.dirty_files.is_empty()
        {
            return Arc::clone(cached);
        }

        let t0 = Instant::now();
        let mut registry = TypeRegistry::new_pooled();
        registry.merge_rbs_registry(user_rbs);
        for entry in self.files.values() {
            entry.analysis.apply_to_registry(&mut registry);
        }
        let t_merge = t0.elapsed();

        let t1 = Instant::now();
        Self::resolve_with_profile(&mut registry, ResolutionProfile::Interactive, None);
        let t_resolve = t1.elapsed();

        self.last_timings.registry_build = t_merge + t_resolve;
        self.last_timings.propagate = t_resolve;
        self.dirty_files.clear();
        self.registry_version += 1;

        let arc = Arc::new(registry);
        self.cached_registry = Some(Arc::clone(&arc));

        arc
    }

    // CLI batch: skips fingerprint/dep-graph since there's no incremental mode (avoids the walk cost over tens of thousands of files).
    pub fn push_batch_file(&mut self, file_path: String, analysis: FileAnalysisSnapshot) {
        let file_id = self.ensure_workspace_file_id(&file_path);
        self.files.insert(
            file_id,
            WorkspaceFileEntry {
                content_hash: 0,
                analysis,
                export_fingerprint: ExportFingerprint(0),
                registry_fingerprint: RegistryFingerprint(0),
                registry_fingerprint_hash: 0,
                on_disk_stamp: None,
            },
        );
    }

    pub fn batch_file_count(&self) -> usize {
        self.files.len()
    }

    #[cfg(test)]
    pub fn snapshots_deep_breakdown(
        &self,
        seen: &mut rustc_hash::FxHashSet<usize>,
    ) -> crate::registry::RegistryDeepBytes {
        let mut total = crate::registry::RegistryDeepBytes::default();
        for entry in self.files.values() {
            total.accumulate(&entry.analysis.deep_breakdown(seen));
        }
        total
    }

    #[cfg(test)]
    pub fn display_base_registry_for_breakdown(&self) -> Option<Arc<TypeRegistry>> {
        self.cached_display_base_registry()
    }

    #[cfg(test)]
    pub fn drop_all_file_snapshots_for_breakdown(&mut self) {
        for entry in self.files.values_mut() {
            entry.analysis = FileAnalysisSnapshot::empty();
        }
    }

    #[cfg(test)]
    pub fn drop_cached_registry_for_breakdown(&mut self) {
        self.cached_registry = None;
        self.cached_display_base_registry = None;
    }

    pub fn dep_graph_deep_bytes(&self) -> usize {
        self.dep_graph.deep_bytes()
    }

    pub fn batch_projection(
        &self,
        base: &TypeRegistry,
        pool: Option<&rayon::ThreadPool>,
    ) -> Arc<TypeRegistry> {
        let mut registry = base.clone();
        for file_id in 0..self.paths_by_id.len() as FileId {
            if let Some(entry) = self.files.get(&file_id) {
                entry.analysis.apply_to_registry(&mut registry);
            }
        }
        Self::resolve_with_profile(&mut registry, ResolutionProfile::Batch, pool);
        Arc::new(registry)
    }

    /// Overlay file-local facts onto `base` without retaining a second snapshot
    /// copy. The clone lives for one file: `compact_file_local_facts` drops
    /// loc-less synthetics that judgment still needs on the borrowed snapshot.
    pub fn project_borrowed_snapshots<'a>(
        mut base: TypeRegistry,
        snapshots: impl IntoIterator<Item = &'a FileAnalysisSnapshot>,
        pool: Option<&rayon::ThreadPool>,
    ) -> Arc<TypeRegistry> {
        for snapshot in snapshots {
            let mut overlay = snapshot.clone();
            overlay.compact_file_local_facts();
            overlay.strip_base_context();
            overlay.apply_to_registry(&mut base);
        }
        Self::resolve_with_profile(&mut base, ResolutionProfile::Batch, pool);
        Arc::new(base)
    }

    fn resolve_with_profile(
        registry: &mut TypeRegistry,
        profile: ResolutionProfile,
        pool: Option<&rayon::ThreadPool>,
    ) {
        match profile {
            ResolutionProfile::Batch => {
                let run = |registry: &mut TypeRegistry| {
                    registry.apply_cli_resolution();
                    registry.prewarm_and_freeze_resolve_params();
                    registry.compact_after_batch_freeze();
                };
                match pool {
                    Some(pool) => pool.install(|| run(registry)),
                    None => run(registry),
                }
            }
            ResolutionProfile::Interactive => {
                let run = |registry: &mut TypeRegistry| {
                    registry.apply_global_resolution();
                    registry.shrink_to_fit_after_compact();
                    registry.drop_transient_collection_state();
                };
                match pool {
                    Some(pool) => pool.install(|| run(registry)),
                    None => run(registry),
                }
            }
        }
    }

    pub fn invalidate_dependents_of(&mut self, symbols: &HashSet<String>) -> HashSet<String> {
        let dependents = self.collect_dependent_file_paths(symbols);
        for dep in &dependents {
            if let Some(file_id) = self.workspace_file_id(dep) {
                self.remove_workspace_file_entry_by_id(file_id);
                self.dirty_files.insert(file_id);
            }
        }
        self.invalidate_registry();
        dependents
    }

    pub fn clear(&mut self) {
        self.file_ids.clear();
        self.paths_by_id.clear();
        self.files.clear();
        self.dep_graph = DependencyGraph::new();
        self.pending_scan_files.clear();
        self.cached_registry = None;
        self.cached_display_base_registry = None;
        self.dirty_files.clear();
        self.registry_fingerprint_aggregate = FingerprintAggregate::default();
    }

    pub fn refresh_file_stamp(&mut self, file_path: &str, on_disk_stamp: Option<FileStamp>) {
        if let Some(file_id) = self.workspace_file_id(file_path)
            && let Some(entry) = self.files.get_mut(&file_id)
        {
            entry.on_disk_stamp = on_disk_stamp;
        }
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// Streaming counterpart to `push_batch_file` + a final drain: merges each chunk's
/// snapshots into the registry as soon as that chunk's analysis finishes, instead of
/// retaining every file's snapshot until the whole scan completes. Caller supplies
/// chunks in the desired apply order (and files within a chunk in order); `Batch`
/// resolution still runs exactly once, in `finish`, matching `ResolutionProfile::Batch`.
pub struct BatchProjectionBuilder {
    registry: TypeRegistry,
    applied: usize,
}

impl BatchProjectionBuilder {
    pub fn new(base: TypeRegistry) -> Self {
        Self {
            registry: base,
            applied: 0,
        }
    }

    /// Apply one chunk's `(file_path, snapshot)` pairs, in order, then drop them.
    pub fn apply_chunk<I>(&mut self, chunk: I)
    where
        I: IntoIterator<Item = (String, FileAnalysisSnapshot)>,
    {
        for (_file_path, analysis) in chunk {
            analysis.apply_to_registry(&mut self.registry);
            self.applied += 1;
        }
    }

    pub fn applied_file_count(&self) -> usize {
        self.applied
    }

    pub fn finish(mut self, pool: Option<&rayon::ThreadPool>) -> Arc<TypeRegistry> {
        WorkspaceState::resolve_with_profile(&mut self.registry, ResolutionProfile::Batch, pool);
        Arc::new(self.registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_workspace_state() {
        let db = WorkspaceState::new();
        assert_eq!(db.file_count(), 0);
        assert!(!db.contains_file("foo.rb"));
    }

    #[test]
    fn test_upsert_file_first_time() {
        let mut db = WorkspaceState::new();
        let analysis = FileAnalysisSnapshot::empty();
        let deps = FileDeps::default();
        let invalidated = db.upsert_file("foo.rb".into(), 42, analysis, deps);
        assert!(invalidated.is_empty());
        assert!(db.contains_file("foo.rb"));
        assert_eq!(db.file_count(), 1);
    }

    #[test]
    fn test_upsert_same_fingerprint_no_invalidation() {
        let mut db = WorkspaceState::new();
        let analysis1 = FileAnalysisSnapshot::empty();
        let deps1 = FileDeps::default();
        db.upsert_file("foo.rb".into(), 1, analysis1, deps1);

        let analysis2 = FileAnalysisSnapshot::empty();
        let deps2 = FileDeps::default();
        let invalidated = db.upsert_file("foo.rb".into(), 2, analysis2, deps2);
        assert!(invalidated.is_empty());
    }

    #[test]
    fn test_remove_file() {
        let mut db = WorkspaceState::new();
        let analysis = FileAnalysisSnapshot::empty();
        let deps = FileDeps::default();
        db.upsert_file("foo.rb".into(), 42, analysis, deps);
        assert!(db.contains_file("foo.rb"));

        db.remove_file("foo.rb");
        assert!(!db.contains_file("foo.rb"));
        assert_eq!(db.file_count(), 0);
    }

    #[test]
    fn test_workspace_registry_caching() {
        let mut db = WorkspaceState::new();
        let user_rbs = TypeRegistry::new();

        let analysis = FileAnalysisSnapshot::empty();
        let deps = FileDeps::default();
        db.upsert_file("a.rb".into(), 1, analysis, deps);

        let _reg1 = db.workspace_registry(&user_rbs);
        assert!(db.dirty_files.is_empty());

        let _reg2 = db.workspace_registry(&user_rbs);
        assert!(db.dirty_files.is_empty());
    }

    #[test]
    fn batch_projection_resolves_cross_file_and_is_deterministic() {
        use crate::analysis::{AnalysisOptions, analyze_file_facts_with_deps};
        use crate::types::Type;

        let owner_source = concat!(
            "class Box\n",
            "  def initialize(value)\n",
            "    @value = value\n",
            "  end\n",
            "\n",
            "  def value\n",
            "    @value\n",
            "  end\n",
            "end\n",
        );
        let callsite_source = "Box.new(1)\n";

        let mut db = WorkspaceState::new();
        let base = TypeRegistry::new();

        let (owner_analysis, owner_deps) = analyze_file_facts_with_deps(
            owner_source,
            None,
            None,
            Some("box.rb"),
            AnalysisOptions::default(),
        );
        db.upsert_file(
            "box.rb".into(),
            hash_content(owner_source),
            owner_analysis,
            owner_deps,
        );
        let (callsite_analysis, callsite_deps) = analyze_file_facts_with_deps(
            callsite_source,
            None,
            None,
            Some("callsite.rb"),
            AnalysisOptions::default(),
        );
        db.upsert_file(
            "callsite.rb".into(),
            hash_content(callsite_source),
            callsite_analysis,
            callsite_deps,
        );

        let value_return = |registry: &TypeRegistry| {
            registry
                .methods_for_file("box.rb")
                .into_iter()
                .find(|(_, method)| method.name == "value")
                .map(|(_, method)| method.return_type)
        };

        let reg1 = db.batch_projection(&base, None);
        let ty1 = value_return(&reg1);
        assert!(
            matches!(ty1, Some(Type::LiteralInteger(1)) | Some(Type::Integer)),
            "batch projection should propagate the integer call site into Box#value, got {ty1:?}"
        );

        // The batch projection must not populate the LSP cache; it is a
        // one-shot short-lived registry.
        assert!(db.cached_registry.is_none());

        // Same inputs → identical resolved return type (determinism guard for
        // the chunk-parallel CLI render that reads this projection).
        let reg2 = db.batch_projection(&base, None);
        assert_eq!(ty1, value_return(&reg2));
    }

    /// Batch path: with a same-file call site, param substitution followed by structural bake/stdlib pure-table lookup reaches the same type as the full path.
    fn batch_snapshot(source: &str, path: &str) -> crate::inference::FileAnalysisSnapshot {
        use crate::analysis::{AnalysisOptions, analyze_compact_file_snapshot_timed};
        use crate::rbs::stdlib_loader::LazyRbsLoader;
        use std::path::PathBuf;
        let loader =
            LazyRbsLoader::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core"));
        analyze_compact_file_snapshot_timed(
            source,
            None,
            &loader,
            None,
            path,
            AnalysisOptions::default(),
            true,
        )
        .0
    }

    fn batch_return_type(registry: &TypeRegistry, class: &str, method: &str) -> String {
        registry
            .lookup_method_sig_exact(class, method, false)
            .map(|sig| sig.return_type.to_string())
            .unwrap_or_else(|| "<missing>".to_string())
    }

    #[test]
    fn batch_projection_resolves_structural_deferred_calls_same_file() {
        let source = concat!(
            "class Sample\n",
            "  def first_of(arr) = arr.first\n",
            "  def keyed(h) = h[:key]\n",
            "  def mapped(arr) = arr.map { |x| x }\n",
            "  def up(s) = s.upcase\n",
            "\n",
            "  def use\n",
            "    first_of([1, 2])\n",
            "    keyed({ key: 1 })\n",
            "    mapped([1, 2])\n",
            "    up(\"a\")\n",
            "  end\n",
            "end\n",
        );
        let mut db = WorkspaceState::new();
        db.push_batch_file("sample.rb".into(), batch_snapshot(source, "sample.rb"));
        let base = TypeRegistry::new();
        let registry = db.batch_projection(&base, None);

        assert_eq!(
            [
                batch_return_type(&registry, "Sample", "first_of"),
                batch_return_type(&registry, "Sample", "keyed"),
                batch_return_type(&registry, "Sample", "mapped"),
                batch_return_type(&registry, "Sample", "up"),
            ],
            ["Integer?", "Integer", "Array[Integer]", "String"]
        );
    }

    /// cross-file: no call site during collection -> keeps the marker, then the post-merge worklist substitutes it once per method.
    #[test]
    fn batch_projection_resolves_param_receiver_calls_cross_file() {
        let def_source = concat!(
            "class Sample\n",
            "  def first_of(arr) = arr.first\n",
            "  def keyed(h) = h[:key]\n",
            "  def mapped(arr) = arr.map { |x| x }\n",
            "  def up(s) = s.upcase\n",
            "end\n",
        );
        let caller_source = concat!(
            "class Caller\n",
            "  def use\n",
            "    sample = Sample.new\n",
            "    sample.first_of([1, 2])\n",
            "    sample.keyed({ key: 1 })\n",
            "    sample.mapped([1, 2])\n",
            "    sample.up(\"a\")\n",
            "  end\n",
            "end\n",
        );
        let mut db = WorkspaceState::new();
        db.push_batch_file("sample.rb".into(), batch_snapshot(def_source, "sample.rb"));
        db.push_batch_file(
            "caller.rb".into(),
            batch_snapshot(caller_source, "caller.rb"),
        );
        let base = TypeRegistry::new();
        let registry = db.batch_projection(&base, None);

        assert_eq!(
            [
                batch_return_type(&registry, "Sample", "first_of"),
                batch_return_type(&registry, "Sample", "keyed"),
                batch_return_type(&registry, "Sample", "mapped"),
                batch_return_type(&registry, "Sample", "up"),
            ],
            ["Integer?", "Integer", "Array[Integer]", "String"]
        );
    }

    #[test]
    fn batch_projection_resolves_cross_file_model_relation_calls() {
        let status_source = concat!(
            "class ActiveRecord::Base; end\n",
            "class Status < ActiveRecord::Base\n",
            "  def self.with_discarded = all\n",
            "end\n",
        );
        let report_source = concat!(
            "class Report < ActiveRecord::Base\n",
            "  def statuses = Status.with_discarded.where(id: [1])\n",
            "end\n",
        );
        let mut db = WorkspaceState::new();
        let status_snapshot = batch_snapshot(status_source, "status.rb");
        db.push_batch_file("status.rb".into(), status_snapshot);
        let report_snapshot = batch_snapshot(report_source, "report.rb");
        db.push_batch_file("report.rb".into(), report_snapshot);

        let registry = db.batch_projection(&TypeRegistry::new(), None);

        assert_eq!(
            batch_return_type(&registry, "Report", "statuses"),
            "ActiveRecord::Relation[Status]"
        );
    }

    /// Boundedness guard: a mutual-reference chain (`def a(x) = x.b` / `def b(y) = y.a`)
    /// converges to untyped in the worklist instead of diverging.
    #[test]
    fn batch_projection_bounds_mutual_param_receiver_chains() {
        let source = concat!(
            "class A\n",
            "  def a(x) = x.b\n",
            "end\n",
            "class B\n",
            "  def b(y) = y.a\n",
            "end\n",
            "class Caller\n",
            "  def use\n",
            "    A.new.a(B.new)\n",
            "    B.new.b(A.new)\n",
            "  end\n",
            "end\n",
        );
        let mut db = WorkspaceState::new();
        db.push_batch_file("sample.rb".into(), batch_snapshot(source, "sample.rb"));
        let base = TypeRegistry::new();
        let registry = db.batch_projection(&base, None);

        // Mutual recursion can't be concretized, so it degrades to untyped (no divergence or incorrect concretization).
        assert_eq!(batch_return_type(&registry, "A", "a"), "untyped");
        assert_eq!(batch_return_type(&registry, "B", "b"), "untyped");
    }

    /// batch projection: without a stdlib receiver loader, `ReceiverMethodRef` degrades to untyped (parity test against the Full path).
    #[test]
    fn test_workspace_registry_excluding_avoids_full_registry_cache() {
        let mut db = WorkspaceState::new();
        let user_rbs = TypeRegistry::new();

        let analysis = FileAnalysisSnapshot::empty();
        let deps = FileDeps::default();
        db.upsert_file("a.rb".into(), 1, analysis, deps);

        let excluding_fp = db.excluding_fingerprint("a.rb");
        let _reg = db.workspace_registry_excluding(&user_rbs, "a.rb", excluding_fp);

        assert!(
            db.cached_registry.is_none(),
            "excluding registry should not materialize the full workspace registry"
        );
        assert!(db.cached_display_base_registry.is_some());
    }

    #[test]
    fn warm_display_base_registry_hits_cache_on_first_excluding() {
        let mut db = WorkspaceState::new();
        let user_rbs = TypeRegistry::new();
        let (a_analysis, a_deps) = crate::analysis::analyze_file_facts_with_deps(
            "class A\n  def foo\n    1\n  end\nend\n",
            None,
            None,
            Some("a.rb"),
            crate::analysis::AnalysisOptions::default(),
        );
        db.upsert_file(
            "a.rb".into(),
            hash_content("class A\n  def foo\n    1\n  end\nend\n"),
            a_analysis,
            a_deps,
        );
        let (b_analysis, b_deps) = crate::analysis::analyze_file_facts_with_deps(
            "class B\n  def bar\n    2\n  end\nend\n",
            None,
            None,
            Some("b.rb"),
            crate::analysis::AnalysisOptions::default(),
        );
        db.upsert_file(
            "b.rb".into(),
            hash_content("class B\n  def bar\n    2\n  end\nend\n"),
            b_analysis,
            b_deps,
        );

        assert!(
            !db.warm_display_base_registry(&user_rbs),
            "first warm should build the base registry"
        );
        let warmed_base = db
            .cached_display_base_registry()
            .expect("display base after warm");

        db.last_timings = WorkspaceTimings::default();
        let _reg =
            db.workspace_registry_excluding(&user_rbs, "a.rb", db.excluding_fingerprint("a.rb"));
        assert!(
            db.last_timings.display_base_cache_hit,
            "excluding should hit the pre-warmed display base"
        );
        assert!(
            Arc::ptr_eq(
                &warmed_base,
                &db.cached_display_base_registry().expect("display base")
            ),
            "excluding should reuse the warmed display base Arc"
        );
    }

    #[test]
    fn workspace_registry_excluding_shares_display_base_across_files() {
        let mut db = WorkspaceState::new();
        let user_rbs = TypeRegistry::new();
        let (a_analysis, a_deps) = crate::analysis::analyze_file_facts_with_deps(
            "class A\n  def foo\n    1\n  end\nend\n",
            None,
            None,
            Some("a.rb"),
            crate::analysis::AnalysisOptions::default(),
        );
        db.upsert_file(
            "a.rb".into(),
            hash_content("class A\n  def foo\n    1\n  end\nend\n"),
            a_analysis,
            a_deps,
        );
        let (b_analysis, b_deps) = crate::analysis::analyze_file_facts_with_deps(
            "class B\n  def bar\n    2\n  end\nend\n",
            None,
            None,
            Some("b.rb"),
            crate::analysis::AnalysisOptions::default(),
        );
        db.upsert_file(
            "b.rb".into(),
            hash_content("class B\n  def bar\n    2\n  end\nend\n"),
            b_analysis,
            b_deps,
        );

        let a_reg =
            db.workspace_registry_excluding(&user_rbs, "a.rb", db.excluding_fingerprint("a.rb"));
        let base1 = db
            .cached_display_base_registry()
            .expect("display base after A");
        assert!(
            a_reg.lookup_method_def("A", "foo", false).is_none(),
            "current-file methods must not leak into the excluding registry"
        );
        assert!(a_reg.lookup_method_def("B", "bar", false).is_some());

        let b_reg =
            db.workspace_registry_excluding(&user_rbs, "b.rb", db.excluding_fingerprint("b.rb"));
        let base2 = db
            .cached_display_base_registry()
            .expect("display base after B");
        assert!(
            Arc::ptr_eq(&base1, &base2),
            "display base should be reused when only the current file changes"
        );
        assert!(b_reg.lookup_method_def("B", "bar", false).is_none());
        assert!(b_reg.lookup_method_def("A", "foo", false).is_some());
    }

    #[test]
    #[ignore = "display-scope pruning removed for correctness; any workspace change invalidates the excluding-base cache now"]
    fn workspace_registry_excluding_reuses_cache_when_unrelated_file_changes() {
        let provider_source = "class Provider\n  def greeting\n    \"hello\"\n  end\nend\n";
        let consumer_source = "class Consumer\n  def call\n    Provider.new.greeting\n  end\nend\n";

        let mut db = WorkspaceState::new();
        let user_rbs = TypeRegistry::new();

        let (provider_analysis, provider_deps) = crate::analysis::analyze_file_facts_with_deps(
            provider_source,
            None,
            None,
            Some("provider.rb"),
            crate::analysis::AnalysisOptions::default(),
        );
        db.upsert_file(
            "provider.rb".into(),
            hash_content(provider_source),
            provider_analysis,
            provider_deps,
        );

        let (consumer_analysis, consumer_deps) = crate::analysis::analyze_file_facts_with_deps(
            consumer_source,
            None,
            None,
            Some("consumer.rb"),
            crate::analysis::AnalysisOptions::default(),
        );
        db.upsert_file(
            "consumer.rb".into(),
            hash_content(consumer_source),
            consumer_analysis,
            consumer_deps,
        );

        for idx in 0..70 {
            let filler_source = format!("class Filler{idx}\nend\n");
            let file_name = format!("filler_{idx}.rb");
            let (analysis, deps) = crate::analysis::analyze_file_facts_with_deps(
                &filler_source,
                None,
                None,
                Some(&file_name),
                crate::analysis::AnalysisOptions::default(),
            );
            db.upsert_file(file_name, hash_content(&filler_source), analysis, deps);
        }

        let _reg1 = db.workspace_registry_excluding(
            &user_rbs,
            "provider.rb",
            db.excluding_fingerprint("provider.rb"),
        );
        let base1 = db
            .cached_display_base_registry()
            .expect("base excluding cache should exist");

        let unrelated_source = "class FillerAlpha\n  def value\n    1\n  end\nend\n";
        let (analysis, deps) = crate::analysis::analyze_file_facts_with_deps(
            unrelated_source,
            None,
            None,
            Some("filler_alpha.rb"),
            crate::analysis::AnalysisOptions::default(),
        );
        db.upsert_file(
            "filler_alpha.rb".into(),
            hash_content(unrelated_source),
            analysis,
            deps,
        );

        let _reg2 = db.workspace_registry_excluding(
            &user_rbs,
            "provider.rb",
            db.excluding_fingerprint("provider.rb"),
        );
        let base2 = db
            .cached_display_base_registry()
            .expect("base excluding cache should still exist");

        assert!(
            Arc::ptr_eq(&base1, &base2),
            "excluding base registry should stay cached when only unrelated files change"
        );
    }

    #[test]
    fn display_can_skip_workspace_for_self_contained_file() {
        let source = "module Ml\n  def self.table_name_prefix\n    'ml_'\n  end\nend\n";
        let (analysis, deps) = crate::analysis::analyze_file_facts_with_deps(
            source,
            None,
            None,
            Some("ml.rb"),
            crate::analysis::AnalysisOptions::default(),
        );

        let mut db = WorkspaceState::new();
        db.upsert_file("ml.rb".into(), hash_content(source), analysis, deps);

        assert!(db.display_can_skip_workspace_context("ml.rb"));
    }

    #[test]
    fn display_keeps_workspace_for_cross_file_method_dependents() {
        let provider_source = "class Provider\n  def greeting\n    \"hello\"\n  end\nend\n";
        let consumer_source = "class Consumer\n  def call\n    Provider.new.greeting\n  end\nend\n";

        let (provider_analysis, provider_deps) = crate::analysis::analyze_file_facts_with_deps(
            provider_source,
            None,
            None,
            Some("provider.rb"),
            crate::analysis::AnalysisOptions::default(),
        );
        let (consumer_analysis, consumer_deps) = crate::analysis::analyze_file_facts_with_deps(
            consumer_source,
            None,
            None,
            Some("consumer.rb"),
            crate::analysis::AnalysisOptions::default(),
        );

        let mut db = WorkspaceState::new();
        db.upsert_file(
            "provider.rb".into(),
            hash_content(provider_source),
            provider_analysis,
            provider_deps,
        );
        db.upsert_file(
            "consumer.rb".into(),
            hash_content(consumer_source),
            consumer_analysis,
            consumer_deps,
        );

        assert!(!db.display_can_skip_workspace_context("provider.rb"));
    }

    #[test]
    fn refresh_file_stamp_updates_existing_entry() {
        let mut db = WorkspaceState::new();
        let analysis = FileAnalysisSnapshot::empty();
        let deps = FileDeps::default();
        db.upsert_file("a.rb".into(), 1, analysis, deps);
        assert_eq!(
            db.workspace_file("a.rb")
                .and_then(|entry| entry.on_disk_stamp),
            None
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.rb");
        std::fs::write(&file, "class A; end\n").expect("write file");
        let stamp = FileStamp::from_path(&file);

        db.refresh_file_stamp("a.rb", stamp);
        assert_eq!(
            db.workspace_file("a.rb")
                .and_then(|entry| entry.on_disk_stamp),
            stamp
        );
    }

    #[test]
    fn test_dirty_files_after_upsert() {
        let mut db = WorkspaceState::new();
        let user_rbs = TypeRegistry::new();

        let analysis = FileAnalysisSnapshot::empty();
        let deps = FileDeps::default();
        db.upsert_file("a.rb".into(), 1, analysis, deps);
        let _reg = db.workspace_registry(&user_rbs);
        assert!(db.dirty_files.is_empty());

        let analysis2 = FileAnalysisSnapshot::empty();
        let deps2 = FileDeps::default();
        db.upsert_file("a.rb".into(), 2, analysis2, deps2);
        assert!(
            !db.is_file_dirty("a.rb"),
            "exports unchanged → should not be dirty"
        );

        let changed_analysis = FileAnalysisSnapshot::from_registry(
            crate::analysis::analyze_source("class NewClass\nend\n"),
        );
        let deps3 = FileDeps::default();
        db.upsert_file("a.rb".into(), 3, changed_analysis, deps3);
        assert!(
            db.is_file_dirty("a.rb"),
            "exports changed → should be dirty"
        );
    }

    #[test]
    fn initial_population_skips_dependent_invalidation_until_first_registry_build() {
        use crate::analysis::{AnalysisOptions, analyze_file_facts_with_deps};

        let provider_v1 = "class Provider\n  def greeting = \"hello\"\nend\n";
        let consumer = "class Consumer\n  def call = Provider.new.greeting\nend\n";
        let provider_v2 = "class Provider\n  def greeting = 42\nend\n";

        let (provider_analysis, provider_deps) = analyze_file_facts_with_deps(
            provider_v1,
            None,
            None,
            Some("provider.rb"),
            AnalysisOptions::default(),
        );
        let (consumer_analysis, consumer_deps) = analyze_file_facts_with_deps(
            consumer,
            None,
            None,
            Some("consumer.rb"),
            AnalysisOptions::default(),
        );

        let mut db = WorkspaceState::new();
        let provider_fingerprints = FileFingerprints {
            export: ExportFingerprint::from_registry(provider_analysis.registry()),
            registry: RegistryFingerprint::from_analysis(&provider_analysis),
        };
        db.upsert_scanned_file_with_stamp_and_fingerprints(
            "provider.rb".into(),
            hash_content(provider_v1),
            provider_analysis,
            provider_deps,
            None,
            provider_fingerprints,
        );
        let consumer_fingerprints = FileFingerprints {
            export: ExportFingerprint::from_registry(consumer_analysis.registry()),
            registry: RegistryFingerprint::from_analysis(&consumer_analysis),
        };
        db.upsert_scanned_file_with_stamp_and_fingerprints(
            "consumer.rb".into(),
            hash_content(consumer),
            consumer_analysis,
            consumer_deps,
            None,
            consumer_fingerprints,
        );

        assert!(
            db.dirty_files.is_empty(),
            "initial population should avoid dependent invalidation work before the first registry build"
        );

        let user_rbs = TypeRegistry::new();
        let _registry = db.workspace_registry(&user_rbs);

        let (provider_analysis_v2, provider_deps_v2) = analyze_file_facts_with_deps(
            provider_v2,
            None,
            None,
            Some("provider.rb"),
            AnalysisOptions::default(),
        );
        db.upsert_file(
            "provider.rb".into(),
            hash_content(provider_v2),
            provider_analysis_v2,
            provider_deps_v2,
        );

        assert!(
            db.is_file_dirty("consumer.rb"),
            "after the first registry build, export changes should still invalidate dependents"
        );
    }

    #[test]
    fn workspace_registry_rebuilds_on_body_only_change() {
        use crate::analysis::{AnalysisOptions, analyze_file_facts_with_deps};
        use crate::types::Type;

        let owner_source = concat!(
            "class Box\n",
            "  def initialize(value)\n",
            "    @value = value\n",
            "  end\n",
            "\n",
            "  def value\n",
            "    @value\n",
            "  end\n",
            "end\n",
        );
        let callsite_int_source = "Box.new(1)\n";
        let callsite_string_source = "Box.new(\"s\")\n";

        let mut db = WorkspaceState::new();
        let user_rbs = TypeRegistry::new();

        let (owner_analysis, owner_deps) = analyze_file_facts_with_deps(
            owner_source,
            None,
            None,
            Some("box.rb"),
            AnalysisOptions::default(),
        );
        db.upsert_file(
            "box.rb".into(),
            hash_content(owner_source),
            owner_analysis,
            owner_deps,
        );

        let (int_analysis, int_deps) = analyze_file_facts_with_deps(
            callsite_int_source,
            None,
            None,
            Some("callsite.rb"),
            AnalysisOptions::default(),
        );
        db.upsert_file(
            "callsite.rb".into(),
            hash_content(callsite_int_source),
            int_analysis,
            int_deps,
        );

        let initial = db.workspace_registry(&user_rbs);
        let initial_ty = initial
            .methods_for_file("box.rb")
            .into_iter()
            .find(|(_, method)| method.name == "value")
            .map(|(_, method)| method.return_type);
        assert!(
            matches!(
                initial_ty,
                Some(Type::LiteralInteger(1)) | Some(Type::Integer)
            ),
            "expected Box#value to reflect integer call site, got {initial_ty:?}"
        );

        let (string_analysis, string_deps) = analyze_file_facts_with_deps(
            callsite_string_source,
            None,
            None,
            Some("callsite.rb"),
            AnalysisOptions::default(),
        );
        db.upsert_file(
            "callsite.rb".into(),
            hash_content(callsite_string_source),
            string_analysis,
            string_deps,
        );

        let updated = db.workspace_registry(&user_rbs);
        let updated_ty = updated
            .methods_for_file("box.rb")
            .into_iter()
            .find(|(_, method)| method.name == "value")
            .map(|(_, method)| method.return_type);
        assert!(
            matches!(
                updated_ty,
                Some(Type::LiteralString(ref value)) if value == "s"
            ) || matches!(updated_ty, Some(Type::String)),
            "expected Box#value to reflect string call site, got {updated_ty:?}"
        );
    }

    #[test]
    fn excluding_fingerprint_tracks_method_body_summary_of_included_files() {
        use crate::analysis::{AnalysisOptions, analyze_file_facts_with_deps};

        let owner_source = concat!(
            "class Box\n",
            "  def initialize(value)\n",
            "    @value = value\n",
            "  end\n",
            "end\n",
        );
        let callsite_int_source = "Box.new(1)\n";
        let callsite_string_source = "Box.new(\"s\")\n";

        let mut db = WorkspaceState::new();

        let (owner_analysis, owner_deps) = analyze_file_facts_with_deps(
            owner_source,
            None,
            None,
            Some("box.rb"),
            AnalysisOptions::default(),
        );
        db.upsert_file(
            "box.rb".into(),
            hash_content(owner_source),
            owner_analysis,
            owner_deps,
        );

        let (int_analysis, int_deps) = analyze_file_facts_with_deps(
            callsite_int_source,
            None,
            None,
            Some("callsite.rb"),
            AnalysisOptions::default(),
        );
        db.upsert_file(
            "callsite.rb".into(),
            hash_content(callsite_int_source),
            int_analysis,
            int_deps,
        );

        let fp_before = db.excluding_fingerprint("current.rb");
        let fp_excluding_callsite_before = db.excluding_fingerprint("callsite.rb");

        let (string_analysis, string_deps) = analyze_file_facts_with_deps(
            callsite_string_source,
            None,
            None,
            Some("callsite.rb"),
            AnalysisOptions::default(),
        );
        db.upsert_file(
            "callsite.rb".into(),
            hash_content(callsite_string_source),
            string_analysis,
            string_deps,
        );

        assert_ne!(fp_before, db.excluding_fingerprint("current.rb"));
        assert_eq!(
            fp_excluding_callsite_before,
            db.excluding_fingerprint("callsite.rb"),
            "excluded file body-only changes should not invalidate the excluding view"
        );
    }

    #[test]
    fn test_export_fingerprint_unchanged_no_downstream_invalidation() {
        use crate::dep_graph::DepEdge;
        use crate::dep_graph::DepEdgeKind;
        use std::collections::HashSet;

        let mut db = WorkspaceState::new();

        let provider_analysis =
            crate::analysis::analyze_source("class Provider\n  def foo\n    42\n  end\nend\n");
        let provider_cached = FileAnalysisSnapshot::from_registry(provider_analysis);
        let provider_deps = FileDeps {
            defined_symbols: HashSet::from(["Provider".to_string()]),
            ..Default::default()
        };
        db.upsert_file("provider.rb".into(), 1, provider_cached, provider_deps);

        let consumer_deps = FileDeps {
            defined_symbols: HashSet::from(["Consumer".to_string()]),
            referenced_symbols: HashSet::from(["Provider".to_string()]),
            edges: vec![DepEdge {
                symbol: "Provider".to_string(),
                kind: DepEdgeKind::MethodCall,
            }],
        };
        db.upsert_file(
            "consumer.rb".into(),
            1,
            FileAnalysisSnapshot::empty(),
            consumer_deps,
        );

        let provider_analysis2 =
            crate::analysis::analyze_source("class Provider\n  def foo\n    42\n  end\nend\n");
        let provider_cached2 = FileAnalysisSnapshot::from_registry(provider_analysis2);
        let provider_deps2 = FileDeps {
            defined_symbols: HashSet::from(["Provider".to_string()]),
            ..Default::default()
        };
        let invalidated = db.upsert_file("provider.rb".into(), 2, provider_cached2, provider_deps2);
        assert!(
            invalidated.is_empty(),
            "consumer.rb should NOT be invalidated when Provider's exports are unchanged"
        );
    }

    #[test]
    fn export_fingerprint_is_stable_across_definition_order() {
        let original = crate::analysis::analyze_source(
            r#"
class Provider
  ANSWER = 42

  def foo
    "hello"
  end

  def bar
    true
  end
end
"#,
        );
        let reordered = crate::analysis::analyze_source(
            r#"
class Provider
  def bar
    true
  end

  ANSWER = 42

  def foo
    "hello"
  end
end
"#,
        );

        assert_eq!(
            ExportFingerprint::from_registry(&original),
            ExportFingerprint::from_registry(&reordered)
        );
    }

    #[test]
    fn test_export_fingerprint_changed_invalidates_downstream() {
        use crate::dep_graph::DepEdge;
        use crate::dep_graph::DepEdgeKind;
        use std::collections::HashSet;

        let mut db = WorkspaceState::new();

        let provider_analysis =
            crate::analysis::analyze_source("class Provider\n  def foo\n    42\n  end\nend\n");
        let provider_cached = FileAnalysisSnapshot::from_registry(provider_analysis);
        let provider_deps = FileDeps {
            defined_symbols: HashSet::from(["Provider".to_string()]),
            ..Default::default()
        };
        db.upsert_file("provider.rb".into(), 1, provider_cached, provider_deps);

        let consumer_deps = FileDeps {
            defined_symbols: HashSet::from(["Consumer".to_string()]),
            referenced_symbols: HashSet::from(["Provider".to_string()]),
            edges: vec![DepEdge {
                symbol: "Provider".to_string(),
                kind: DepEdgeKind::MethodCall,
            }],
        };
        db.upsert_file(
            "consumer.rb".into(),
            1,
            FileAnalysisSnapshot::empty(),
            consumer_deps,
        );

        let changed_analysis = crate::analysis::analyze_source(
            "class Provider\n  def foo\n    \"hello\"\n  end\n  def bar\n    true\n  end\nend\n",
        );
        let changed_cached = FileAnalysisSnapshot::from_registry(changed_analysis);
        let changed_deps = FileDeps {
            defined_symbols: HashSet::from(["Provider".to_string()]),
            ..Default::default()
        };
        let invalidated = db.upsert_file("provider.rb".into(), 3, changed_cached, changed_deps);
        assert!(
            invalidated.contains("consumer.rb"),
            "consumer.rb SHOULD be invalidated when Provider's exports changed"
        );
    }

    #[test]
    fn cli_and_lsp_produce_same_cross_file_hover_types() {
        use crate::analysis::{AnalysisOptions, analyze_cached_file_with_deps, hover_at};
        use crate::rbs::stdlib_loader::LazyRbsLoader;

        let core_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);

        let provider_source = "class Provider\n  def greeting\n    \"hello\"\n  end\nend\n";
        let consumer_source = concat!(
            "class Consumer\n",
            "  def run\n",
            "    Provider.new.greeting\n",
            "  end\n",
            "end\n",
        );

        let cli_provider_reg = crate::analysis::analyze_source_with_file_path_rails_timed_lazy(
            provider_source,
            None,
            &loader,
            None,
            "provider.rb",
            AnalysisOptions::default(),
            true,
        )
        .0;
        let cli_consumer_reg = crate::analysis::analyze_source_with_file_path_rails_timed_lazy(
            consumer_source,
            None,
            &loader,
            None,
            "consumer.rb",
            AnalysisOptions::default(),
            true,
        )
        .0;

        let mut cli_workspace = TypeRegistry::new();
        cli_workspace.merge_rbs_registry(&cli_provider_reg);
        cli_workspace.merge_rbs_registry(&cli_consumer_reg);
        cli_workspace.apply_global_resolution();

        let cli_hover = hover_at(
            consumer_source,
            Some(&cli_workspace),
            &loader,
            "consumer.rb",
            3,
            18,
        )
        .expect("CLI hover");

        let mut db = WorkspaceState::new();
        let lsp_provider = analyze_cached_file_with_deps(
            provider_source,
            None,
            Some(&loader),
            Some("provider.rb"),
            AnalysisOptions::default(),
        );
        db.upsert_file(
            "provider.rb".into(),
            hash_content(provider_source),
            lsp_provider.0,
            lsp_provider.1,
        );

        let user_rbs = TypeRegistry::new();
        let lsp_workspace = db.workspace_registry(&user_rbs);

        let lsp_hover = hover_at(
            consumer_source,
            Some(&lsp_workspace),
            &loader,
            "consumer.rb",
            3,
            18,
        )
        .expect("LSP hover");

        assert_eq!(
            cli_hover.ty.to_string(),
            lsp_hover.ty.to_string(),
            "CLI and LSP must produce the same hover type for cross-file method call"
        );
        assert_eq!(cli_hover.name, lsp_hover.name);
    }

    /// Verify that cross-file inheritance, ivar references, and predicate
    /// methods produce identical hover results in both pipelines.
    #[test]
    fn cli_and_lsp_parity_inheritance_and_ivar() {
        use crate::analysis::{AnalysisOptions, analyze_cached_file_with_deps, hover_at};
        use crate::rbs::stdlib_loader::LazyRbsLoader;

        let core_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);

        let base_source = concat!(
            "class Base\n",
            "  def initialize(name)\n",
            "    @name = name\n",
            "  end\n",
            "\n",
            "  def has_name?\n",
            "    !@name.nil?\n",
            "  end\n",
            "end\n",
        );
        let child_source = concat!(
            "class Child < Base\n",
            "  def label\n",
            "    has_name? ? \"yes\" : \"no\"\n",
            "  end\n",
            "end\n",
        );

        // hover target: `has_name?` in child_source line 3, col 4

        let cli_base = crate::analysis::analyze_source_with_file_path_rails_timed_lazy(
            base_source,
            None,
            &loader,
            None,
            "base.rb",
            AnalysisOptions::default(),
            true,
        )
        .0;
        let cli_child = crate::analysis::analyze_source_with_file_path_rails_timed_lazy(
            child_source,
            None,
            &loader,
            None,
            "child.rb",
            AnalysisOptions::default(),
            true,
        )
        .0;

        let mut cli_workspace = TypeRegistry::new();
        cli_workspace.merge_rbs_registry(&cli_base);
        cli_workspace.merge_rbs_registry(&cli_child);
        cli_workspace.apply_global_resolution();

        let cli_hover = hover_at(
            child_source,
            Some(&cli_workspace),
            &loader,
            "child.rb",
            3,
            4,
        )
        .expect("CLI hover for has_name?");

        let lsp_base = analyze_cached_file_with_deps(
            base_source,
            None,
            Some(&loader),
            Some("base.rb"),
            AnalysisOptions::default(),
        );
        let mut db = WorkspaceState::new();
        db.upsert_file(
            "base.rb".into(),
            hash_content(base_source),
            lsp_base.0,
            lsp_base.1,
        );

        let user_rbs = TypeRegistry::new();
        let lsp_workspace = db.workspace_registry(&user_rbs);

        let lsp_hover = hover_at(
            child_source,
            Some(&lsp_workspace),
            &loader,
            "child.rb",
            3,
            4,
        )
        .expect("LSP hover for has_name?");

        // --- Assert parity ---
        assert_eq!(cli_hover.name, lsp_hover.name);
        assert_eq!(
            cli_hover.ty.to_string(),
            lsp_hover.ty.to_string(),
            "CLI and LSP must agree on inherited predicate method type"
        );
    }

    /// LSP incremental design (design-lsp-incremental.md §6, "premise #3"):
    /// fragment-replace only works if class-level merge results depend on the
    /// contributor set, not on merge order. Merges a fixed subset of real
    /// files through the CLI's own `push_batch_file` + `batch_projection` path
    /// in two different insertion orders (natural vs. a fixed-seed shuffle)
    /// and byte-compares the rendered RBS. A diff here is K3: fragment-replace
    /// is dead unless the diverging classes are all merge-local (contributor
    /// order fixes them); a diff that crosses classes kills the design.
    #[test]
    fn merge_order_shuffle_renders_identically_gitlab_concerns_subset() {
        use crate::analysis::{AnalysisOptions, analyze_compact_file_snapshot_timed};
        use crate::rbs::render::render_rbs;
        use crate::rbs::stdlib_loader::LazyRbsLoader;

        let concerns_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("subject/gitlab/app/models/concerns");
        if !concerns_dir.exists() {
            eprintln!(
                "skipping: gitlab subject not found: {}",
                concerns_dir.display()
            );
            return;
        }

        let mut paths = crate::workspace_discovery::collect_rb_files_from_roots(
            std::slice::from_ref(&concerns_dir),
        );
        paths.sort();
        paths.truncate(40); // "a few dozen" files: enough to exercise cross-file merge rules, fast enough for --test-threads=1.
        assert!(
            paths.len() >= 20,
            "expected at least 20 concern files under {}, found {}",
            concerns_dir.display(),
            paths.len()
        );

        let loader = LazyRbsLoader::new(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core"),
        );
        let base = TypeRegistry::new();

        let snapshots: Vec<(String, FileAnalysisSnapshot)> = paths
            .iter()
            .map(|path| {
                let source = std::fs::read_to_string(path)
                    .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
                let file_path = path.to_string_lossy().into_owned();
                let snapshot = analyze_compact_file_snapshot_timed(
                    &source,
                    Some(&base),
                    &loader,
                    None,
                    &file_path,
                    AnalysisOptions::default(),
                    true,
                )
                .0;
                (file_path, snapshot)
            })
            .collect();

        // Fixed-seed xorshift32 Fisher-Yates: deterministic across runs without a `rand` dev-dependency.
        fn shuffled_order(seed: u32, len: usize) -> Vec<usize> {
            let mut state = seed;
            let mut next_u32 = move || {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state
            };
            let mut order: Vec<usize> = (0..len).collect();
            for i in (1..len).rev() {
                let j = (next_u32() as usize) % (i + 1);
                order.swap(i, j);
            }
            order
        }

        let render_in_order = |order: &[usize]| -> String {
            let mut db = WorkspaceState::new();
            for &index in order {
                let (file_path, snapshot) = &snapshots[index];
                db.push_batch_file(file_path.clone(), snapshot.clone());
            }
            let registry = db.batch_projection(&base, None);
            render_rbs(&registry)
        };

        let natural: Vec<usize> = (0..snapshots.len()).collect();
        let natural_rendered = render_in_order(&natural);

        // Several seeds: one permutation can miss an ordering hazard that only shows up
        // when a specific pair of contributors swaps.
        let mut diverged: Option<(u32, String)> = None;
        for seed in [0xC0FF_EE42u32, 0x1234_5678, 0xDEAD_BEEF] {
            let shuffled = shuffled_order(seed, snapshots.len());
            assert_ne!(
                shuffled, natural,
                "seed {seed:#x} degenerated into the identity order; pick a different seed"
            );
            let shuffled_rendered = render_in_order(&shuffled);
            if shuffled_rendered != natural_rendered {
                diverged = Some((seed, shuffled_rendered));
                break;
            }
        }

        let Some((seed, shuffled_rendered)) = diverged else {
            eprintln!(
                "[merge-order] identical across {} files / {} bytes rendered (natural vs. 3 shuffled orders)",
                snapshots.len(),
                natural_rendered.len(),
            );
            return;
        };
        eprintln!("[merge-order] first diverging seed: {seed:#x}");

        // Diverged: split into class blocks (blank-line separated, matches the renderer's own separator) and report the first few mismatches.
        fn split_classes(text: &str) -> Vec<&str> {
            text.split("\n\n")
                .filter(|block| !block.trim().is_empty())
                .collect()
        }
        let natural_classes = split_classes(&natural_rendered);
        let shuffled_classes = split_classes(&shuffled_rendered);
        let mut diffs: Vec<(&str, &str)> = Vec::new();
        for (a, b) in natural_classes.iter().zip(shuffled_classes.iter()) {
            if a != b {
                diffs.push((a, b));
            }
        }
        eprintln!(
            "[merge-order] DIVERGED: {} files, natural={} blocks/{} bytes shuffled={} blocks/{} bytes, {} differing block(s)",
            snapshots.len(),
            natural_classes.len(),
            natural_rendered.len(),
            shuffled_classes.len(),
            shuffled_rendered.len(),
            diffs.len(),
        );
        for (natural_block, shuffled_block) in diffs.iter().take(5) {
            eprintln!("--- natural ---\n{natural_block}\n--- shuffled ---\n{shuffled_block}\n");
        }
        panic!(
            "merge order changed rendered RBS output ({} differing block(s) of {}/{}); see stderr above for the first differing class blocks",
            diffs.len(),
            natural_classes.len(),
            shuffled_classes.len(),
        );
    }
}
