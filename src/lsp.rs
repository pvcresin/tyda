use crate::types::Sym;
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rustc_hash::FxHasher;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

mod completion;
mod source_support;

use crate::analysis::{
    AnalysisOptions, QueryAnalysisMode, SyntaxErrorSuppressor,
    analyze_file_facts_with_deps_and_rbi, analyze_file_facts_with_deps_and_rbi_with_options_ref,
    analyze_file_for_query, analyze_source_for_display, format_hover_body,
};
#[cfg(test)]
use crate::diagnostics::has_type_hole;
use crate::inference::{DefinitionLookupTarget, FileAnalysisSnapshot};
use crate::project::{DslActivation, ProjectVersions, detect_dsl_activation};
use crate::query::TypeQueryEngine;
use crate::rbs::display::format_method_sig_for_lens_with_names;
use crate::rbs::stdlib_loader::LazyRbsLoader;
use crate::rbs::workspace::{load_workspace_type_environment, reload_external_type_file};
use crate::registry::TypeRegistry;
use crate::sorbet::rbi::LazyRbiLoader;
#[cfg(test)]
use crate::types::Param;
use crate::types::{MethodSig, ParamKind, SourceLocation, Type};
use crate::workspace_discovery::collect_rb_files_from_roots;
use crate::workspace_state::{DisplayScopeKey, FileFingerprints, FileStamp, WorkspaceState};
use completion::{constant_completion_items, method_completion_items};
#[cfg(test)]
use source_support::source_line_supports_signature_comment;
use source_support::{
    SendMethodNameReceiver, apply_content_changes, byte_offset_to_lsp_position,
    code_lens_range_for_method, dot_method_completion_context,
    double_colon_constant_completion_context, fallback_code_lens_methods_from_source,
    insert_signature_comment, lsp_position_to_byte_offset, method_definition_name_offset,
    method_name_offset_for_definition_line, offset_to_line_col,
    rbs_comment_type_definition_context, send_method_name_completion_context,
    source_line_has_direct_annotation, source_line_supports_signature_comment_from_lines,
    split_source_lines, uri_to_path,
};
const CODE_LENS_CHANGE_REFRESH_DEBOUNCE_MS: u64 = 75;
/// `didChange` re-infers the whole file to publish diagnostics; a burst of keystrokes
/// coalesces into one run instead of one per event.
const DIAGNOSTICS_CHANGE_DEBOUNCE_MS: u64 = 150;
#[cfg(test)]
static DIAGNOSTICS_CHANGE_DEBOUNCE_TEST_MS: AtomicU64 = AtomicU64::new(5);

fn diagnostics_change_debounce_ms() -> u64 {
    #[cfg(test)]
    {
        DIAGNOSTICS_CHANGE_DEBOUNCE_TEST_MS.load(Ordering::SeqCst)
    }
    #[cfg(not(test))]
    {
        DIAGNOSTICS_CHANGE_DEBOUNCE_MS
    }
}
const MISSING_METHOD_DIAGNOSTIC_CODE: &str = "tyda.missingMethod";
const ARGUMENT_TYPE_MISMATCH_DIAGNOSTIC_CODE: &str = "tyda.argumentTypeMismatch";
const UNRESOLVED_CONSTANT_DIAGNOSTIC_CODE: &str = "tyda.unresolvedConstant";
const UNUSED_IGNORE_DIAGNOSTIC_CODE: &str = "tyda.unusedIgnore";

#[allow(clippy::large_enum_variant)]
enum WorkspaceScanResult {
    Analyzed {
        file_path: String,
        content_hash: u64,
        analysis: FileAnalysisSnapshot,
        fingerprints: FileFingerprints,
        file_deps: crate::dep_graph::FileDeps,
        on_disk_stamp: Option<FileStamp>,
    },
    RefreshStamp {
        file_path: String,
        on_disk_stamp: Option<FileStamp>,
    },
}

const LSP_SCAN_CHUNK_SIZE: usize = 2048;

#[derive(Clone, Copy)]
struct WorkspaceScanCacheEntry {
    content_hash: u64,
    on_disk_stamp: Option<FileStamp>,
}

struct WorkspaceScanInputs<'a> {
    cached_entries: &'a HashMap<String, WorkspaceScanCacheEntry>,
    open_docs: &'a HashMap<String, String>,
    user_rbs: &'a TypeRegistry,
    stdlib_loader: &'a LazyRbsLoader,
    lazy_rbi_loader: Option<&'a LazyRbiLoader>,
    options: &'a AnalysisOptions,
}

/// Returns the P-core count on Apple Silicon (avoids fixed analysis on E-cores only).
#[cfg(target_os = "macos")]
fn performance_core_count() -> Option<usize> {
    let name = c"hw.perflevel0.logicalcpu";
    let mut value: i32 = 0;
    let mut size = std::mem::size_of::<i32>();
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            &mut value as *mut _ as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (rc == 0 && value > 0).then_some(value as usize)
}
#[cfg(not(target_os = "macos"))]
fn performance_core_count() -> Option<usize> {
    None
}

/// Worker count for the LSP analysis pool (leaves headroom for UI/tokio).
fn lsp_analysis_thread_count() -> usize {
    let logical = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .max(1);
    // Hybrid CPUs use P-cores minus 1; otherwise logical cores minus 2 (minimum 2).
    let auto = match performance_core_count() {
        Some(perf) => perf.saturating_sub(1),
        None => logical.saturating_sub(2),
    }
    .max(2);
    if let Ok(override_str) = std::env::var("TYDA_LSP_ANALYSIS_THREADS")
        && let Ok(override_val) = override_str.parse::<usize>()
        && override_val > 0
    {
        return override_val;
    }
    auto
}

fn lsp_analysis_pool() -> &'static rayon::ThreadPool {
    static ANALYSIS_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    ANALYSIS_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(lsp_analysis_thread_count())
            .stack_size(crate::analysis::ANALYSIS_WORKER_STACK_SIZE)
            .build()
            .expect("failed to build LSP analysis thread pool")
    })
}

fn closed_file_scan_key(stamp: Option<FileStamp>) -> u64 {
    let mut hasher = FxHasher::default();
    stamp.hash(&mut hasher);
    hasher.finish()
}

fn for_each_workspace_scan_result(
    scan_files: &[PathBuf],
    context: WorkspaceScanInputs<'_>,
    mut should_abort: impl FnMut() -> bool,
    mut on_result: impl FnMut(WorkspaceScanResult),
) {
    use rayon::prelude::*;

    let cold_closed_file_scan = context.cached_entries.is_empty() && context.open_docs.is_empty();

    for chunk in scan_files.chunks(LSP_SCAN_CHUNK_SIZE) {
        // Checked once per chunk (not per file) so a superseded scan bails out
        // without a full walk, but without adding per-file lock overhead.
        if should_abort() {
            return;
        }
        let results: Vec<_> = lsp_analysis_pool().install(|| {
            chunk
                .par_iter()
                .filter_map(|file_path| {
                    if cold_closed_file_scan {
                        let source = std::fs::read_to_string(file_path).ok()?;
                        let on_disk_stamp = FileStamp::from_path(file_path);
                        let content_hash = closed_file_scan_key(on_disk_stamp);
                        let file_path_string = file_path.to_string_lossy().to_string();
                        let (analysis, file_deps) =
                            analyze_file_facts_with_deps_and_rbi_with_options_ref(
                                &source,
                                Some(context.user_rbs),
                                Some(context.stdlib_loader),
                                context.lazy_rbi_loader,
                                Some(file_path_string.as_str()),
                                context.options,
                            );
                        let fingerprints = FileFingerprints::from_analysis(&analysis);
                        return Some(WorkspaceScanResult::Analyzed {
                            file_path: file_path_string,
                            content_hash,
                            analysis,
                            fingerprints,
                            file_deps,
                            on_disk_stamp,
                        });
                    }

                    let fp_str = file_path.to_string_lossy().to_string();
                    let (source, on_disk_stamp) = match context.open_docs.get(&fp_str) {
                        Some(source) => (Cow::Borrowed(source.as_str()), None),
                        None => {
                            let current_stamp = FileStamp::from_path(file_path);
                            if let Some(cached) = context.cached_entries.get(&fp_str)
                                && cached.on_disk_stamp == current_stamp
                            {
                                return None;
                            }
                            (
                                Cow::Owned(std::fs::read_to_string(file_path).ok()?),
                                current_stamp,
                            )
                        }
                    };
                    let content_hash = if on_disk_stamp.is_some() {
                        closed_file_scan_key(on_disk_stamp)
                    } else {
                        crate::workspace_state::hash_content(source.as_ref())
                    };
                    if context
                        .cached_entries
                        .get(&fp_str)
                        .is_some_and(|cached| cached.content_hash == content_hash)
                    {
                        return Some(WorkspaceScanResult::RefreshStamp {
                            file_path: fp_str,
                            on_disk_stamp,
                        });
                    }
                    let (analysis, file_deps) =
                        analyze_file_facts_with_deps_and_rbi_with_options_ref(
                            source.as_ref(),
                            Some(context.user_rbs),
                            Some(context.stdlib_loader),
                            context.lazy_rbi_loader,
                            Some(fp_str.as_str()),
                            context.options,
                        );
                    let fingerprints = FileFingerprints::from_analysis(&analysis);
                    Some(WorkspaceScanResult::Analyzed {
                        file_path: fp_str,
                        content_hash,
                        analysis,
                        fingerprints,
                        file_deps,
                        on_disk_stamp,
                    })
                })
                .collect()
        });
        for result in results {
            on_result(result);
        }
    }
}

fn collect_workspace_scan_files(
    scan_roots: &[PathBuf],
    known_files: &[PathBuf],
    pending_scan_files: &[PathBuf],
    open_docs: &HashMap<String, String>,
    analysis_unit_roots: Option<&[PathBuf]>,
    force_full: bool,
) -> Vec<PathBuf> {
    if force_full || known_files.is_empty() {
        // Before discovery finishes, scan the whole root (doesn't degrade to just the open set even with a didOpen-first upsert).
        let mut files: BTreeSet<PathBuf> = collect_rb_files_from_roots(scan_roots)
            .into_iter()
            .collect();
        for file in pending_scan_files {
            if file.extension().is_some_and(|ext| ext == "rb") {
                files.insert(file.clone());
            }
        }
        for file_path in open_docs.keys() {
            if is_target_ruby_path(analysis_unit_roots, file_path) {
                files.insert(PathBuf::from(file_path));
            }
        }
        return files.into_iter().collect();
    }

    let mut files = BTreeSet::new();
    // Incremental rescan covers only dirty/open files (re-stat-ing all known files would burn CPU constantly at scale). With no delta, fall back to all known files as insurance against a missed watcher event.
    let has_explicit_delta = !pending_scan_files.is_empty() || !open_docs.is_empty();
    if !has_explicit_delta {
        for file in known_files {
            files.insert(file.clone());
        }
    }
    for file in pending_scan_files {
        if file.extension().is_some_and(|ext| ext == "rb") {
            files.insert(file.clone());
        }
    }
    for file_path in open_docs.keys() {
        if is_target_ruby_path(analysis_unit_roots, file_path) {
            files.insert(PathBuf::from(file_path));
        }
    }
    files.into_iter().collect()
}

#[cfg(test)]
fn choose_scan_benchmark_target(rb_files: &[PathBuf]) -> Option<PathBuf> {
    let score = |path: &PathBuf| -> (u8, usize, String) {
        let text = path.to_string_lossy();
        let priority = if text.contains("/app/models/")
            && (text.ends_with("/account.rb") || text.ends_with("\\account.rb"))
        {
            0
        } else if text.contains("/app/models/") {
            1
        } else if text.contains("/models/") {
            2
        } else if text.contains("/app/controllers/") {
            3
        } else {
            4
        };
        (priority, text.len(), text.into_owned())
    };

    rb_files.iter().min_by_key(|path| score(path)).cloned()
}

#[cfg(test)]
fn clear_display_caches(state: &mut LspState) {
    state.cached_display.clear();
    state.cached_display_registry.clear();
}

#[cfg(test)]
fn choose_display_probe_files(rb_files: &[PathBuf], state: &LspState) -> Option<(String, String)> {
    let mut candidates: Vec<String> = rb_files
        .iter()
        .filter(|path| path.to_string_lossy().contains("/app/models/"))
        .map(|path| path.to_string_lossy().into_owned())
        .filter(|path| {
            !state
                .workspace_state
                .display_can_skip_workspace_context(path)
        })
        .collect();
    candidates.sort();
    if candidates.len() < 2 {
        return None;
    }
    Some((candidates[0].clone(), candidates[1].clone()))
}

#[cfg(test)]
fn probe_display_registry_build(state: &mut LspState, label: &str, file_path: &str, source: &str) {
    state.workspace_state.last_timings = crate::workspace_state::WorkspaceTimings::default();
    let skip_workspace = state
        .workspace_state
        .display_can_skip_workspace_context(file_path);
    let (analysis, workspace_registry) =
        TydaLsp::analyze_current_file_for_display(state, file_path, source);
    let timings = state.workspace_state.last_timings;
    let basename = std::path::Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(file_path);
    eprintln!(
        "[probe:{label}] file={basename} skip_workspace={skip_workspace} cache_hit={} merge_ms={:.1} clone_ms={:.1} resolve_ms={:.1} total_registry_ms={:.1} current_file_solve_ms={:.1} analysis_classes={} workspace_classes={}",
        timings.display_base_cache_hit,
        timings.display_merge.as_secs_f64() * 1000.0,
        timings.display_clone.as_secs_f64() * 1000.0,
        timings.propagate.as_secs_f64() * 1000.0,
        timings.registry_build.as_secs_f64() * 1000.0,
        timings.current_file_solve.as_secs_f64() * 1000.0,
        analysis.registry().class_count(),
        workspace_registry.class_count(),
    );
    let analysis = timings.analysis_timings;
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    let buckets = [
        ("parse", ms(analysis.parse)),
        ("comments", ms(analysis.comments)),
        ("preload", ms(analysis.preload)),
        ("definition_collection", ms(analysis.definition_collection)),
        ("build_subclass_index", ms(analysis.build_subclass_index)),
        (
            "finalize_pending_scoped_type_refs",
            ms(analysis.finalize_pending_scoped_type_refs),
        ),
        (
            "resolve_subclass_method_refs",
            ms(analysis.resolve_subclass_method_refs),
        ),
        (
            "merge_alias_call_sites",
            ms(analysis.merge_alias_call_sites),
        ),
        (
            "parameter_reference_resolution",
            ms(analysis.parameter_reference_resolution),
        ),
        (
            "resolve_method_return_refs",
            ms(analysis.resolve_method_return_refs),
        ),
        ("backward_propagate", ms(analysis.backward_propagate)),
        (
            "sync_module_function_mirrors",
            ms(analysis.sync_module_function_mirrors),
        ),
        ("receiver_preload", ms(analysis.receiver_reference_preload)),
        ("hover_snapshots", ms(analysis.hover_snapshots)),
        ("deps", ms(analysis.deps)),
        (
            "into_file_analysis_snapshot",
            ms(analysis.into_file_analysis_snapshot),
        ),
        (
            "finalize_call_site_summaries",
            ms(timings.finalize_call_site_summaries),
        ),
    ];
    let bucket_sum_ms: f64 = buckets.iter().map(|(_, value)| value).sum();
    eprintln!("[probe:{label}:solve]");
    for (name, value) in buckets {
        eprintln!("  {name}: {value:.1}ms");
    }
    eprintln!(
        "  bucket_sum: {bucket_sum_ms:.1}ms wall: {:.1}ms delta: {:.1}ms",
        timings.current_file_solve.as_secs_f64() * 1000.0,
        timings.current_file_solve.as_secs_f64() * 1000.0 - bucket_sum_ms,
    );
}

#[derive(Clone, Copy)]
struct CodeLensLatencyProbe {
    generation: u64,
    changed_at: std::time::Instant,
    refresh_sent_at: Option<std::time::Instant>,
}

#[derive(Debug, serde::Deserialize)]
struct TypeprofConfig {
    analysis_unit_dirs: Option<Vec<String>>,
    dsl: Option<Vec<String>>,
}

pub struct TydaLsp {
    client: Client,
    state: Arc<Mutex<LspState>>,
    code_lens_refresh_epoch: Arc<AtomicU64>,
    code_lens_latency_probes: Arc<Mutex<HashMap<String, CodeLensLatencyProbe>>>,
    /// Per-document generation for the `didChange` diagnostics debounce: a pending
    /// publish is dropped once a newer change (or an open/close) bumps its entry.
    diagnostics_epochs: Arc<Mutex<HashMap<Url, u64>>>,
}

struct LspState {
    documents: HashMap<Url, String>,
    document_cache_updates_in_progress: HashMap<String, u64>,
    stdlib_loader: Arc<LazyRbsLoader>,
    user_rbs: Arc<TypeRegistry>,
    workspace_root: Option<PathBuf>,
    analysis_unit_roots: Option<Vec<PathBuf>>,
    signature_enabled: bool,
    output_parameter_names: bool,
    rails_mode: bool,
    dsl_activation: DslActivation,
    project_versions: ProjectVersions,
    lazy_rbi_loader: Option<Arc<LazyRbiLoader>>,
    type_file_classes: HashMap<String, Vec<String>>,
    workspace_state: WorkspaceState,
    workspace_scanned: bool,
    workspace_scan_in_progress: bool,
    // Bumped on every reset of workspace_scanned/workspace_scan_in_progress; lets a
    // stale scan thread detect it was superseded and skip committing its result.
    workspace_scan_generation: u64,
    // Distinct from `workspace_scanned`: don't degrade the cold scan to just the open set until root-wide discovery finishes.
    workspace_fully_discovered: bool,
    // Type-environment changes outside the fingerprint, e.g. `.rbs`/`.rbi` reloads.
    type_env_generation: u64,
    cached_display_registry: DisplayRegistryCache,
    cached_display: DisplayResultCache,
}

const DISPLAY_RESULT_CACHE_LIMIT: usize = 4;

#[derive(Clone)]
struct CachedDisplayRegistry {
    file_path: String,
    display_scope_key: Option<DisplayScopeKey>,
    type_env_generation: u64,
    registry: Arc<TypeRegistry>,
}

#[derive(Clone)]
struct CachedDisplay {
    file_path: String,
    content_hash: u64,
    display_scope_key: Option<DisplayScopeKey>,
    type_env_generation: u64,
    analysis: FileAnalysisSnapshot,
    registry: Arc<TypeRegistry>,
}

#[derive(Default)]
struct DisplayRegistryCache {
    entries: Vec<CachedDisplayRegistry>,
}

impl DisplayRegistryCache {
    fn get(
        &self,
        file_path: &str,
        display_scope_key: &Option<DisplayScopeKey>,
        type_env_generation: u64,
    ) -> Option<&CachedDisplayRegistry> {
        self.entries.iter().find(|cached| {
            cached.file_path == file_path
                && cached.display_scope_key == *display_scope_key
                && cached.type_env_generation == type_env_generation
        })
    }

    fn insert(&mut self, entry: CachedDisplayRegistry) {
        self.entries
            .retain(|cached| cached.file_path != entry.file_path);
        self.entries.insert(0, entry);
        self.entries.truncate(DISPLAY_RESULT_CACHE_LIMIT);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    fn most_recent(&self) -> Option<&CachedDisplayRegistry> {
        self.entries.first()
    }
}

#[derive(Default)]
struct DisplayResultCache {
    entries: Vec<CachedDisplay>,
}

impl DisplayResultCache {
    fn get(
        &self,
        file_path: &str,
        content_hash: u64,
        display_scope_key: &Option<DisplayScopeKey>,
        type_env_generation: u64,
    ) -> Option<&CachedDisplay> {
        self.entries.iter().find(|cached| {
            cached.file_path == file_path
                && cached.content_hash == content_hash
                && cached.display_scope_key == *display_scope_key
                && cached.type_env_generation == type_env_generation
        })
    }

    fn get_for_file(&self, file_path: &str) -> Option<&CachedDisplay> {
        self.entries
            .iter()
            .find(|cached| cached.file_path == file_path)
    }

    fn insert(&mut self, entry: CachedDisplay) {
        self.entries
            .retain(|cached| cached.file_path != entry.file_path);
        self.entries.insert(0, entry);
        self.entries.truncate(DISPLAY_RESULT_CACHE_LIMIT);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    #[cfg(test)]
    fn most_recent(&self) -> Option<&CachedDisplay> {
        self.entries.first()
    }
}

impl TydaLsp {
    fn debug_log(msg: &str) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/tyda-lsp-debug.log")
        {
            let _ = writeln!(
                f,
                "[{:.3}] {}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64(),
                msg
            );
        }
    }

    pub fn new(client: Client) -> Self {
        let core_dir = crate::rbs::workspace::default_vendor_rbs_root().join("core");
        let stdlib_loader = Arc::new(LazyRbsLoader::new(core_dir));
        Self {
            client,
            state: Arc::new(Mutex::new(LspState {
                documents: HashMap::new(),
                document_cache_updates_in_progress: HashMap::new(),
                stdlib_loader,
                user_rbs: Arc::new(TypeRegistry::new()),
                workspace_root: None,
                analysis_unit_roots: None,
                signature_enabled: true,
                output_parameter_names: false,
                rails_mode: false,
                dsl_activation: DslActivation::default(),
                project_versions: ProjectVersions::default(),
                lazy_rbi_loader: None,
                type_file_classes: HashMap::new(),
                workspace_state: WorkspaceState::new(),
                workspace_scanned: false,
                workspace_scan_in_progress: false,
                workspace_scan_generation: 0,
                workspace_fully_discovered: false,
                type_env_generation: 0,
                cached_display_registry: Default::default(),
                cached_display: Default::default(),
            })),
            code_lens_refresh_epoch: Arc::new(AtomicU64::new(0)),
            code_lens_latency_probes: Arc::new(Mutex::new(HashMap::new())),
            diagnostics_epochs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Invalidates any pending debounced diagnostics publish for `uri` and returns the new generation.
    fn next_diagnostics_epoch(&self, uri: &Url) -> u64 {
        let mut epochs = self.diagnostics_epochs.lock().unwrap();
        let epoch = epochs.get(uri).map(|epoch| epoch + 1).unwrap_or(1);
        epochs.insert(uri.clone(), epoch);
        epoch
    }

    fn schedule_diagnostics_publish_after_change(&self, uri: Url, source: String) {
        let epoch = self.next_diagnostics_epoch(&uri);
        let epochs = Arc::clone(&self.diagnostics_epochs);
        let state = Arc::clone(&self.state);
        let client = self.client.clone();
        let delay_ms = diagnostics_change_debounce_ms();
        tokio::spawn(async move {
            if delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }
            if epochs.lock().unwrap().get(&uri).copied() != Some(epoch) {
                return;
            }
            let diagnostics = {
                let mut state = state.lock().unwrap();
                Self::diagnostics_for_document_with_state(&mut state, &uri, &source)
            };
            // Re-checked after the (potentially slow) inference so a change that landed
            // meanwhile isn't overwritten by this stale result.
            if epochs.lock().unwrap().get(&uri).copied() != Some(epoch) {
                return;
            }
            client.publish_diagnostics(uri, diagnostics, None).await;
        });
    }

    fn next_code_lens_refresh_epoch(&self) -> u64 {
        self.code_lens_refresh_epoch.fetch_add(1, Ordering::SeqCst) + 1
    }

    async fn request_code_lens_refresh_now(&self) {
        self.next_code_lens_refresh_epoch();
        self.client.code_lens_refresh().await.unwrap_or_default();
    }

    fn schedule_code_lens_refresh_after_change_for_file(
        &self,
        file_path: Option<String>,
        generation: Option<u64>,
    ) {
        let epoch = self.next_code_lens_refresh_epoch();
        let refresh_epoch = Arc::clone(&self.code_lens_refresh_epoch);
        let client = self.client.clone();
        let probes = Arc::clone(&self.code_lens_latency_probes);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(
                CODE_LENS_CHANGE_REFRESH_DEBOUNCE_MS,
            ))
            .await;
            if refresh_epoch.load(Ordering::SeqCst) != epoch {
                return;
            }
            if let (Some(file_path), Some(generation)) = (file_path.as_ref(), generation) {
                let mut probes = probes.lock().unwrap();
                if let Some(probe) = probes.get_mut(file_path)
                    && probe.generation == generation
                {
                    let refresh_sent_at = std::time::Instant::now();
                    let since_change_ms = refresh_sent_at
                        .duration_since(probe.changed_at)
                        .as_secs_f64()
                        * 1000.0;
                    probe.refresh_sent_at = Some(refresh_sent_at);
                    Self::debug_log(&format!(
                        "code_lens_probe refresh_sent file={} gen={} since_change_ms={:.1}",
                        file_path, generation, since_change_ms
                    ));
                }
            }
            client.code_lens_refresh().await.unwrap_or_default();
        });
    }

    fn record_code_lens_change_probe(&self, file_path: &str) -> u64 {
        let mut probes = self.code_lens_latency_probes.lock().unwrap();
        let generation = probes
            .get(file_path)
            .map(|probe| probe.generation + 1)
            .unwrap_or(1);
        probes.insert(
            file_path.to_string(),
            CodeLensLatencyProbe {
                generation,
                changed_at: std::time::Instant::now(),
                refresh_sent_at: None,
            },
        );
        generation
    }

    fn code_lens_probe_snapshot(&self, file_path: &str) -> Option<CodeLensLatencyProbe> {
        self.code_lens_latency_probes
            .lock()
            .unwrap()
            .get(file_path)
            .copied()
    }

    fn clear_code_lens_probe(&self, file_path: &str, generation: u64) {
        let mut probes = self.code_lens_latency_probes.lock().unwrap();
        if probes
            .get(file_path)
            .is_some_and(|probe| probe.generation == generation)
        {
            probes.remove(file_path);
        }
    }

    fn build_analysis_options(state: &LspState) -> AnalysisOptions {
        AnalysisOptions {
            rails_mode: state.rails_mode,
            dsl_activation: state.dsl_activation.clone(),
            project_versions: state.project_versions,
            project_root: state.workspace_root.clone(),
        }
    }

    fn analyze_file_with_cache(
        &self,
        file_path: &str,
        source: &str,
    ) -> (FileAnalysisSnapshot, crate::dep_graph::FileDeps) {
        let state = self.state.lock().unwrap();
        analyze_file_facts_with_deps_and_rbi(
            source,
            Some(state.user_rbs.as_ref()),
            Some(&state.stdlib_loader),
            state.lazy_rbi_loader.as_deref(),
            Some(file_path),
            Self::build_analysis_options(&state),
        )
    }

    fn should_analyze_ruby_path(&self, file_path: &str) -> bool {
        let state = self.state.lock().unwrap();
        is_target_ruby_path(state.analysis_unit_roots.as_deref(), file_path)
    }

    fn analyze_current_file_for_display(
        state: &mut LspState,
        file_path: &str,
        source: &str,
    ) -> (FileAnalysisSnapshot, Arc<TypeRegistry>) {
        let content_hash = crate::workspace_state::hash_content(source);

        Self::ensure_base_analysis_cached(state, file_path, source);

        let skip_workspace_context = state
            .workspace_state
            .display_can_skip_workspace_context(file_path);
        let display_scope_key =
            (!skip_workspace_context).then(|| state.workspace_state.display_scope_key(file_path));

        if let Some(cached) = state.cached_display.get(
            file_path,
            content_hash,
            &display_scope_key,
            state.type_env_generation,
        ) {
            return (cached.analysis.clone(), Arc::clone(&cached.registry));
        }

        let t0 = std::time::Instant::now();
        let workspace_registry = if skip_workspace_context {
            Arc::new(TypeRegistry::new())
        } else if let Some(cached_registry) = state.cached_display_registry.get(
            file_path,
            &display_scope_key,
            state.type_env_generation,
        ) {
            Arc::clone(&cached_registry.registry)
        } else {
            let user_rbs = state.user_rbs.as_ref();
            let workspace_state = &mut state.workspace_state;
            let registry = workspace_state.workspace_registry_excluding_with_key(
                user_rbs,
                file_path,
                display_scope_key.as_ref().expect("display scope key"),
            );
            state.cached_display_registry.insert(CachedDisplayRegistry {
                file_path: file_path.to_string(),
                display_scope_key: display_scope_key.clone(),
                type_env_generation: state.type_env_generation,
                registry: Arc::clone(&registry),
            });
            registry
        };
        let t_registry = t0.elapsed();

        let t1 = std::time::Instant::now();
        let (mut analysis, _deps, analysis_timings) = analyze_source_for_display(
            source,
            (!skip_workspace_context).then_some(&*workspace_registry),
            Some(&state.stdlib_loader),
            state.lazy_rbi_loader.as_deref(),
            Some(file_path),
            Self::build_analysis_options(state),
        );
        let finalize_call_site_summaries_started = std::time::Instant::now();
        analysis
            .facts
            .registry
            .finalize_pending_call_site_summaries();
        let finalize_call_site_summaries = finalize_call_site_summaries_started.elapsed();
        let t_solve = t1.elapsed();
        analysis.compact_current_pass_facts();
        state.workspace_state.last_timings.current_file_solve = t_solve;
        #[cfg(test)]
        {
            state.workspace_state.last_timings.analysis_timings = analysis_timings;
            state
                .workspace_state
                .last_timings
                .finalize_call_site_summaries = finalize_call_site_summaries;
        }
        #[cfg(not(test))]
        {
            let _ = (analysis_timings, finalize_call_site_summaries);
        }

        Self::debug_log(&format!(
            "display: registry={:.1}ms solve={:.1}ms files={} skip_workspace={}",
            t_registry.as_secs_f64() * 1000.0,
            t_solve.as_secs_f64() * 1000.0,
            state.workspace_state.file_count(),
            skip_workspace_context,
        ));

        state.cached_display.insert(CachedDisplay {
            file_path: file_path.to_string(),
            content_hash,
            display_scope_key,
            type_env_generation: state.type_env_generation,
            analysis: analysis.clone(),
            registry: Arc::clone(&workspace_registry),
        });
        (analysis, workspace_registry)
    }

    fn external_registry_for_completion(
        state: &mut LspState,
        file_path: &str,
        current_source: &str,
    ) -> Option<Arc<TypeRegistry>> {
        let cached_current_source =
            state
                .workspace_state
                .workspace_file(file_path)
                .is_some_and(|entry| {
                    entry.content_hash == crate::workspace_state::hash_content(current_source)
                });
        if cached_current_source
            && state
                .workspace_state
                .display_can_skip_workspace_context(file_path)
        {
            return Self::registry_has_completion_facts(state.user_rbs.as_ref())
                .then(|| Arc::clone(&state.user_rbs));
        }

        let user_rbs = Arc::clone(&state.user_rbs);
        let display_scope_key = state.workspace_state.display_scope_key(file_path);
        Some(state.workspace_state.workspace_registry_excluding_with_key(
            user_rbs.as_ref(),
            file_path,
            &display_scope_key,
        ))
    }

    fn registry_has_completion_facts(registry: &TypeRegistry) -> bool {
        registry.iter_class_data().next().is_some() || !registry.type_aliases().is_empty()
    }

    fn analyze_completion_source(
        state: &LspState,
        source: &str,
        external_registry: Option<&TypeRegistry>,
        file_path: &str,
        mode: QueryAnalysisMode,
    ) -> FileAnalysisSnapshot {
        analyze_file_for_query(
            source,
            external_registry,
            Some(&state.stdlib_loader),
            state.lazy_rbi_loader.as_deref(),
            Some(file_path),
            Self::build_analysis_options(state),
            mode,
        )
    }

    fn ensure_base_analysis_cached(state: &mut LspState, file_path: &str, source: &str) {
        let content_hash = crate::workspace_state::hash_content(source);
        if state
            .workspace_state
            .workspace_file(file_path)
            .is_some_and(|e| e.content_hash == content_hash)
        {
            return;
        }

        let t0 = std::time::Instant::now();
        let options = Self::build_analysis_options(state);
        let (analysis, file_deps) = analyze_file_facts_with_deps_and_rbi(
            source,
            Some(state.user_rbs.as_ref()),
            Some(&state.stdlib_loader),
            state.lazy_rbi_loader.as_deref(),
            Some(file_path),
            options,
        );
        state
            .workspace_state
            .upsert_file(file_path.to_string(), content_hash, analysis, file_deps);
        Self::debug_log(&format!(
            "ensure_base_analysis_cached: {:.1}ms for {}",
            t0.elapsed().as_secs_f64() * 1000.0,
            file_path
        ));
    }

    fn build_code_lenses(&self, uri: &Url, source: &str) -> Vec<CodeLens> {
        let t_total = std::time::Instant::now();
        let (file_path, output_parameter_names) = {
            let state = self.state.lock().unwrap();
            if !state.signature_enabled {
                return Vec::new();
            }
            (uri_to_path(uri), state.output_parameter_names)
        };
        let source_lines = split_source_lines(source);

        let workspace_ready = self.workspace_scan_ready();
        if workspace_ready {
            self.start_open_document_cache_updates_except(uri);
        } else {
            self.start_workspace_scan_if_needed();
        }
        let (visible_methods, semantic_def_lines) = {
            let mut state = self.state.lock().unwrap();
            let options = Self::build_analysis_options(&state);
            let (analysis, workspace_registry) =
                Self::analyze_current_file_for_display(&mut state, &file_path, source);
            let mut methods = dedupe_code_lens_methods(
                analysis.methods_for_file(&file_path),
                output_parameter_names,
            );
            let semantic_def_lines: Vec<u32> = methods
                .iter()
                .filter_map(|(_, sig)| sig.loc.map(|loc| loc.line))
                .collect();
            let covered_lines: std::collections::HashSet<usize> = methods
                .iter()
                .filter_map(|(_, sig)| sig.loc.map(|loc| loc.line.saturating_sub(1) as usize))
                .collect();
            methods.extend(fallback_code_lens_methods_from_source(
                source,
                &covered_lines,
            ));
            let collected = collapse_accessor_code_lens_methods(methods)
                .into_iter()
                .filter_map(|(class_name, sig)| {
                    if !should_show_code_lens(&sig) {
                        return None;
                    }
                    let range = code_lens_range_for_method(source, &sig)?;
                    let line = range.start.line;
                    if source_line_has_direct_annotation(&source_lines, line as usize) {
                        return None;
                    }
                    if !source_line_supports_signature_comment_from_lines(
                        &source_lines,
                        line as usize,
                    ) {
                        return None;
                    }
                    Some((class_name, sig, range))
                })
                .map(|(class_name, sig, range)| {
                    let rbs_text = self.code_lens_rbs_text_with_context(
                        &file_path,
                        source,
                        &sig,
                        output_parameter_names,
                        CodeLensDisplayContext {
                            stdlib_loader: &state.stdlib_loader,
                            workspace_registry: &workspace_registry,
                            options: options.clone(),
                        },
                    );
                    (class_name, sig, range, rbs_text)
                })
                .collect::<Vec<_>>();
            (collected, semantic_def_lines)
        };
        // Only suppresses codelens for semantic methods inside a syntax-error region (same policy as the playground).
        let suppressor = SyntaxErrorSuppressor::new(source, &semantic_def_lines);
        let lenses: Vec<CodeLens> = visible_methods
            .into_iter()
            .filter(|(_, _, range, _)| !suppressor.suppresses_def_line(range.start.line + 1))
            .map(|(class_name, sig, range, rbs_text)| {
                let title = code_lens_title(&rbs_text);
                let command_context = SignatureCommandContext {
                    class_name: class_name.clone(),
                    method_name: sig.name.clone(),
                    is_singleton: sig.is_singleton,
                    original_loc: sig.loc.map(|loc| (loc.line, loc.column)),
                };

                let line = range.start.line;
                let command = Command {
                    title,
                    command: "typeprof.createSignature".to_string(),
                    arguments: Some(vec![
                        serde_json::to_value(uri.as_str()).unwrap(),
                        serde_json::to_value(line).unwrap(),
                        serde_json::to_value(&rbs_text).unwrap(),
                        serde_json::to_value(command_context).unwrap(),
                    ]),
                };

                CodeLens {
                    range,
                    command: Some(command),
                    data: None,
                }
            })
            .collect::<Vec<_>>();
        let titles: Vec<String> = lenses
            .iter()
            .filter_map(|l| {
                let cmd = l.command.as_ref()?;
                Some(format!("L{}:{}", l.range.start.line + 1, cmd.title))
            })
            .collect();
        Self::debug_log(&format!(
            "code_lens done: {} lenses in {:.1}ms titles={:?}",
            lenses.len(),
            t_total.elapsed().as_secs_f64() * 1000.0,
            titles
        ));
        lenses
    }

    fn hover_result_at(
        &self,
        uri: &Url,
        source: &str,
        pos: Position,
    ) -> Option<crate::analysis::HoverResult> {
        let file_path = uri_to_path(uri);
        if !self.should_analyze_ruby_path(&file_path) {
            return None;
        }

        self.ensure_workspace_scanned();
        self.ensure_open_documents_cached_except(uri);
        let byte_offset = lsp_position_to_byte_offset(source, pos)?;
        let mut state = self.state.lock().unwrap();
        let (analysis, workspace_registry) =
            Self::analyze_current_file_for_display(&mut state, &file_path, source);
        // Suppresses hover inside a syntax-error region (same policy as codelens/diagnostics).
        let def_lines: Vec<u32> = analysis
            .methods_for_file(&file_path)
            .iter()
            .filter_map(|(_, sig)| sig.loc.map(|loc| loc.line))
            .collect();
        if SyntaxErrorSuppressor::new(source, &def_lines).suppresses_line(pos.line + 1) {
            return None;
        }
        let hover = TypeQueryEngine::new(&analysis, source, &state.stdlib_loader)
            .with_external_registry(Some(&workspace_registry))
            .hover_at(byte_offset)?;
        Some(enrich_hover_from_definition_context(
            &analysis,
            &file_path,
            source,
            byte_offset,
            hover,
            &state.stdlib_loader,
            &workspace_registry,
        ))
    }

    fn completion_items_at(
        &self,
        uri: &Url,
        source: &str,
        pos: Position,
    ) -> Option<Vec<CompletionItem>> {
        let file_path = uri_to_path(uri);
        if !self.should_analyze_ruby_path(&file_path) {
            return None;
        }
        if let Some(completion_context) = send_method_name_completion_context(source, pos) {
            self.ensure_workspace_scanned();
            self.ensure_open_documents_cached_except(uri);

            let mut state = self.state.lock().unwrap();
            let output_parameter_names = state.output_parameter_names;
            let stdlib_loader = Arc::clone(&state.stdlib_loader);
            let external_registry =
                Self::external_registry_for_completion(&mut state, &file_path, source);
            let analysis = Self::analyze_completion_source(
                &state,
                &completion_context.source,
                external_registry.as_deref(),
                &file_path,
                QueryAnalysisMode::HoverSnapshots,
            );
            let query = TypeQueryEngine::new(&analysis, &completion_context.source, &stdlib_loader)
                .with_external_registry(external_registry.as_deref());
            let completion = match completion_context.receiver {
                SendMethodNameReceiver::Explicit { receiver_offset } => {
                    query.method_completion_at_receiver(receiver_offset)?
                }
                SendMethodNameReceiver::Implicit { class_context } => {
                    let receiver_type = if class_context.is_empty() {
                        Type::Class(Sym::new("Object"))
                    } else {
                        Type::Class(Sym::new(class_context))
                    };
                    query.method_completion_for_receiver_type(receiver_type)
                }
            };
            return Some(method_completion_items(
                completion.candidates,
                completion_context.replace_range,
                output_parameter_names,
            ));
        }
        if let Some(completion_context) = dot_method_completion_context(source, pos) {
            self.ensure_workspace_scanned();
            self.ensure_open_documents_cached_except(uri);

            let mut state = self.state.lock().unwrap();
            let output_parameter_names = state.output_parameter_names;
            let stdlib_loader = Arc::clone(&state.stdlib_loader);
            let external_registry =
                Self::external_registry_for_completion(&mut state, &file_path, source);
            let analysis = Self::analyze_completion_source(
                &state,
                &completion_context.source,
                external_registry.as_deref(),
                &file_path,
                QueryAnalysisMode::HoverSnapshots,
            );
            let completion =
                TypeQueryEngine::new(&analysis, &completion_context.source, &stdlib_loader)
                    .with_external_registry(external_registry.as_deref())
                    .method_completion_at_receiver(completion_context.receiver_offset)?;
            return Some(method_completion_items(
                completion.candidates,
                completion_context.replace_range,
                output_parameter_names,
            ));
        }

        if let Some(completion_context) = double_colon_constant_completion_context(source, pos) {
            self.ensure_workspace_scanned();
            self.ensure_open_documents_cached_except(uri);

            let mut state = self.state.lock().unwrap();
            let stdlib_loader = Arc::clone(&state.stdlib_loader);
            let external_registry =
                Self::external_registry_for_completion(&mut state, &file_path, source);
            let analysis = Self::analyze_completion_source(
                &state,
                &completion_context.source,
                external_registry.as_deref(),
                &file_path,
                QueryAnalysisMode::FileFactsOnly,
            );

            let candidates =
                TypeQueryEngine::new(&analysis, &completion_context.source, &stdlib_loader)
                    .with_external_registry(external_registry.as_deref())
                    .constant_completion_candidates(
                        &completion_context.namespace,
                        &completion_context.class_context,
                        &completion_context.prefix,
                    );
            return Some(constant_completion_items(candidates, &completion_context));
        }

        None
    }

    fn reload_type_definitions(&self) {
        let mut state = self.state.lock().unwrap();
        let Some(ref root) = state.workspace_root else {
            return;
        };
        let root = root.clone();
        let loaded = load_workspace_type_environment(&root, &state.stdlib_loader);
        let mut user_rbs = loaded.user_rbs;
        let type_file_classes = loaded.type_file_classes;
        let lazy_rbi_loader = loaded.lazy_rbi_loader.map(Arc::new);

        let config = load_typeprof_config(&root).ok().flatten();
        let mut dsl_activation = detect_dsl_activation(&root);
        if let Some(config) = config
            && let Some(tokens) = config.dsl
        {
            dsl_activation.apply_cli_spec(&tokens.join(","));
        }
        let rails_mode = if dsl_activation.rails_mode_compat() {
            crate::rails::load_project_types_with_activation(&root, &mut user_rbs, &dsl_activation)
        } else {
            false
        };
        user_rbs.shrink_to_fit_after_compact();

        state.user_rbs = Arc::new(user_rbs);
        state.type_file_classes = type_file_classes;
        state.lazy_rbi_loader = lazy_rbi_loader;
        state.rails_mode = rails_mode;
        state.dsl_activation = dsl_activation;
        state.workspace_state.clear();
        state.workspace_scanned = false;
        state.workspace_scan_in_progress = false;
        state.workspace_scan_generation += 1;
        state.workspace_fully_discovered = false;
        state.type_env_generation += 1;
        state.cached_display_registry.clear();
        state.cached_display.clear();
    }

    fn reload_single_type_file(&self, file_path: &str) -> Vec<String> {
        let mut state = self.state.lock().unwrap();
        let mut type_file_classes = std::mem::take(&mut state.type_file_classes);
        let path = Path::new(file_path);
        let is_rbi = path.extension().is_some_and(|ext| ext == "rbi");
        let affected = if is_rbi {
            if let Some(ref lazy_rbi_loader) = state.lazy_rbi_loader {
                let reload = lazy_rbi_loader.reload_path(path);
                if reload.current_classes.is_empty() {
                    type_file_classes.remove(file_path);
                } else {
                    type_file_classes.insert(file_path.to_string(), reload.current_classes);
                }
                reload.affected_classes
            } else {
                Vec::new()
            }
        } else {
            let mut user_rbs = (*state.user_rbs).clone();
            let affected = reload_external_type_file(
                file_path,
                &mut user_rbs,
                &mut type_file_classes,
                &state.stdlib_loader,
            );
            user_rbs.shrink_to_fit_after_compact();
            state.user_rbs = Arc::new(user_rbs);
            affected
        };
        let defined_symbols = type_file_classes
            .get(file_path)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        state.type_file_classes = type_file_classes;

        let deps = crate::dep_graph::FileDeps {
            defined_symbols,
            referenced_symbols: std::collections::HashSet::new(),
            edges: Vec::new(),
        };
        state
            .workspace_state
            .dep_graph_mut()
            .update_file(file_path, deps);
        state.workspace_state.invalidate_registry();
        state.type_env_generation += 1;
        state.cached_display_registry.clear();
        state.cached_display.clear();

        affected
    }

    fn invalidate_caches_for_symbols(&self, symbols: &std::collections::HashSet<String>) {
        if symbols.is_empty() {
            return;
        }
        let mut state = self.state.lock().unwrap();
        state.workspace_state.invalidate_dependents_of(symbols);
    }

    fn invalidate_file_cache(&self, uri: &Url) {
        let file_path = uri_to_path(uri);
        let mut state = self.state.lock().unwrap();
        state.workspace_state.mark_file_dirty(&file_path);
    }

    fn workspace_scan_ready(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.workspace_scanned
    }

    // True if `scan_generation` (captured when a scan thread started) is still the
    // live generation, i.e. no reset happened while the scan was running. A stale
    // scan must not commit `workspace_scanned`/diagnostics computed against a type
    // environment or dep graph that has since been reset out from under it.
    fn should_commit_workspace_scan(state: &LspState, scan_generation: u64) -> bool {
        state.workspace_scan_generation == scan_generation
    }

    // If a panic doesn't reset `workspace_scan_in_progress`, hover/completion would wait forever.
    fn run_workspace_scan_guarded(state: &Arc<Mutex<LspState>>, scan_generation: u64) -> bool {
        let scan = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::scan_workspace_for_deps_shared(state, scan_generation);
        }));
        if scan.is_ok() {
            return true;
        }
        Self::debug_log("workspace scan panicked; resetting scan flags for retry");
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Only clear the flag this panic actually owns; a newer scan may already
        // have taken over `workspace_scan_in_progress` under a later generation.
        if Self::should_commit_workspace_scan(&state, scan_generation) {
            state.workspace_scan_in_progress = false;
        }
        false
    }

    fn start_workspace_scan_if_needed(&self) {
        let handle = tokio::runtime::Handle::try_current().ok();
        let should_start = {
            let mut state = self.state.lock().unwrap();
            if state.workspace_scanned || state.workspace_scan_in_progress {
                None
            } else {
                state.workspace_scan_in_progress = true;
                Some(state.workspace_scan_generation)
            }
        };
        let Some(scan_generation) = should_start else {
            return;
        };

        let state = Arc::clone(&self.state);
        let client = self.client.clone();
        std::thread::spawn(move || {
            if !Self::run_workspace_scan_guarded(&state, scan_generation) {
                return;
            }
            let scanned_files;
            {
                let mut state = state.lock().unwrap();
                if !Self::should_commit_workspace_scan(&state, scan_generation) {
                    Self::debug_log("workspace scan superseded; discarding stale result");
                    return;
                }
                state.workspace_scanned = true;
                state.workspace_scan_in_progress = false;
                scanned_files = state.workspace_state.file_count();
            }
            if scanned_files >= MEMORY_RECLAIM_MIN_FILES {
                crate::reclaim_freed_memory(Some(lsp_analysis_pool()));
            }
            if let Some(handle) = handle {
                let state_for_diagnostics = Arc::clone(&state);
                handle.spawn(async move {
                    Self::publish_open_document_diagnostics_shared(
                        state_for_diagnostics,
                        client.clone(),
                    )
                    .await;
                    client.code_lens_refresh().await.unwrap_or_default();
                });
            }
        });
    }

    fn start_document_cache_update_if_needed(&self, file_path: String, source: String) {
        let content_hash = crate::workspace_state::hash_content(&source);
        let should_start = {
            let mut state = self.state.lock().unwrap();
            let already_current = state
                .workspace_state
                .workspace_file(&file_path)
                .is_some_and(|entry| entry.content_hash == content_hash);
            let already_pending = state
                .document_cache_updates_in_progress
                .get(&file_path)
                .is_some_and(|pending_hash| *pending_hash == content_hash);
            if already_current || already_pending {
                false
            } else {
                state
                    .document_cache_updates_in_progress
                    .insert(file_path.clone(), content_hash);
                true
            }
        };
        if !should_start {
            return;
        }

        let state = Arc::clone(&self.state);
        // Funnels didChange into a bounded pool (avoids unbounded thread spawns per keystroke).
        lsp_analysis_pool().spawn(move || {
            let (user_rbs, stdlib_loader, lazy_rbi_loader, options) = {
                let state = state.lock().unwrap();
                if state
                    .document_cache_updates_in_progress
                    .get(&file_path)
                    .copied()
                    != Some(content_hash)
                {
                    return;
                }
                (
                    Arc::clone(&state.user_rbs),
                    Arc::clone(&state.stdlib_loader),
                    state.lazy_rbi_loader.clone(),
                    Self::build_analysis_options(&state),
                )
            };
            let (analysis, file_deps) = analyze_file_facts_with_deps_and_rbi(
                &source,
                Some(user_rbs.as_ref()),
                Some(&stdlib_loader),
                lazy_rbi_loader.as_deref(),
                Some(&file_path),
                options,
            );

            let mut state = state.lock().unwrap();
            let Some(pending_hash) = state
                .document_cache_updates_in_progress
                .get(&file_path)
                .copied()
            else {
                return;
            };
            if pending_hash != content_hash {
                return;
            }
            let latest_open_hash = state.documents.iter().find_map(|(uri, text)| {
                (uri_to_path(uri) == file_path).then(|| crate::workspace_state::hash_content(text))
            });
            if latest_open_hash.is_some_and(|latest_hash| latest_hash != content_hash) {
                state.document_cache_updates_in_progress.remove(&file_path);
                return;
            }
            state
                .workspace_state
                .upsert_file(file_path.clone(), content_hash, analysis, file_deps);
            state.document_cache_updates_in_progress.remove(&file_path);
        });
    }

    fn start_open_document_cache_updates_except(&self, current_uri: &Url) {
        let docs_to_update: Vec<(String, String)> = {
            let state = self.state.lock().unwrap();
            state
                .documents
                .iter()
                .filter(|(uri, _)| *uri != current_uri)
                .filter_map(|(uri, source)| {
                    let file_path = uri_to_path(uri);
                    if !is_target_ruby_path(state.analysis_unit_roots.as_deref(), &file_path) {
                        return None;
                    }
                    let content_hash = crate::workspace_state::hash_content(source);
                    if state
                        .workspace_state
                        .workspace_file(&file_path)
                        .is_some_and(|entry| entry.content_hash == content_hash)
                    {
                        return None;
                    }
                    if state
                        .document_cache_updates_in_progress
                        .get(&file_path)
                        .is_some_and(|pending_hash| *pending_hash == content_hash)
                    {
                        return None;
                    }
                    Some((file_path, source.clone()))
                })
                .collect()
        };
        for (file_path, source) in docs_to_update {
            self.start_document_cache_update_if_needed(file_path, source);
        }
    }

    fn ensure_workspace_scanned(&self) {
        loop {
            let (workspace_scanned, workspace_scan_in_progress) = {
                let state = self.state.lock().unwrap();
                (state.workspace_scanned, state.workspace_scan_in_progress)
            };
            if workspace_scanned {
                return;
            }
            if workspace_scan_in_progress {
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
            let scan_generation = {
                let mut state = self.state.lock().unwrap();
                if state.workspace_scanned {
                    return;
                }
                if state.workspace_scan_in_progress {
                    continue;
                }
                state.workspace_scan_in_progress = true;
                state.workspace_scan_generation
            };
            if !Self::run_workspace_scan_guarded(&self.state, scan_generation) {
                // After a panic, don't retry; keep going with what's already known.
                return;
            }
            let mut state = self.state.lock().unwrap();
            if !Self::should_commit_workspace_scan(&state, scan_generation) {
                // Superseded by a reset while scanning; the result is stale — retry.
                drop(state);
                continue;
            }
            state.workspace_scanned = true;
            state.workspace_scan_in_progress = false;
            let scanned_files = state.workspace_state.file_count();
            drop(state);
            if scanned_files >= MEMORY_RECLAIM_MIN_FILES {
                crate::reclaim_freed_memory(Some(lsp_analysis_pool()));
            }
            return;
        }
    }

    fn ensure_open_documents_cached_except(&self, current_uri: &Url) {
        let docs_to_analyze: Vec<(String, String)> = {
            let state = self.state.lock().unwrap();
            state
                .documents
                .iter()
                .filter(|(uri, _)| *uri != current_uri)
                .filter_map(|(uri, source)| {
                    let file_path = uri_to_path(uri);
                    if !is_target_ruby_path(state.analysis_unit_roots.as_deref(), &file_path) {
                        return None;
                    }
                    let content_hash = crate::workspace_state::hash_content(source);
                    if state
                        .workspace_state
                        .workspace_file(&file_path)
                        .is_some_and(|e| e.content_hash == content_hash)
                    {
                        return None;
                    }
                    Some((file_path, source.clone()))
                })
                .collect()
        };
        for (file_path, source) in docs_to_analyze {
            let (analysis, file_deps) = self.analyze_file_with_cache(&file_path, &source);
            let content_hash = crate::workspace_state::hash_content(&source);
            let mut state = self.state.lock().unwrap();
            state
                .workspace_state
                .upsert_file(file_path, content_hash, analysis, file_deps);
        }
    }

    fn scan_workspace_for_deps_shared(state: &Arc<Mutex<LspState>>, scan_generation: u64) {
        let (
            root,
            configured_roots,
            user_rbs,
            stdlib_loader,
            lazy_rbi_loader,
            options,
            cached_entries,
            open_docs,
            known_files,
            pending_scan_files,
            analysis_unit_roots,
            force_full,
        ) = {
            let state_guard = state.lock().unwrap();
            let root = state_guard.workspace_root.clone();
            let configured_roots = state_guard.analysis_unit_roots.clone();
            let user_rbs = Arc::clone(&state_guard.user_rbs);
            let stdlib_loader = Arc::clone(&state_guard.stdlib_loader);
            let lazy_rbi_loader = state_guard.lazy_rbi_loader.clone();
            let options = Self::build_analysis_options(&state_guard);
            let cached_entries: HashMap<String, WorkspaceScanCacheEntry> = state_guard
                .workspace_state
                .file_paths()
                .filter_map(|path| {
                    state_guard
                        .workspace_state
                        .workspace_file(path)
                        .map(|entry| {
                            (
                                path.to_string(),
                                WorkspaceScanCacheEntry {
                                    content_hash: entry.content_hash,
                                    on_disk_stamp: entry.on_disk_stamp,
                                },
                            )
                        })
                })
                .collect();
            let open_docs: HashMap<String, String> = state_guard
                .documents
                .iter()
                .map(|(uri, src)| (uri_to_path(uri), src.clone()))
                .collect();
            let known_files: Vec<PathBuf> = state_guard
                .workspace_state
                .file_paths()
                .map(PathBuf::from)
                .collect();
            let pending_scan_files: Vec<PathBuf> = state_guard
                .workspace_state
                .pending_scan_files()
                .map(PathBuf::from)
                .collect();
            (
                root,
                configured_roots,
                user_rbs,
                stdlib_loader,
                lazy_rbi_loader,
                options,
                cached_entries,
                open_docs,
                known_files,
                pending_scan_files,
                state_guard.analysis_unit_roots.clone(),
                !state_guard.workspace_fully_discovered,
            )
        };
        let Some(root) = root else { return };

        let t_scan = std::time::Instant::now();
        let state_for_abort_check = Arc::clone(state);
        let should_abort = move || {
            let state = state_for_abort_check.lock().unwrap();
            !Self::should_commit_workspace_scan(&state, scan_generation)
        };
        if should_abort() {
            Self::debug_log("workspace scan superseded before walk started; skipping");
            return;
        }
        let scan_roots = configured_roots.unwrap_or_else(|| vec![root]);
        let scan_files = collect_workspace_scan_files(
            &scan_roots,
            &known_files,
            &pending_scan_files,
            &open_docs,
            analysis_unit_roots.as_deref(),
            force_full,
        );
        let t_collect = t_scan.elapsed();
        for_each_workspace_scan_result(
            &scan_files,
            WorkspaceScanInputs {
                cached_entries: &cached_entries,
                open_docs: &open_docs,
                user_rbs: user_rbs.as_ref(),
                stdlib_loader: &stdlib_loader,
                lazy_rbi_loader: lazy_rbi_loader.as_deref(),
                options: &options,
            },
            should_abort,
            |result| {
                let mut state = state.lock().unwrap();
                // The chunk-level abort leaves a window: results analyzed against the
                // pre-reset type environment must not land in the fresh workspace_state.
                if !Self::should_commit_workspace_scan(&state, scan_generation) {
                    return;
                }
                match result {
                    WorkspaceScanResult::Analyzed {
                        file_path,
                        content_hash,
                        analysis,
                        fingerprints,
                        file_deps,
                        on_disk_stamp,
                    } => {
                        state
                            .workspace_state
                            .upsert_scanned_file_with_stamp_and_fingerprints(
                                file_path.clone(),
                                content_hash,
                                analysis,
                                file_deps,
                                on_disk_stamp,
                                fingerprints,
                            );
                        state.workspace_state.remove_pending_scan_file(&file_path);
                    }
                    WorkspaceScanResult::RefreshStamp {
                        file_path,
                        on_disk_stamp,
                    } => {
                        state
                            .workspace_state
                            .refresh_file_stamp(&file_path, on_disk_stamp);
                        state.workspace_state.remove_pending_scan_file(&file_path);
                    }
                }
            },
        );
        let mut state = state.lock().unwrap();
        if !Self::should_commit_workspace_scan(&state, scan_generation) {
            // Aborted partway through: some pending_scan_files were never
            // processed, so leave them marked rather than clearing them all.
            Self::debug_log("workspace scan superseded during walk; leaving pending marks");
            return;
        }
        state.workspace_state.clear_pending_scan_files();
        let user_rbs = Arc::clone(&state.user_rbs);
        state
            .workspace_state
            .warm_display_base_registry(user_rbs.as_ref());
        if force_full {
            state.workspace_fully_discovered = true;
        }
        Self::debug_log(&format!(
            "workspace scan: {} files in {:.1}s (collect {:.1}ms, force_full={})",
            scan_files.len(),
            t_scan.elapsed().as_secs_f64(),
            t_collect.as_secs_f64() * 1000.0,
            force_full,
        ));
    }

    async fn publish_diagnostics_for_document(&self, uri: Url, source: String) {
        let diagnostics = {
            let mut state = self.state.lock().unwrap();
            Self::diagnostics_for_document_with_state(&mut state, &uri, &source)
        };
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn publish_open_document_diagnostics_shared(state: Arc<Mutex<LspState>>, client: Client) {
        let documents: Vec<(Url, String)> = {
            let state = state.lock().unwrap();
            state
                .documents
                .iter()
                .map(|(uri, source)| (uri.clone(), source.clone()))
                .collect()
        };
        for (uri, source) in documents {
            let diagnostics = {
                let mut state = state.lock().unwrap();
                Self::diagnostics_for_document_with_state(&mut state, &uri, &source)
            };
            client.publish_diagnostics(uri, diagnostics, None).await;
        }
    }

    fn diagnostics_for_document_with_state(
        state: &mut LspState,
        uri: &Url,
        source: &str,
    ) -> Vec<Diagnostic> {
        let file_path = uri_to_path(uri);
        if !is_target_ruby_path(state.analysis_unit_roots.as_deref(), &file_path) {
            return Vec::new();
        }
        // Suppresses missing_method/unresolved_constant before the scan finishes (re-issued once it completes).
        let workspace_knowledge_incomplete =
            state.workspace_root.is_some() && !state.workspace_scanned;
        let stdlib_loader = Arc::clone(&state.stdlib_loader);
        let lazy_rbi_loader = state.lazy_rbi_loader.clone();
        let (analysis, workspace_registry) =
            Self::analyze_current_file_for_display(state, &file_path, source);
        let mut diagnostics = method_call_lsp_diagnostics(
            &analysis,
            source,
            &stdlib_loader,
            lazy_rbi_loader.as_deref(),
            Some(&workspace_registry),
        );
        // Suppresses syntax-error regions and same-line diagnostic comments.
        let def_lines: Vec<u32> = analysis
            .methods_for_file(&file_path)
            .iter()
            .filter_map(|(_, sig)| sig.loc.map(|loc| loc.line))
            .collect();
        let suppressor = SyntaxErrorSuppressor::new(source, &def_lines);
        let unused_ignore_diagnostics = if workspace_knowledge_incomplete {
            Vec::new()
        } else {
            unused_ignore_lsp_diagnostics(&diagnostics, source, &suppressor)
        };
        diagnostics.retain(|diag| {
            let line = diag.range.start.line + 1;
            let syntax_suppressed = suppressor.suppresses_line(line);
            let comment_suppressed = match &diag.code {
                Some(NumberOrString::String(code)) => suppressor.suppresses_diagnostic(line, code),
                _ => false,
            };
            !syntax_suppressed && !comment_suppressed
        });
        if workspace_knowledge_incomplete {
            diagnostics.retain(|diag| {
                !matches!(
                    &diag.code,
                    Some(NumberOrString::String(code))
                        if code == MISSING_METHOD_DIAGNOSTIC_CODE
                            || code == UNRESOLVED_CONSTANT_DIAGNOSTIC_CODE
                )
            });
        }
        diagnostics.extend(unused_ignore_diagnostics);
        diagnostics
    }
}

fn line_col_to_offset(source: &[u8], target_line: usize, target_col: usize) -> Option<usize> {
    let mut line = 1;
    let mut col = 0;
    for (i, &b) in source.iter().enumerate() {
        if line == target_line && col == target_col {
            return Some(i);
        }
        if b == b'\n' {
            if line == target_line {
                return Some(i);
            }
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    if line == target_line && col == target_col {
        Some(source.len())
    } else {
        None
    }
}

fn scan_roots_from_typeprof_config(workspace_root: &Path) -> Option<Vec<PathBuf>> {
    let config = load_typeprof_config(workspace_root).ok()??;
    let dirs = config.analysis_unit_dirs?;
    if dirs.is_empty() {
        return None;
    }

    let resolved: Vec<PathBuf> = dirs
        .into_iter()
        .map(|dir| workspace_root.join(dir))
        .filter(|path| path.exists())
        .collect();

    Some(resolved)
}

fn is_target_ruby_path(analysis_unit_roots: Option<&[PathBuf]>, file_path: &str) -> bool {
    let path = Path::new(file_path);
    let is_ruby = path.extension().is_some_and(|ext| ext == "rb");
    if !is_ruby {
        return false;
    }

    let Some(roots) = analysis_unit_roots else {
        return true;
    };
    roots.iter().any(|root| path.starts_with(root))
}

fn load_typeprof_config(workspace_root: &Path) -> std::io::Result<Option<TypeprofConfig>> {
    let config_path = workspace_root.join("typeprof.conf.jsonc");
    if !config_path.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(config_path)?;
    let stripped = strip_jsonc_comments(&raw);
    let config = serde_json::from_str::<TypeprofConfig>(&stripped).ok();
    Ok(config)
}

// Small workspaces skip the rebuild reclaim (the walk cost would exceed the pages actually returned).
const MEMORY_RECLAIM_MIN_FILES: usize = 256;

fn build_hover_workspace_registry(
    state: &mut LspState,
    _current_file_path: &str,
) -> Arc<TypeRegistry> {
    // The demand-driven post-pass is disabled (same reason as the CLI; see roadmap); global resolution runs on the bounded pool instead.
    let user_rbs = Arc::clone(&state.user_rbs);
    let ws = &mut state.workspace_state;
    let version_before = ws.registry_version();
    let registry = lsp_analysis_pool().install(|| ws.workspace_registry(user_rbs.as_ref()));
    // Reclaims via a pool worker only after a rebuild (a cached hit has an unchanged version, so it's skipped).
    if ws.registry_version() != version_before && ws.file_count() >= MEMORY_RECLAIM_MIN_FILES {
        crate::reclaim_freed_memory(Some(lsp_analysis_pool()));
    }
    registry
}

fn source_location_to_range(loc: SourceLocation) -> Range {
    let start = Position::new(loc.line.saturating_sub(1), loc.column);
    Range::new(start, start)
}

fn source_location_to_location(file_path: &str, loc: SourceLocation) -> Option<Location> {
    let uri = Url::from_file_path(file_path).ok()?;
    Some(Location::new(uri, source_location_to_range(loc)))
}

fn method_call_lsp_diagnostics(
    analysis: &FileAnalysisSnapshot,
    source: &str,
    stdlib_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    workspace_registry: Option<&TypeRegistry>,
) -> Vec<Diagnostic> {
    let (unresolved, mismatches, unresolved_constants) =
        analysis.method_call_diagnostics(stdlib_loader, lazy_rbi_loader, workspace_registry);
    let mut diagnostics: Vec<Diagnostic> = unresolved
        .into_iter()
        .map(|call| {
            let range = Range::new(
                byte_offset_to_lsp_position(source, call.start),
                byte_offset_to_lsp_position(source, call.end),
            );
            Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String(
                    MISSING_METHOD_DIAGNOSTIC_CODE.to_string(),
                )),
                source: Some("Tyda".to_string()),
                message: missing_method_diagnostic_message(
                    &call.method_name,
                    &call.unresolved_method,
                ),
                ..Default::default()
            }
        })
        .collect();
    diagnostics.extend(mismatches.into_iter().map(|mismatch| {
        let range = Range::new(
            byte_offset_to_lsp_position(source, mismatch.start),
            byte_offset_to_lsp_position(source, mismatch.end),
        );
        Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(
                ARGUMENT_TYPE_MISMATCH_DIAGNOSTIC_CODE.to_string(),
            )),
            source: Some("Tyda".to_string()),
            message: crate::diagnostics::argument_type_mismatch_message(
                &mismatch.param_name,
                &mismatch.expected,
                &mismatch.actual,
            ),
            ..Default::default()
        }
    }));
    diagnostics.extend(unresolved_constants.into_iter().map(|constant| {
        let range = Range::new(
            byte_offset_to_lsp_position(source, constant.start),
            byte_offset_to_lsp_position(source, constant.end),
        );
        Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::INFORMATION),
            code: Some(NumberOrString::String(
                UNRESOLVED_CONSTANT_DIAGNOSTIC_CODE.to_string(),
            )),
            source: Some("Tyda".to_string()),
            message: crate::diagnostics::unresolved_constant_message(&constant.name),
            ..Default::default()
        }
    }));
    diagnostics
}

fn unused_ignore_lsp_diagnostics(
    diagnostics: &[Diagnostic],
    source: &str,
    suppressor: &SyntaxErrorSuppressor,
) -> Vec<Diagnostic> {
    suppressor
        .diagnostic_comment_lines()
        .into_iter()
        .filter_map(|line| {
            if suppressor.suppresses_line(line)
                || diagnostics.iter().any(|diagnostic| {
                    let diagnostic_line = diagnostic.range.start.line + 1;
                    diagnostic_line == line
                        && match &diagnostic.code {
                            Some(NumberOrString::String(code)) => {
                                suppressor.suppresses_diagnostic(line, code)
                            }
                            _ => false,
                        }
                })
            {
                return None;
            }
            let (byte_start, byte_end) = suppressor.diagnostic_comment_range(line)?;
            Some(Diagnostic {
                range: Range::new(
                    byte_offset_to_lsp_position(source, byte_start),
                    byte_offset_to_lsp_position(source, byte_end),
                ),
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String(
                    UNUSED_IGNORE_DIAGNOSTIC_CODE.to_string(),
                )),
                source: Some("Tyda".to_string()),
                message: "Diagnostic ignore comment does not match any diagnostic on this line"
                    .to_string(),
                ..Default::default()
            })
        })
        .collect()
}

fn missing_method_diagnostic_message(method_name: &str, unresolved_method: &str) -> String {
    if let Some((owner, method)) = unresolved_method.rsplit_once('#') {
        return format!("Method `{method}` not found for `{owner}`");
    }
    format!("Method `{method_name}` not found")
}

fn push_unique_location(
    locations: &mut Vec<Location>,
    seen: &mut BTreeSet<(String, u32, u32)>,
    location: Location,
) {
    let key = (
        location.uri.to_string(),
        location.range.start.line,
        location.range.start.character,
    );
    if seen.insert(key) {
        locations.push(location);
    }
}

fn collect_type_definition_class_names(ty: &Type, names: &mut BTreeSet<String>) {
    match ty {
        Type::Union(parts) => {
            for part in parts {
                collect_type_definition_class_names(part, names);
            }
        }
        _ => {
            if let Some(name) = TypeRegistry::type_to_class_name_pub(ty) {
                names.insert(name);
            }
        }
    }
}

fn find_decl_range_in_type_file(content: &str, class_name: &str) -> Option<Range> {
    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len().saturating_sub(trimmed.len());
        for keyword in ["class ", "module "] {
            let Some(rest) = trimmed.strip_prefix(keyword) else {
                continue;
            };
            if !rest.starts_with(class_name) {
                continue;
            }
            let next = rest.chars().nth(class_name.len());
            if !matches!(next, None | Some(' ') | Some('<') | Some('[')) {
                continue;
            }
            let start = Position::new(line_index as u32, (indent + keyword.len()) as u32);
            let end = Position::new(
                line_index as u32,
                (indent + keyword.len() + class_name.len()) as u32,
            );
            return Some(Range::new(start, end));
        }
    }
    None
}

fn find_type_file_location(file_path: &str, class_name: &str) -> Option<Location> {
    let content = std::fs::read_to_string(file_path).ok()?;
    let range = find_decl_range_in_type_file(&content, class_name)?;
    let uri = Url::from_file_path(file_path).ok()?;
    Some(Location::new(uri, range))
}

fn collect_type_definition_locations(
    state: &mut LspState,
    current_file_path: &str,
    ty: &Type,
) -> Vec<Location> {
    let mut class_names = BTreeSet::new();
    collect_type_definition_class_names(ty, &mut class_names);

    let cached_display = state
        .cached_display
        .get_for_file(current_file_path)
        .map(|cached| (cached.analysis.clone(), Arc::clone(&cached.registry)));
    let full_workspace_registry = cached_display
        .is_none()
        .then(|| build_hover_workspace_registry(state, current_file_path));

    let mut locations = Vec::new();
    let mut seen = BTreeSet::new();
    for class_name in class_names {
        let class_data = cached_display
            .as_ref()
            .and_then(|(analysis, workspace_registry)| {
                analysis
                    .registry()
                    .class_data_for(&class_name)
                    .or_else(|| workspace_registry.class_data_for(&class_name))
            })
            .or_else(|| {
                full_workspace_registry
                    .as_ref()
                    .and_then(|registry| registry.class_data_for(&class_name))
            });

        if let Some(data) = class_data
            && let (Some(file_path), Some(loc)) = (data.file_path.as_deref(), data.loc)
            && let Some(location) = source_location_to_location(file_path, loc)
        {
            push_unique_location(&mut locations, &mut seen, location);
        }

        let mut type_file_paths: Vec<&String> = state
            .type_file_classes
            .iter()
            .filter_map(|(file_path, class_names)| {
                class_names.contains(&class_name).then_some(file_path)
            })
            .collect();
        type_file_paths.sort();
        for file_path in type_file_paths {
            if let Some(location) = find_type_file_location(file_path, &class_name) {
                push_unique_location(&mut locations, &mut seen, location);
            }
        }
    }
    locations
}

fn collect_rbs_comment_type_definition_locations(
    state: &mut LspState,
    current_file_path: &str,
    analysis: &FileAnalysisSnapshot,
    workspace_registry: &TypeRegistry,
    type_name: &str,
    class_context: &str,
) -> Vec<Location> {
    let mut merged_registry = workspace_registry.clone();
    analysis.apply_to_registry(&mut merged_registry);
    let mut locations = collect_type_name_definition_locations_from_registry(
        state,
        &merged_registry,
        type_name,
        class_context,
    );
    if !locations.is_empty() {
        return locations;
    }

    let full_workspace_registry = build_hover_workspace_registry(state, current_file_path);
    let mut full_registry = (*full_workspace_registry).clone();
    analysis.apply_to_registry(&mut full_registry);
    locations = collect_type_name_definition_locations_from_registry(
        state,
        &full_registry,
        type_name,
        class_context,
    );
    locations
}

fn collect_type_name_definition_locations_from_registry(
    state: &LspState,
    registry: &TypeRegistry,
    type_name: &str,
    class_context: &str,
) -> Vec<Location> {
    let Some(class_name) = registry.resolve_class_name_for_type_name(type_name, class_context)
    else {
        return Vec::new();
    };

    let mut locations = Vec::new();
    let mut seen = BTreeSet::new();
    push_class_definition_locations(state, registry, &class_name, &mut locations, &mut seen);
    locations
}

fn push_class_definition_locations(
    state: &LspState,
    registry: &TypeRegistry,
    class_name: &str,
    locations: &mut Vec<Location>,
    seen: &mut BTreeSet<(String, u32, u32)>,
) {
    if let Some(data) = registry.class_data_for(class_name)
        && let (Some(file_path), Some(loc)) = (data.file_path.as_deref(), data.loc)
        && let Some(location) = source_location_to_location(file_path, loc)
    {
        push_unique_location(locations, seen, location);
    }

    let mut type_file_paths: Vec<&String> = state
        .type_file_classes
        .iter()
        .filter_map(|(file_path, class_names)| {
            class_names
                .iter()
                .any(|name| name == class_name)
                .then_some(file_path)
        })
        .collect();
    type_file_paths.sort();
    for file_path in type_file_paths {
        if let Some(location) = find_type_file_location(file_path, class_name) {
            push_unique_location(locations, seen, location);
        }
    }
}

fn method_definition_location_from_registry(
    registry: &TypeRegistry,
    class_name: &str,
    method_name: &str,
    prefer_singleton: bool,
    exact_singleton: Option<bool>,
) -> Option<Location> {
    let (file_path, loc) = if let Some(is_singleton) = exact_singleton {
        registry.lookup_method_definition_location_exact(class_name, method_name, is_singleton)?
    } else {
        registry.lookup_method_definition_location_with_hint(
            class_name,
            method_name,
            prefer_singleton,
        )?
    };
    source_location_to_location(&file_path, loc)
}

fn method_definition_location_from_registry_for_dispatch(
    registry: &TypeRegistry,
    class_name: &str,
    method_name: &str,
    method_is_singleton: bool,
) -> Option<Location> {
    let (file_path, loc) = registry.lookup_method_definition_location_for_dispatch(
        class_name,
        method_name,
        method_is_singleton,
    )?;
    source_location_to_location(&file_path, loc)
}

fn constant_definition_location_from_registry(
    registry: &TypeRegistry,
    owner: &str,
    name: &str,
) -> Option<Location> {
    let (file_path, loc) =
        registry.lookup_constant_definition_location_through_ancestors(owner, name)?;
    source_location_to_location(&file_path, loc)
}

#[derive(Clone, Copy)]
struct MethodDefinitionLookup<'a> {
    class_name: &'a str,
    method_name: &'a str,
    prefer_singleton: bool,
    exact_singleton: Option<bool>,
}

fn push_method_definition_location(
    analysis_registry: &TypeRegistry,
    workspace_registry: &TypeRegistry,
    locations: &mut Vec<Location>,
    seen: &mut BTreeSet<(String, u32, u32)>,
    lookup: MethodDefinitionLookup<'_>,
) {
    let location = method_definition_location_from_registry(
        analysis_registry,
        lookup.class_name,
        lookup.method_name,
        lookup.prefer_singleton,
        lookup.exact_singleton,
    )
    .or_else(|| {
        method_definition_location_from_registry(
            workspace_registry,
            lookup.class_name,
            lookup.method_name,
            lookup.prefer_singleton,
            lookup.exact_singleton,
        )
    });
    if let Some(location) = location {
        push_unique_location(locations, seen, location);
    }
}

fn push_method_dispatch_definition_location(
    analysis_registry: &TypeRegistry,
    workspace_registry: &TypeRegistry,
    locations: &mut Vec<Location>,
    seen: &mut BTreeSet<(String, u32, u32)>,
    class_name: &str,
    method_name: &str,
    method_is_singleton: bool,
) -> bool {
    let before = locations.len();
    let location = method_definition_location_from_registry_for_dispatch(
        analysis_registry,
        class_name,
        method_name,
        method_is_singleton,
    )
    .or_else(|| {
        method_definition_location_from_registry_for_dispatch(
            workspace_registry,
            class_name,
            method_name,
            method_is_singleton,
        )
    });
    if let Some(location) = location {
        push_unique_location(locations, seen, location);
    }
    locations.len() > before
}

fn collect_method_call_definition_locations(
    analysis_registry: &TypeRegistry,
    workspace_registry: &TypeRegistry,
    receiver_type: &Type,
    method_name: &str,
    locations: &mut Vec<Location>,
    seen: &mut BTreeSet<(String, u32, u32)>,
) {
    match receiver_type {
        Type::Union(parts) => {
            for part in parts {
                collect_method_call_definition_locations(
                    analysis_registry,
                    workspace_registry,
                    part,
                    method_name,
                    locations,
                    seen,
                );
            }
        }
        _ => {
            if let Some(class_name) = TypeRegistry::type_to_class_name_pub(receiver_type) {
                if matches!(receiver_type, Type::Singleton(_)) && method_name == "new" {
                    let found_new = push_method_dispatch_definition_location(
                        analysis_registry,
                        workspace_registry,
                        locations,
                        seen,
                        &class_name,
                        "new",
                        true,
                    );
                    if !found_new {
                        push_method_dispatch_definition_location(
                            analysis_registry,
                            workspace_registry,
                            locations,
                            seen,
                            &class_name,
                            "initialize",
                            false,
                        );
                    }
                    return;
                }

                let prefer_singleton = matches!(receiver_type, Type::Singleton(_));
                push_method_definition_location(
                    analysis_registry,
                    workspace_registry,
                    locations,
                    seen,
                    MethodDefinitionLookup {
                        class_name: &class_name,
                        method_name,
                        prefer_singleton,
                        exact_singleton: None,
                    },
                );
            }
        }
    }
}

fn collect_definition_locations(
    state: &mut LspState,
    current_file_path: &str,
    analysis: &FileAnalysisSnapshot,
    workspace_registry: &TypeRegistry,
    target: DefinitionLookupTarget,
) -> Vec<Location> {
    let mut locations = Vec::new();
    let mut seen = BTreeSet::new();
    match target {
        DefinitionLookupTarget::Source(loc) => {
            if let Some(location) = source_location_to_location(current_file_path, loc) {
                push_unique_location(&mut locations, &mut seen, location);
            }
        }
        DefinitionLookupTarget::Constant { owner, name } => {
            let location =
                constant_definition_location_from_registry(analysis.registry(), &owner, &name)
                    .or_else(|| {
                        constant_definition_location_from_registry(
                            workspace_registry,
                            &owner,
                            &name,
                        )
                    });
            if let Some(location) = location {
                push_unique_location(&mut locations, &mut seen, location);
            }
        }
        DefinitionLookupTarget::TypeDefinition(ty) => {
            locations = collect_type_definition_locations(state, current_file_path, &ty);
        }
        DefinitionLookupTarget::MethodCall {
            receiver_type,
            method_name,
        } => {
            collect_method_call_definition_locations(
                analysis.registry(),
                workspace_registry,
                &receiver_type,
                &method_name,
                &mut locations,
                &mut seen,
            );
        }
        DefinitionLookupTarget::MethodDefinition {
            owner_type,
            method_name,
            is_singleton,
        } => {
            if let Some(class_name) = TypeRegistry::type_to_class_name_pub(&owner_type) {
                push_method_definition_location(
                    analysis.registry(),
                    workspace_registry,
                    &mut locations,
                    &mut seen,
                    MethodDefinitionLookup {
                        class_name: &class_name,
                        method_name: &method_name,
                        prefer_singleton: is_singleton,
                        exact_singleton: Some(is_singleton),
                    },
                );
            }
        }
    }
    locations
}

fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_string {
            result.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                result.push(ch);
            }
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                for next in chars.by_ref() {
                    if next == '\n' {
                        result.push('\n');
                        break;
                    }
                }
            }
            _ => result.push(ch),
        }
    }

    result
}

#[tower_lsp::async_trait]
impl LanguageServer for TydaLsp {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let output_parameter_names =
            output_parameter_names_from_initialize_options(params.initialization_options.as_ref());
        if let Some(root_uri) = params.root_uri
            && let Ok(root_path) = root_uri.to_file_path()
        {
            let analysis_unit_roots = scan_roots_from_typeprof_config(&root_path);
            let project_versions = ProjectVersions::detect(&root_path);
            let vendor_rbs_root = crate::rbs::workspace::default_vendor_rbs_root();
            {
                let mut state = self.state.lock().unwrap();
                state.workspace_root = Some(root_path);
                state.analysis_unit_roots = analysis_unit_roots;
                state.output_parameter_names = output_parameter_names;
                state.project_versions = project_versions;
                state.stdlib_loader = Arc::new(LazyRbsLoader::for_ruby_version(
                    vendor_rbs_root,
                    project_versions.effective_ruby(),
                ));
            }
            self.reload_type_definitions();
        } else {
            let mut state = self.state.lock().unwrap();
            state.output_parameter_names = output_parameter_names;
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        ..Default::default()
                    },
                )),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "typeprof.createSignature".to_string(),
                        "typeprof.enableSignature".to_string(),
                        "typeprof.disableSignature".to_string(),
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let register_options = DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![
                FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/*.rb".to_string()),
                    kind: Some(WatchKind::all()),
                },
                FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/*.rbs".to_string()),
                    kind: Some(WatchKind::all()),
                },
                FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/*.rbi".to_string()),
                    kind: Some(WatchKind::all()),
                },
            ],
        };

        let registration = Registration {
            id: "tyda-file-watcher".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(serde_json::to_value(register_options).unwrap()),
        };

        let _ = self.client.register_capability(vec![registration]).await;

        self.client
            .send_notification::<notification::ShowMessage>(ShowMessageParams {
                typ: MessageType::INFO,
                message: "Tyda LSP server ready".to_string(),
            })
            .await;

        let _ = self
            .client
            .send_notification::<TypeprofEnableToggleButton>(())
            .await;

        self.start_workspace_scan_if_needed();
    }

    async fn shutdown(&self) -> Result<()> {
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            std::process::exit(0);
        });
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let file_path = uri_to_path(&uri);
        if !self.should_analyze_ruby_path(&file_path) {
            return;
        }
        self.invalidate_file_cache(&uri);
        {
            let mut state = self.state.lock().unwrap();
            state.documents.insert(uri.clone(), text.clone());
        }
        // Starts the cold scan even for clients that send didOpen before `initialized`
        // (idempotent: a no-op if a scan is already done or in progress).
        self.start_workspace_scan_if_needed();
        let diagnostics_source = text.clone();
        self.start_document_cache_update_if_needed(file_path, text);
        self.next_diagnostics_epoch(&uri);
        self.publish_diagnostics_for_document(uri, diagnostics_source)
            .await;
        self.request_code_lens_refresh_now().await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let file_path = uri_to_path(&uri);
        if !self.should_analyze_ruby_path(&file_path) {
            return;
        }
        Self::debug_log(&format!("did_change: {}", file_path));
        self.invalidate_file_cache(&uri);
        let updated_source = {
            let mut state = self.state.lock().unwrap();
            let current_source = state
                .documents
                .get(&uri)
                .cloned()
                .or_else(|| std::fs::read_to_string(&file_path).ok())
                .unwrap_or_default();
            let Some(updated_source) =
                apply_content_changes(&current_source, &params.content_changes)
            else {
                Self::debug_log(&format!(
                    "did_change: failed to apply change for {}",
                    file_path
                ));
                return;
            };
            state.documents.insert(uri.clone(), updated_source.clone());
            updated_source
        };
        Self::debug_log(&format!(
            "did_change: applied {} changes, {} bytes",
            params.content_changes.len(),
            updated_source.len()
        ));
        let probe_generation = self.record_code_lens_change_probe(&file_path);
        Self::debug_log(&format!(
            "code_lens_probe change_received file={} gen={}",
            file_path, probe_generation
        ));
        self.start_document_cache_update_if_needed(file_path, updated_source.clone());
        self.schedule_diagnostics_publish_after_change(uri.clone(), updated_source);
        self.schedule_code_lens_refresh_after_change_for_file(
            Some(uri_to_path(&uri)),
            Some(probe_generation),
        );
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let file_path = uri_to_path(&params.text_document.uri);
        if !self.should_analyze_ruby_path(&file_path) {
            return;
        }
        self.invalidate_file_cache(&params.text_document.uri);
        {
            let mut state = self.state.lock().unwrap();
            state.documents.remove(&params.text_document.uri);
            state.document_cache_updates_in_progress.remove(&file_path);
        }
        // Drops any pending debounced publish for this document.
        self.diagnostics_epochs
            .lock()
            .unwrap()
            .remove(&params.text_document.uri);
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let mut all_affected_symbols: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut has_non_open_rb_change = false;

        for change in &params.changes {
            let path = change.uri.path();
            if path.ends_with(".rbs") || path.ends_with(".rbi") {
                let file_path = uri_to_path(&change.uri);
                let affected = self.reload_single_type_file(&file_path);
                all_affected_symbols.extend(affected);
            } else if path.ends_with(".rb") {
                {
                    let state = self.state.lock().unwrap();
                    if state.documents.contains_key(&change.uri) {
                        continue;
                    }
                }
                let file_path = uri_to_path(&change.uri);
                if !self.should_analyze_ruby_path(&file_path) {
                    continue;
                }
                match change.typ {
                    FileChangeType::DELETED => {
                        let mut state = self.state.lock().unwrap();
                        let old_defs = state.workspace_state.dep_graph().definitions_of(&file_path);
                        all_affected_symbols.extend(old_defs.into_iter().flatten());
                        state.workspace_state.remove_file(&file_path);
                        state.workspace_state.remove_pending_scan_file(&file_path);
                    }
                    _ => {
                        has_non_open_rb_change = true;
                        {
                            let state = self.state.lock().unwrap();
                            let old_defs =
                                state.workspace_state.dep_graph().definitions_of(&file_path);
                            all_affected_symbols.extend(old_defs.into_iter().flatten());
                        }
                        self.invalidate_file_cache(&change.uri);
                        let mut state = self.state.lock().unwrap();
                        state.workspace_state.mark_file_pending_scan(file_path);
                    }
                }
            }
        }

        if has_non_open_rb_change || !all_affected_symbols.is_empty() {
            {
                let mut state = self.state.lock().unwrap();
                state.workspace_scanned = false;
                state.workspace_scan_in_progress = false;
                state.workspace_scan_generation += 1;
            }
            self.invalidate_caches_for_symbols(&all_affected_symbols);
            self.start_workspace_scan_if_needed();
            self.request_code_lens_refresh_now().await;
        }
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        let file_path = uri_to_path(&uri);
        Self::debug_log(&format!("code_lens requested: {}", file_path));
        if !self.should_analyze_ruby_path(&file_path) {
            return Ok(Some(Vec::new()));
        }
        let source = {
            let state = self.state.lock().unwrap();
            state.documents.get(&uri).cloned()
        };
        let source = match source {
            Some(s) => s,
            None => {
                let path = uri_to_path(&uri);
                match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(_) => return Ok(None),
                }
            }
        };
        let t0 = std::time::Instant::now();
        let probe = self.code_lens_probe_snapshot(&file_path);
        if let Some(probe) = probe {
            let since_change_ms = probe.changed_at.elapsed().as_secs_f64() * 1000.0;
            let since_refresh_ms = probe
                .refresh_sent_at
                .map(|refresh_sent_at| refresh_sent_at.elapsed().as_secs_f64() * 1000.0);
            Self::debug_log(&format!(
                "code_lens_probe request_start file={} gen={} since_change_ms={:.1} since_refresh_ms={}",
                file_path,
                probe.generation,
                since_change_ms,
                since_refresh_ms
                    .map(|value| format!("{value:.1}"))
                    .unwrap_or_else(|| "none".to_string())
            ));
        }
        let lenses = self.build_code_lenses(&uri, &source);
        Self::debug_log(&format!(
            "code_lens done: {} lenses in {:.1}ms for {}",
            lenses.len(),
            t0.elapsed().as_secs_f64() * 1000.0,
            file_path
        ));
        if let Some(probe) = probe {
            let since_change_ms = probe.changed_at.elapsed().as_secs_f64() * 1000.0;
            let since_refresh_ms = probe
                .refresh_sent_at
                .map(|refresh_sent_at| refresh_sent_at.elapsed().as_secs_f64() * 1000.0);
            Self::debug_log(&format!(
                "code_lens_probe request_done file={} gen={} since_change_ms={:.1} since_refresh_ms={} build_ms={:.1} lenses={}",
                file_path,
                probe.generation,
                since_change_ms,
                since_refresh_ms
                    .map(|value| format!("{value:.1}"))
                    .unwrap_or_else(|| "none".to_string()),
                t0.elapsed().as_secs_f64() * 1000.0,
                lenses.len()
            ));
            self.clear_code_lens_probe(&file_path, probe.generation);
        }
        Ok(Some(lenses))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let file_path = uri_to_path(&uri);
        if !self.should_analyze_ruby_path(&file_path) {
            return Ok(None);
        }

        let source = {
            let state = self.state.lock().unwrap();
            state.documents.get(&uri).cloned()
        };
        let source = match source {
            Some(s) => s,
            None => match std::fs::read_to_string(&file_path) {
                Ok(s) => s,
                Err(_) => return Ok(None),
            },
        };

        let items = self
            .completion_items_at(&uri, &source, pos)
            .unwrap_or_default();
        Ok(Some(CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items,
        })))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_path = uri_to_path(&uri);
        if !self.should_analyze_ruby_path(&file_path) {
            return Ok(None);
        }

        let source = {
            let state = self.state.lock().unwrap();
            state.documents.get(&uri).cloned()
        };
        let source = match source {
            Some(s) => s,
            None => {
                let path = uri_to_path(&uri);
                match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(_) => return Ok(None),
                }
            }
        };
        let result = self.hover_result_at(&uri, &source, pos);

        match result {
            Some(hover_result) => {
                let contents = format_hover_contents(&hover_result);
                Ok(Some(Hover {
                    contents,
                    range: None,
                }))
            }
            None => Ok(None),
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_path = uri_to_path(&uri);
        if !self.should_analyze_ruby_path(&file_path) {
            return Ok(None);
        }

        let source = {
            let state = self.state.lock().unwrap();
            state.documents.get(&uri).cloned()
        };
        let source = match source {
            Some(s) => s,
            None => match std::fs::read_to_string(&file_path) {
                Ok(s) => s,
                Err(_) => return Ok(None),
            },
        };

        self.ensure_workspace_scanned();
        self.ensure_open_documents_cached_except(&uri);
        let Some(byte_offset) = lsp_position_to_byte_offset(&source, pos) else {
            return Ok(None);
        };
        let source_bytes = source.as_bytes();
        if source_bytes.get(byte_offset).is_some_and(|byte| {
            *byte == b'.'
                || (*byte == b':'
                    && (source_bytes.get(byte_offset + 1) == Some(&b':')
                        || byte_offset
                            .checked_sub(1)
                            .is_some_and(|offset| source_bytes.get(offset) == Some(&b':'))))
        }) {
            return Ok(None);
        }

        let locations = {
            let mut state = self.state.lock().unwrap();
            let (analysis, workspace_registry) =
                Self::analyze_current_file_for_display(&mut state, &file_path, &source);
            if let Some(context) = rbs_comment_type_definition_context(&source, byte_offset) {
                let locations = collect_rbs_comment_type_definition_locations(
                    &mut state,
                    &file_path,
                    &analysis,
                    &workspace_registry,
                    &context.type_name,
                    &context.class_context,
                );
                if !locations.is_empty() {
                    return Ok(Some(GotoDefinitionResponse::Array(locations)));
                }
            }
            let Some(target) = analysis.definition_lookup_target_at(
                byte_offset,
                &state.stdlib_loader,
                Some(&workspace_registry),
            ) else {
                return Ok(None);
            };
            collect_definition_locations(
                &mut state,
                &file_path,
                &analysis,
                &workspace_registry,
                target,
            )
        };

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(GotoDefinitionResponse::Array(locations)))
        }
    }

    async fn goto_type_definition(
        &self,
        params: request::GotoTypeDefinitionParams,
    ) -> Result<Option<request::GotoTypeDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_path = uri_to_path(&uri);
        if !self.should_analyze_ruby_path(&file_path) {
            return Ok(None);
        }

        let source = {
            let state = self.state.lock().unwrap();
            state.documents.get(&uri).cloned()
        };
        let source = match source {
            Some(s) => s,
            None => match std::fs::read_to_string(&file_path) {
                Ok(s) => s,
                Err(_) => return Ok(None),
            },
        };

        let Some(hover_result) = self.hover_result_at(&uri, &source, pos) else {
            return Ok(None);
        };
        let locations = {
            let mut state = self.state.lock().unwrap();
            collect_type_definition_locations(&mut state, &file_path, &hover_result.ty)
        };
        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(request::GotoTypeDefinitionResponse::Array(locations)))
        }
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        match params.command.as_str() {
            "typeprof.createSignature" if params.arguments.len() >= 3 => {
                let uri_str = params.arguments[0].as_str().unwrap_or_default();
                let fallback_line = params.arguments[1].as_u64().unwrap_or(0) as u32;
                let rbs_text = params.arguments[2].as_str().unwrap_or_default();
                let command_context = params
                    .arguments
                    .get(3)
                    .and_then(|value| serde_json::from_value(value.clone()).ok());

                if let Ok(uri) = Url::parse(uri_str) {
                    let source = {
                        let state = self.state.lock().unwrap();
                        state
                            .documents
                            .get(&uri)
                            .cloned()
                            .or_else(|| std::fs::read_to_string(uri_to_path(&uri)).ok())
                    };
                    let Some(source) = source else {
                        return Ok(None);
                    };
                    let file_path = uri_to_path(&uri);
                    let target_line = {
                        let mut state = self.state.lock().unwrap();
                        resolve_signature_insertion_line(
                            &mut state,
                            &file_path,
                            &source,
                            fallback_line,
                            command_context.as_ref(),
                        )
                    };
                    let Some(updated_source) =
                        insert_signature_comment(&source, target_line, rbs_text)
                    else {
                        Self::debug_log(&format!(
                            "createSignature: failed to insert comment for {} at line {}",
                            file_path,
                            target_line + 1
                        ));
                        return Ok(None);
                    };
                    let def_line = source.lines().nth(target_line as usize).unwrap_or_default();
                    let trimmed = def_line.trim_start();
                    let indent = &def_line[..def_line.len().saturating_sub(trimmed.len())];
                    let new_text = format!("{indent}#: {rbs_text}\n");
                    let position = Position::new(target_line, 0);

                    let edit = WorkspaceEdit {
                        changes: Some(HashMap::from([(
                            uri.clone(),
                            vec![TextEdit {
                                range: Range::new(position, position),
                                new_text,
                            }],
                        )])),
                        ..Default::default()
                    };

                    if let Ok(response) = self.client.apply_edit(edit).await
                        && response.applied
                    {
                        {
                            let mut state = self.state.lock().unwrap();
                            state.documents.insert(uri.clone(), updated_source.clone());
                        }
                        self.start_document_cache_update_if_needed(
                            file_path.clone(),
                            updated_source,
                        );
                        self.invalidate_file_cache(&uri);
                        self.request_code_lens_refresh_now().await;
                    }
                }
            }
            "typeprof.enableSignature" => {
                {
                    let mut state = self.state.lock().unwrap();
                    state.signature_enabled = true;
                }
                self.request_code_lens_refresh_now().await;
            }
            "typeprof.disableSignature" => {
                {
                    let mut state = self.state.lock().unwrap();
                    state.signature_enabled = false;
                }
                self.request_code_lens_refresh_now().await;
            }
            _ => {}
        }
        Ok(None)
    }
}

struct TypeprofEnableToggleButton;
impl notification::Notification for TypeprofEnableToggleButton {
    type Params = ();
    const METHOD: &'static str = "typeprof.enableToggleButton";
}

fn output_parameter_names_from_initialize_options(options: Option<&serde_json::Value>) -> bool {
    let Some(options) = options else {
        return false;
    };
    options
        .get("output_parameter_names")
        .and_then(|value| value.as_bool())
        .or_else(|| {
            options
                .get("typeprof")
                .and_then(|value| value.get("output_parameter_names"))
                .and_then(|value| value.as_bool())
        })
        .unwrap_or(false)
}

type AccessorCodeLensKey = (String, String, bool, u32, u32);
type AccessorCodeLensPair = (Option<usize>, Option<usize>);
type CodeLensMethodKey = (String, String, bool, Option<(u32, u32)>);

fn code_lens_title(rbs_text: &str) -> String {
    format!("#: {rbs_text}")
}

fn collapse_accessor_code_lens_methods(
    methods: Vec<(String, MethodSig)>,
) -> Vec<(String, MethodSig)> {
    let mut pairs: HashMap<AccessorCodeLensKey, AccessorCodeLensPair> = HashMap::new();
    for (idx, (class_name, sig)) in methods.iter().enumerate() {
        let Some(loc) = sig.loc else {
            continue;
        };
        let base_name = sig.name.strip_suffix('=').unwrap_or(&sig.name).to_string();
        let entry = pairs
            .entry((
                class_name.clone(),
                base_name,
                sig.is_singleton,
                loc.line,
                loc.column,
            ))
            .or_insert((None, None));
        if sig.name.ends_with('=') {
            entry.1 = Some(idx);
        } else {
            entry.0 = Some(idx);
        }
    }

    let mut collapsed = HashMap::new();
    let mut skipped = HashSet::new();
    for (getter_idx, setter_idx) in pairs.values() {
        let (Some(getter_idx), Some(setter_idx)) = (getter_idx, setter_idx) else {
            continue;
        };
        let getter = &methods[*getter_idx].1;
        let setter = &methods[*setter_idx].1;
        let Some(combined) = collapse_accessor_pair(getter, setter) else {
            continue;
        };
        collapsed.insert(*getter_idx, combined);
        skipped.insert(*setter_idx);
    }

    methods
        .into_iter()
        .enumerate()
        .filter_map(|(idx, (class_name, sig))| {
            if skipped.contains(&idx) {
                return None;
            }
            Some((class_name, collapsed.remove(&idx).unwrap_or(sig)))
        })
        .collect()
}

#[cfg(test)]
fn collapse_accessor_pair(getter: &MethodSig, setter: &MethodSig) -> Option<MethodSig> {
    if setter.name != format!("{}=", getter.name)
        || !getter.params.is_empty()
        || !getter.overloads.is_empty()
        || !setter.overloads.is_empty()
        || getter.return_type != setter.return_type
        || getter.loc != setter.loc
    {
        return None;
    }
    let [setter_param] = setter.params.as_slice() else {
        return None;
    };
    if !matches!(setter_param.kind, ParamKind::Required | ParamKind::Optional) {
        return None;
    }

    let mut combined = getter.clone();
    combined.params = vec![crate::types::Param {
        name: setter_param.name.clone(),
        param_type: setter_param.param_type.clone(),
        kind: ParamKind::Optional,
    }];
    Some(combined)
}

#[cfg(not(test))]
fn collapse_accessor_pair(getter: &MethodSig, setter: &MethodSig) -> Option<MethodSig> {
    if setter.name != format!("{}=", getter.name)
        || !getter.params.is_empty()
        || !getter.overloads.is_empty()
        || !setter.overloads.is_empty()
        || getter.return_type != setter.return_type
        || getter.loc != setter.loc
    {
        return None;
    }
    let [setter_param] = setter.params.as_slice() else {
        return None;
    };
    if !matches!(setter_param.kind, ParamKind::Required | ParamKind::Optional) {
        return None;
    }

    let mut combined = getter.clone();
    combined.params = vec![crate::types::Param {
        name: setter_param.name.clone(),
        param_type: setter_param.param_type.clone(),
        kind: ParamKind::Optional,
    }];
    Some(combined)
}

fn format_hover_contents(hover_result: &crate::analysis::HoverResult) -> HoverContents {
    HoverContents::Scalar(MarkedString::LanguageString(LanguageString {
        language: "rbs".to_string(),
        value: format_hover_body(hover_result),
    }))
}

#[cfg(test)]
fn hover_needs_workspace_fallback(hover: &crate::analysis::HoverResult) -> bool {
    if hover.can_enrich_from_workspace {
        return true;
    }
    if has_type_hole(&hover.ty) {
        return true;
    }
    if let Some(display_rbs) = &hover.display_rbs {
        return contains_unresolved_marker(display_rbs);
    }
    false
}

fn split_top_level_segments(input: &str, delimiter: char) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    for (idx, ch) in input.char_indices() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
        if ch == delimiter && paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
            segments.push(input[start..idx].trim());
            start = idx + ch.len_utf8();
        }
    }
    segments.push(input[start..].trim());
    segments
}

fn contains_unresolved_marker(segment: &str) -> bool {
    segment.contains("untyped")
}

fn hover_signature_unresolved_counts(display: &str) -> (usize, usize) {
    display
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            if let Some(arrow_idx) = line.rfind("->") {
                let (params_part, return_part) = line.split_at(arrow_idx);
                let return_part = return_part.trim_start_matches("->").trim();
                let return_count = usize::from(contains_unresolved_marker(return_part));
                let mut param_count = 0;
                if let (Some(open_idx), Some(close_idx)) =
                    (params_part.find('('), params_part.rfind(')'))
                {
                    let params = &params_part[open_idx + 1..close_idx];
                    for segment in split_top_level_segments(params, ',') {
                        if !segment.is_empty() && contains_unresolved_marker(segment) {
                            param_count += 1;
                        }
                    }
                } else if contains_unresolved_marker(params_part) {
                    param_count += 1;
                }
                (param_count, return_count)
            } else if contains_unresolved_marker(line) {
                (0, 1)
            } else {
                (0, 0)
            }
        })
        .fold(
            (0, 0),
            |(param_total, return_total), (param_count, return_count)| {
                (param_total + param_count, return_total + return_count)
            },
        )
}

fn hover_untyped_slots(display: &str) -> usize {
    let (param_count, return_count) = hover_signature_unresolved_counts(display);
    param_count + return_count
}

#[cfg(test)]
fn hover_untyped_count(hover: &crate::analysis::HoverResult) -> usize {
    if let Some(display) = &hover.display_rbs {
        hover_untyped_slots(display)
    } else {
        usize::from(matches!(hover.ty, Type::Untyped | Type::Todo))
    }
}

fn signature_union_count(display: &str) -> usize {
    display
        .lines()
        .map(|line| line.matches(" | ").count())
        .sum()
}

fn is_better_signature_display(candidate_display: &str, current_display: &str) -> bool {
    let (candidate_param_unresolved, candidate_return_unresolved) =
        hover_signature_unresolved_counts(candidate_display);
    let (current_param_unresolved, current_return_unresolved) =
        hover_signature_unresolved_counts(current_display);
    if candidate_param_unresolved != current_param_unresolved {
        return candidate_param_unresolved < current_param_unresolved;
    }
    if candidate_return_unresolved != current_return_unresolved {
        return candidate_return_unresolved < current_return_unresolved;
    }

    let candidate_untyped = hover_untyped_slots(candidate_display);
    let current_untyped = hover_untyped_slots(current_display);
    if candidate_untyped != current_untyped {
        return candidate_untyped < current_untyped;
    }

    let candidate_variants = signature_union_count(candidate_display);
    let current_variants = signature_union_count(current_display);
    if candidate_variants != current_variants {
        return candidate_variants > current_variants;
    }

    false
}

fn is_better_code_lens_sig(
    candidate: &MethodSig,
    current: &MethodSig,
    output_parameter_names: bool,
) -> bool {
    let candidate_display =
        format_method_sig_for_lens_with_names(candidate, output_parameter_names);
    let current_display = format_method_sig_for_lens_with_names(current, output_parameter_names);
    is_better_signature_display(&candidate_display, &current_display)
}

fn dedupe_code_lens_methods(
    methods: Vec<(String, MethodSig)>,
    output_parameter_names: bool,
) -> Vec<(String, MethodSig)> {
    let mut deduped: HashMap<CodeLensMethodKey, (String, MethodSig)> = HashMap::new();
    for (class_name, method) in methods {
        let key = (
            class_name.clone(),
            method.name.clone(),
            method.is_singleton,
            method.loc.map(|loc| (loc.line, loc.column)),
        );
        match deduped.get_mut(&key) {
            Some((_, existing)) => {
                if is_better_code_lens_sig(&method, existing, output_parameter_names) {
                    *existing = method;
                }
            }
            None => {
                deduped.insert(key, (class_name, method));
            }
        }
    }
    deduped.into_values().collect()
}

#[cfg(test)]
fn is_better_hover_result(
    candidate: &crate::analysis::HoverResult,
    current: &crate::analysis::HoverResult,
) -> bool {
    match (
        candidate.display_rbs.is_some(),
        current.display_rbs.is_some(),
    ) {
        (true, false) => return true,
        (false, true) => return false,
        _ => {}
    }
    let candidate_untyped = hover_untyped_count(candidate);
    let current_untyped = hover_untyped_count(current);
    if candidate_untyped != current_untyped {
        return candidate_untyped < current_untyped;
    }
    if let (Some(candidate_display), Some(current_display)) = (
        candidate.display_rbs.as_deref(),
        current.display_rbs.as_deref(),
    ) && is_better_signature_display(candidate_display, current_display)
    {
        return true;
    }
    if candidate.ty != current.ty {
        if !matches!(candidate.ty, Type::Untyped | Type::Todo)
            && matches!(current.ty, Type::Untyped | Type::Todo)
        {
            return true;
        }
        if matches!(candidate.ty, Type::Untyped | Type::Todo)
            && !matches!(current.ty, Type::Untyped | Type::Todo)
        {
            return false;
        }
        let candidate_parts = hover_widened_type_parts(&candidate.ty);
        let current_parts = hover_widened_type_parts(&current.ty);
        if candidate_parts != current_parts {
            if candidate_parts.is_superset(&current_parts)
                && candidate_parts.len() > current_parts.len()
            {
                return true;
            }
            if current_parts.is_superset(&candidate_parts)
                && current_parts.len() > candidate_parts.len()
            {
                return false;
            }
        } else {
            let candidate_literal = hover_literal_specificity(&candidate.ty);
            let current_literal = hover_literal_specificity(&current.ty);
            if candidate_literal != current_literal {
                return candidate_literal > current_literal;
            }
        }
    }
    false
}

#[cfg(test)]
fn hover_widened_type_parts(ty: &Type) -> BTreeSet<String> {
    let mut parts = BTreeSet::new();
    collect_hover_widened_type_parts(ty, &mut parts);
    parts
}

#[cfg(test)]
fn collect_hover_widened_type_parts(ty: &Type, parts: &mut BTreeSet<String>) {
    match ty {
        Type::Union(inner) => {
            for part in inner {
                collect_hover_widened_type_parts(part, parts);
            }
        }
        _ => {
            parts.insert(ty.widen().to_string());
        }
    }
}

#[cfg(test)]
fn hover_literal_specificity(ty: &Type) -> usize {
    match ty {
        Type::LiteralInteger(_)
        | Type::LiteralFloat(_)
        | Type::LiteralString(_)
        | Type::LiteralSymbol(_)
        | Type::True
        | Type::False => 1,
        Type::Union(parts) => parts.iter().map(hover_literal_specificity).sum(),
        _ => 0,
    }
}

#[cfg(test)]
fn choose_better_hover_result(
    primary: Option<crate::analysis::HoverResult>,
    fallback: Option<crate::analysis::HoverResult>,
) -> Option<crate::analysis::HoverResult> {
    match (primary, fallback) {
        (Some(primary), Some(fallback)) => {
            if is_better_hover_result(&fallback, &primary) {
                Some(fallback)
            } else {
                Some(primary)
            }
        }
        (Some(primary), None) => Some(primary),
        (None, Some(fallback)) => Some(fallback),
        (None, None) => None,
    }
}

fn should_show_code_lens(sig: &MethodSig) -> bool {
    !sig.rbs_file_source && !sig.rbs_inline_annotated && !sig.sig_annotated
}

#[cfg(test)]
fn resolve_code_lens_method_sig(
    analysis: &FileAnalysisSnapshot,
    source: &str,
    method: &MethodSig,
    lazy_loader: &LazyRbsLoader,
    workspace_registry: &TypeRegistry,
) -> Option<MethodSig> {
    let byte_offset = method_definition_name_offset(source, method)?;
    let mut resolved =
        analysis.method_definition_sig_at(byte_offset, lazy_loader, Some(workspace_registry))?;
    resolved.is_singleton = method.is_singleton;
    resolved.rbs_annotated = method.rbs_annotated;
    resolved.rbs_inline_annotated = method.rbs_inline_annotated;
    resolved.sig_annotated = method.sig_annotated;
    resolved.rbs_file_source = method.rbs_file_source;
    resolved.synthetic_dsl_source = method.synthetic_dsl_source;
    resolved.loc = method.loc;
    Some(resolved)
}

fn resolve_signature_insertion_line(
    state: &mut LspState,
    file_path: &str,
    source: &str,
    fallback_line: u32,
    context: Option<&SignatureCommandContext>,
) -> u32 {
    let Some(context) = context else {
        return fallback_line;
    };

    let (analysis, _workspace_registry) =
        TydaLsp::analyze_current_file_for_display(state, file_path, source);
    let methods = collapse_accessor_code_lens_methods(dedupe_code_lens_methods(
        analysis.methods_for_file(file_path),
        state.output_parameter_names,
    ));
    methods
        .into_iter()
        .filter(|(class_name, sig)| {
            class_name == &context.class_name
                && sig.name == context.method_name
                && sig.is_singleton == context.is_singleton
        })
        .min_by_key(|(_, sig)| {
            let fallback_distance = sig
                .loc
                .map(|loc| loc.line.abs_diff(fallback_line + 1))
                .unwrap_or(u32::MAX);
            let original_distance = match (sig.loc, context.original_loc) {
                (Some(loc), Some((line, column))) => {
                    (loc.line.abs_diff(line), loc.column.abs_diff(column))
                }
                _ => (u32::MAX, u32::MAX),
            };
            (original_distance.0, original_distance.1, fallback_distance)
        })
        .and_then(|(_, sig)| sig.loc.map(|loc| loc.line.saturating_sub(1)))
        .unwrap_or(fallback_line)
}

fn method_sig_for_definition_line(
    analysis: &FileAnalysisSnapshot,
    file_path: &str,
    source: &str,
    byte_offset: usize,
) -> Option<MethodSig> {
    let (line, _col) = offset_to_line_col(source.as_bytes(), byte_offset);
    let mut matches: Vec<MethodSig> = analysis
        .methods_for_file(file_path)
        .into_iter()
        .map(|(_class_name, sig)| sig)
        .filter(|sig| sig.loc.is_some_and(|loc| loc.line as usize == line))
        .collect();
    if matches.len() == 1 {
        return matches.pop();
    }
    let method_name_offset = method_name_offset_for_definition_line(source, byte_offset)?;
    matches
        .into_iter()
        .find(|sig| method_definition_name_offset(source, sig) == Some(method_name_offset))
}

fn enrich_hover_from_definition_context(
    analysis: &FileAnalysisSnapshot,
    file_path: &str,
    source: &str,
    byte_offset: usize,
    hover: crate::analysis::HoverResult,
    lazy_loader: &LazyRbsLoader,
    workspace_registry: &TypeRegistry,
) -> crate::analysis::HoverResult {
    let from_methods = method_sig_for_definition_line(analysis, file_path, source, byte_offset);
    let from_definition = method_name_offset_for_definition_line(source, byte_offset).and_then(
        |method_name_offset| {
            analysis.method_definition_sig_at(
                method_name_offset,
                lazy_loader,
                Some(workspace_registry),
            )
        },
    );
    let Some(sig) = (match (from_methods, from_definition) {
        (Some(methods_sig), Some(definition_sig)) => {
            if is_better_code_lens_sig(&definition_sig, &methods_sig, true) {
                Some(definition_sig)
            } else {
                Some(methods_sig)
            }
        }
        (Some(sig), None) | (None, Some(sig)) => Some(sig),
        (None, None) => None,
    }) else {
        return hover;
    };

    if hover.name == sig.name {
        let display_rbs = crate::rbs::display::format_hover_callable_type(&sig);
        if hover.display_rbs.as_deref() != Some(display_rbs.as_str())
            || matches!(hover.ty, Type::Untyped | Type::Todo)
        {
            return crate::analysis::HoverResult {
                ty: sig.return_type.clone(),
                display_rbs: Some(display_rbs),
                ..hover
            };
        }
        return hover;
    }

    if let Some(param) = sig.params.iter().find(|param| param.name == hover.name)
        && !matches!(param.param_type, Type::Untyped | Type::Todo)
    {
        return crate::analysis::HoverResult {
            ty: param.param_type.clone(),
            display_rbs: None,
            ..hover
        };
    }

    hover
}

impl TydaLsp {
    fn code_lens_rbs_text_with_context(
        &self,
        file_path: &str,
        source: &str,
        sig: &MethodSig,
        output_parameter_names: bool,
        context: CodeLensDisplayContext<'_>,
    ) -> String {
        let current = format_method_sig_for_lens_with_names(sig, output_parameter_names);
        if !contains_unresolved_marker(&current) {
            return current;
        }
        let Some(byte_offset) = method_definition_name_offset(source, sig) else {
            return current;
        };
        let (line, col) = offset_to_line_col(source.as_bytes(), byte_offset);
        let Some(fallback) = crate::analysis::hover_at_with_analysis_options(
            source,
            Some(context.workspace_registry),
            context.stdlib_loader,
            file_path,
            line,
            col,
            context.options,
        ) else {
            return current;
        };
        let Some(candidate) = fallback.display_rbs.as_deref().map(|display| {
            display
                .strip_prefix(&format!("{}: ", sig.name))
                .unwrap_or(display)
                .to_string()
        }) else {
            return current;
        };
        if is_better_signature_display(&candidate, &current) {
            candidate
        } else {
            current
        }
    }
}

#[derive(Clone)]
struct CodeLensDisplayContext<'a> {
    stdlib_loader: &'a LazyRbsLoader,
    workspace_registry: &'a TypeRegistry,
    options: AnalysisOptions,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SignatureCommandContext {
    class_name: String,
    method_name: String,
    is_singleton: bool,
    original_loc: MethodLocationKey,
}

type MethodLocationKey = Option<(u32, u32)>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ParamKind;
    use futures::{SinkExt, StreamExt};
    use tempfile::tempdir;
    use tower::Service;
    use tower_lsp::ClientSocket;
    use tower_lsp::jsonrpc::{Request, Response};

    static MASTODON_BENCH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn mastodon_bench_guard() -> std::sync::MutexGuard<'static, ()> {
        MASTODON_BENCH_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("mastodon bench lock poisoned")
    }

    fn stdlib_loader() -> Arc<crate::rbs::stdlib_loader::LazyRbsLoader> {
        let core_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        Arc::new(crate::rbs::stdlib_loader::LazyRbsLoader::new(core_dir))
    }

    #[allow(dead_code)]
    fn new_test_state() -> LspState {
        LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        }
    }

    fn insert_test_cache(state: &mut LspState, file_path: &str, analysis: FileAnalysisSnapshot) {
        let deps = crate::dep_graph::FileDeps::default();
        state
            .workspace_state
            .upsert_file(file_path.to_string(), 0, analysis, deps);
    }

    fn insert_analyzed_test_file(state: &mut LspState, file_path: &str, source: &str) {
        let (analysis, deps) = analyze_file_facts_with_deps_and_rbi(
            source,
            Some(state.user_rbs.as_ref()),
            Some(&state.stdlib_loader),
            state.lazy_rbi_loader.as_deref(),
            Some(file_path),
            TydaLsp::build_analysis_options(state),
        );
        state.workspace_state.upsert_file(
            file_path.to_string(),
            crate::workspace_state::hash_content(source),
            analysis,
            deps,
        );
    }

    async fn respond_ok(socket: &mut ClientSocket, request: &Request, result: serde_json::Value) {
        if let Some(id) = request.id().cloned() {
            socket
                .send(Response::from_ok(id, result))
                .await
                .expect("send client response");
        }
    }

    async fn drive_request(
        service: &mut tower_lsp::LspService<TydaLsp>,
        socket: &mut ClientSocket,
        request: Request,
    ) -> (Option<Response>, Vec<Request>) {
        let server_call = Service::call(service, request);
        tokio::pin!(server_call);

        let mut client_requests = Vec::new();
        loop {
            tokio::select! {
                response = &mut server_call => {
                    let response = response.expect("server call");
                    while let Ok(Some(request)) = tokio::time::timeout(
                        std::time::Duration::from_millis(150),
                        socket.next(),
                    )
                    .await
                    {
                        let result = if request.method() == "workspace/applyEdit" {
                            serde_json::json!({ "applied": true })
                        } else {
                            serde_json::json!(null)
                        };
                        respond_ok(socket, &request, result).await;
                        client_requests.push(request);
                    }
                    return (response, client_requests);
                }
                maybe_request = socket.next() => {
                    let request = maybe_request.expect("client request stream closed");
                    let result = if request.method() == "workspace/applyEdit" {
                        serde_json::json!({ "applied": true })
                    } else {
                        serde_json::json!(null)
                    };
                    respond_ok(socket, &request, result).await;
                    client_requests.push(request);
                }
            }
        }
    }

    async fn initialize_lsp(
        root_uri: Option<Url>,
    ) -> (tower_lsp::LspService<TydaLsp>, ClientSocket) {
        let (mut service, mut socket) = tower_lsp::LspService::new(TydaLsp::new);

        let params = serde_json::json!({
            "capabilities": {},
            "rootUri": root_uri,
        });
        let response = Service::call(
            &mut service,
            Request::build("initialize").id(1).params(params).finish(),
        )
        .await
        .expect("initialize request")
        .expect("initialize response");
        assert!(response.is_ok(), "initialize failed: {response:?}");

        let (_, requests) = drive_request(
            &mut service,
            &mut socket,
            Request::build("initialized")
                .params(serde_json::json!({}))
                .finish(),
        )
        .await;
        let methods: Vec<_> = requests
            .iter()
            .map(|request| request.method().to_string())
            .collect();
        assert!(
            methods
                .iter()
                .any(|method| method == "client/registerCapability")
        );
        assert!(methods.iter().any(|method| method == "window/showMessage"));
        assert!(
            methods
                .iter()
                .any(|method| method == "typeprof.enableToggleButton")
        );

        (service, socket)
    }

    async fn open_document(
        service: &mut tower_lsp::LspService<TydaLsp>,
        socket: &mut ClientSocket,
        uri: &Url,
        text: &str,
    ) -> Vec<Request> {
        let (_, requests) = drive_request(
            service,
            socket,
            Request::build("textDocument/didOpen")
                .params(serde_json::json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": "ruby",
                        "version": 1,
                        "text": text,
                    }
                }))
                .finish(),
        )
        .await;
        requests
    }

    async fn request_definition_locations_maybe(
        service: &mut tower_lsp::LspService<TydaLsp>,
        uri: &Url,
        line: u32,
        character: u32,
        id: i64,
    ) -> Option<Vec<Location>> {
        let response = Service::call(
            service,
            Request::build("textDocument/definition")
                .id(id)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }))
                .finish(),
        )
        .await
        .expect("definition request")
        .expect("definition response");
        let value = response.result().cloned()?;
        if value.is_null() {
            return None;
        }
        Some(serde_json::from_value(value).expect("decode definition"))
    }

    async fn request_definition_locations(
        service: &mut tower_lsp::LspService<TydaLsp>,
        uri: &Url,
        line: u32,
        character: u32,
        id: i64,
    ) -> Vec<Location> {
        request_definition_locations_maybe(service, uri, line, character, id)
            .await
            .unwrap_or_default()
    }

    async fn request_definition_locations_at(
        service: &mut tower_lsp::LspService<TydaLsp>,
        uri: &Url,
        position: Position,
        id: i64,
    ) -> Vec<Location> {
        request_definition_locations(service, uri, position.line, position.character, id).await
    }

    async fn assert_no_definition_at(
        service: &mut tower_lsp::LspService<TydaLsp>,
        uri: &Url,
        position: Position,
        id: i64,
    ) {
        let locations =
            request_definition_locations_maybe(service, uri, position.line, position.character, id)
                .await;
        assert!(
            locations.as_ref().is_none_or(Vec::is_empty),
            "expected no definition at {position:?}, got {locations:?}"
        );
    }

    fn assert_single_definition_location(
        locations: &[Location],
        uri: &Url,
        position: Position,
        context: &str,
    ) {
        assert_eq!(locations.len(), 1, "{context}");
        assert_eq!(locations[0].uri, *uri, "{context}");
        assert_eq!(locations[0].range.start, position, "{context}");
    }

    fn position_of(source: &str, needle: &str) -> Position {
        let offset = source
            .find(needle)
            .unwrap_or_else(|| panic!("missing needle {needle:?}"));
        byte_offset_to_lsp_position(source, offset)
    }

    async fn request_hover(
        service: &mut tower_lsp::LspService<TydaLsp>,
        uri: &Url,
        line: u32,
        character: u32,
        id: i64,
    ) -> Hover {
        let response = Service::call(
            service,
            Request::build("textDocument/hover")
                .id(id)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        serde_json::from_value(response.result().cloned().expect("hover result"))
            .expect("decode hover")
    }

    fn hover_language_value(hover: Hover) -> String {
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                language_string.value
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    async fn request_completion_items(
        service: &mut tower_lsp::LspService<TydaLsp>,
        uri: &Url,
        line: u32,
        character: u32,
        id: i64,
    ) -> Vec<CompletionItem> {
        let response = Service::call(
            service,
            Request::build("textDocument/completion")
                .id(id)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character }
                }))
                .finish(),
        )
        .await
        .expect("completion request")
        .expect("completion response");
        let response: CompletionResponse =
            serde_json::from_value(response.result().cloned().expect("completion result"))
                .expect("decode completion");
        match response {
            CompletionResponse::Array(items) => items,
            CompletionResponse::List(list) => list.items,
        }
    }

    fn source_with_cursor(marked_source: &str) -> (String, Position) {
        let marker = "<caret>";
        let marker_offset = marked_source.find(marker).expect("cursor marker");
        let source = marked_source.replace(marker, "");
        let position = byte_offset_to_lsp_position(&source, marker_offset);
        (source, position)
    }

    fn completion_detail<'a>(items: &'a [CompletionItem], label: &str) -> &'a str {
        items
            .iter()
            .find(|item| item.label == label)
            .and_then(|item| item.detail.as_deref())
            .unwrap_or_else(|| panic!("missing completion item {label}; got {items:?}"))
    }

    fn completion_item<'a>(items: &'a [CompletionItem], label: &str) -> &'a CompletionItem {
        items
            .iter()
            .find(|item| item.label == label)
            .unwrap_or_else(|| panic!("missing completion item {label}; got {items:?}"))
    }

    async fn change_document(
        service: &mut tower_lsp::LspService<TydaLsp>,
        socket: &mut ClientSocket,
        uri: &Url,
        version: i32,
        text: &str,
    ) -> Vec<Request> {
        change_document_raw(
            service,
            socket,
            uri,
            version,
            vec![serde_json::json!({
                "text": text,
            })],
        )
        .await
    }

    async fn change_document_raw(
        service: &mut tower_lsp::LspService<TydaLsp>,
        socket: &mut ClientSocket,
        uri: &Url,
        version: i32,
        content_changes: Vec<serde_json::Value>,
    ) -> Vec<Request> {
        let (_, requests) = drive_request(
            service,
            socket,
            Request::build("textDocument/didChange")
                .params(serde_json::json!({
                    "textDocument": {
                        "uri": uri,
                        "version": version,
                    },
                    "contentChanges": content_changes
                }))
                .finish(),
        )
        .await;
        requests
    }

    async fn close_document(
        service: &mut tower_lsp::LspService<TydaLsp>,
        socket: &mut ClientSocket,
        uri: &Url,
    ) -> Vec<Request> {
        let (_, requests) = drive_request(
            service,
            socket,
            Request::build("textDocument/didClose")
                .params(serde_json::json!({
                    "textDocument": {
                        "uri": uri,
                    }
                }))
                .finish(),
        )
        .await;
        requests
    }

    fn assert_has_code_lens_refresh(requests: &[Request]) {
        assert!(
            requests
                .iter()
                .any(|request| request.method() == "workspace/codeLens/refresh"),
            "missing workspace/codeLens/refresh; got {:?}",
            requests
                .iter()
                .map(|request| request.method())
                .collect::<Vec<_>>()
        );
    }

    fn diagnostics_notifications(requests: &[Request], uri: &Url) -> Vec<PublishDiagnosticsParams> {
        requests
            .iter()
            .filter(|request| request.method() == "textDocument/publishDiagnostics")
            .filter_map(|request| {
                let params = request.params()?;
                serde_json::from_value::<PublishDiagnosticsParams>(
                    serde_json::to_value(params).expect("diagnostic params json"),
                )
                .ok()
            })
            .filter(|params| &params.uri == uri)
            .collect()
    }

    #[test]
    fn output_parameter_names_defaults_to_false() {
        assert!(!output_parameter_names_from_initialize_options(None));
        assert!(!output_parameter_names_from_initialize_options(Some(
            &serde_json::json!({})
        )));
    }

    #[test]
    fn output_parameter_names_reads_top_level_flag() {
        assert!(output_parameter_names_from_initialize_options(Some(
            &serde_json::json!({ "output_parameter_names": true })
        )));
    }

    #[test]
    fn output_parameter_names_reads_nested_typeprof_flag() {
        assert!(output_parameter_names_from_initialize_options(Some(
            &serde_json::json!({ "typeprof": { "output_parameter_names": true } })
        )));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn initialize_advertises_incremental_text_sync() {
        let (mut service, _socket) = tower_lsp::LspService::new(TydaLsp::new);
        let response = Service::call(
            &mut service,
            Request::build("initialize")
                .id(97)
                .params(serde_json::json!({
                    "capabilities": {},
                    "rootUri": serde_json::Value::Null,
                }))
                .finish(),
        )
        .await
        .expect("initialize request")
        .expect("initialize response");
        let result: InitializeResult =
            serde_json::from_value(response.result().cloned().expect("initialize result"))
                .expect("decode initialize result");

        assert_eq!(
            result.capabilities.text_document_sync,
            Some(TextDocumentSyncCapability::Options(
                TextDocumentSyncOptions {
                    open_close: Some(true),
                    change: Some(TextDocumentSyncKind::INCREMENTAL),
                    ..Default::default()
                }
            ))
        );
        let completion = result
            .capabilities
            .completion_provider
            .expect("completion provider");
        assert_eq!(completion.resolve_provider, Some(false));
        assert_eq!(
            completion.trigger_characters,
            Some(vec![".".to_string(), ":".to_string()])
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_completion_after_dot_uses_inferred_receiver_type() {
        let (mut service, mut socket) = initialize_lsp(None).await;
        let uri = Url::parse("file:///completion_basic.rb").unwrap();
        let (source, position) = source_with_cursor(
            r#"class Foo
  def foo(n)
    1
  end

  def bar(n)
    "str"
  end

  #: (Foo) -> Foo
  def baz(_)
    _
  end
end

def test1(x)
  x.<caret>
end

Foo.new.foo(1.0)
test1(Foo.new)
"#,
        );
        open_document(&mut service, &mut socket, &uri, &source).await;

        let items =
            request_completion_items(&mut service, &uri, position.line, position.character, 201)
                .await;

        let foo = completion_detail(&items, "foo");
        assert!(foo.starts_with("Foo#foo : (Float"), "{foo}");
        let bar = completion_detail(&items, "bar");
        assert!(bar.starts_with("Foo#bar : (untyped"), "{bar}");
        assert_eq!(completion_detail(&items, "baz"), "Foo#baz : (Foo) -> Foo");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_completion_after_dot_masks_incomplete_selector_prefix() {
        let (mut service, mut socket) = initialize_lsp(None).await;
        let uri = Url::parse("file:///completion_prefix.rb").unwrap();
        let (source, position) = source_with_cursor(
            r#"class Foo
  def format_value = :ok
end

x = Foo.new
x.fo<caret>
"#,
        );
        open_document(&mut service, &mut socket, &uri, &source).await;

        let items =
            request_completion_items(&mut service, &uri, position.line, position.character, 207)
                .await;

        let item = completion_item(&items, "format_value");
        assert_eq!(item.detail.as_deref(), Some("Foo#format_value : -> :ok"));
        assert_eq!(
            item.text_edit,
            Some(CompletionTextEdit::Edit(TextEdit {
                range: Range::new(Position::new(5, 2), Position::new(5, 4)),
                new_text: "format_value".to_string(),
            }))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_completion_after_dot_includes_mixin_and_superclass_methods() {
        let (mut service, mut socket) = initialize_lsp(None).await;
        let uri = Url::parse("file:///completion_module.rb").unwrap();
        let (source, position) = source_with_cursor(
            r#"module M
  def bar = :BAR
end

class P
  def baz = :BAZ
end

class C < P
  include M
  def foo = :FOO
end

x = C.new
x.<caret>
"#,
        );
        open_document(&mut service, &mut socket, &uri, &source).await;

        let items =
            request_completion_items(&mut service, &uri, position.line, position.character, 202)
                .await;

        assert_eq!(completion_detail(&items, "foo"), "C#foo : -> :FOO");
        assert_eq!(completion_detail(&items, "bar"), "M#bar : -> :BAR");
        assert_eq!(completion_detail(&items, "baz"), "P#baz : -> :BAZ");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_completion_inside_send_symbol_uses_explicit_receiver_type() {
        let (mut service, mut socket) = initialize_lsp(None).await;
        let uri = Url::parse("file:///completion_send_receiver.rb").unwrap();
        let (source, position) = source_with_cursor(
            r#"class Widget
  def missing = 1
end

Widget.new.send(:mi<caret>
"#,
        );
        open_document(&mut service, &mut socket, &uri, &source).await;

        let items =
            request_completion_items(&mut service, &uri, position.line, position.character, 208)
                .await;

        let item = completion_item(&items, "missing");
        assert_eq!(item.detail.as_deref(), Some("Widget#missing : -> 1"));
        assert_eq!(
            item.text_edit,
            Some(CompletionTextEdit::Edit(TextEdit {
                range: Range::new(Position::new(4, 17), Position::new(4, 19)),
                new_text: "missing".to_string(),
            }))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_completion_inside_bare_send_symbol_uses_current_self_type() {
        let (mut service, mut socket) = initialize_lsp(None).await;
        let uri = Url::parse("file:///completion_send_bare.rb").unwrap();
        let (source, position) = source_with_cursor(
            r#"class Widget
  def missing = 1

  def call
    send(:mi<caret>
  end
end
"#,
        );
        open_document(&mut service, &mut socket, &uri, &source).await;

        let items =
            request_completion_items(&mut service, &uri, position.line, position.character, 209)
                .await;

        let item = completion_item(&items, "missing");
        assert_eq!(item.detail.as_deref(), Some("Widget#missing : -> 1"));
        assert_eq!(
            item.text_edit,
            Some(CompletionTextEdit::Edit(TextEdit {
                range: Range::new(Position::new(4, 10), Position::new(4, 12)),
                new_text: "missing".to_string(),
            }))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_completion_after_double_colon_returns_nested_constants() {
        let (mut service, mut socket) = initialize_lsp(None).await;
        let uri = Url::parse("file:///completion_constants.rb").unwrap();
        let (source, position) = source_with_cursor(
            r#"module Outer
  VALUE = 1

  class Inner
  end

  module Helper
  end
end

Outer::I<caret>
"#,
        );
        open_document(&mut service, &mut socket, &uri, &source).await;

        let items =
            request_completion_items(&mut service, &uri, position.line, position.character, 203)
                .await;

        let inner = completion_item(&items, "Inner");
        assert_eq!(inner.kind, Some(CompletionItemKind::CLASS));
        assert_eq!(inner.detail.as_deref(), Some("class Outer::Inner"));
        assert_eq!(
            inner.text_edit,
            Some(CompletionTextEdit::Edit(TextEdit {
                range: Range::new(Position::new(10, 7), Position::new(10, 8)),
                new_text: "Inner".to_string(),
            }))
        );
        assert!(
            !items.iter().any(|item| item.label == "Helper"),
            "{items:?}"
        );

        let (source, position) = source_with_cursor(&source.replace("Outer::I", "Outer::<caret>"));
        open_document(&mut service, &mut socket, &uri, &source).await;
        let items =
            request_completion_items(&mut service, &uri, position.line, position.character, 204)
                .await;

        assert_eq!(
            completion_item(&items, "Helper").kind,
            Some(CompletionItemKind::MODULE)
        );
        assert_eq!(completion_detail(&items, "VALUE"), "Outer::VALUE : 1");

        let (source, position) = source_with_cursor(
            r#"module Outer
  class Inner
  end
end

Alias = Outer
Alias::I<caret>
"#,
        );
        open_document(&mut service, &mut socket, &uri, &source).await;
        let items =
            request_completion_items(&mut service, &uri, position.line, position.character, 205)
                .await;
        assert_eq!(completion_detail(&items, "Inner"), "class Outer::Inner");

        let (source, position) = source_with_cursor(
            r#"module Outer
  module Inner
    VALUE = 1
  end

  class C
    Inner::V<caret>
  end
end
"#,
        );
        open_document(&mut service, &mut socket, &uri, &source).await;
        let items =
            request_completion_items(&mut service, &uri, position.line, position.character, 206)
                .await;
        assert_eq!(
            completion_detail(&items, "VALUE"),
            "Outer::Inner::VALUE : 1"
        );
    }

    #[test]
    fn completion_external_registry_skips_empty_user_rbs_for_self_contained_file() {
        let mut state = new_test_state();
        let source = "module Local\n  def self.value = 1\nend\n";
        insert_analyzed_test_file(&mut state, "local.rb", source);

        assert!(
            state
                .workspace_state
                .display_can_skip_workspace_context("local.rb")
        );
        assert!(
            TydaLsp::external_registry_for_completion(&mut state, "local.rb", source).is_none()
        );
        assert!(state.cached_display_registry.is_empty());
    }

    #[test]
    fn completion_external_registry_keeps_user_rbs_without_workspace_build() {
        let mut state = new_test_state();
        let mut user_rbs = TypeRegistry::new();
        crate::rbs::import::load_rbs_string(
            r#"
class Object
  def project_helper: () -> String
end
"#,
            &mut user_rbs,
        );
        state.user_rbs = Arc::new(user_rbs);
        let user_rbs = Arc::clone(&state.user_rbs);
        let source = "module Local\n  def self.value = 1\nend\n";
        insert_analyzed_test_file(&mut state, "local.rb", source);

        assert!(
            state
                .workspace_state
                .display_can_skip_workspace_context("local.rb")
        );
        let external = TydaLsp::external_registry_for_completion(&mut state, "local.rb", source)
            .expect("user RBS");

        assert!(Arc::ptr_eq(&external, &user_rbs));
        assert!(state.cached_display_registry.is_empty());
    }

    #[test]
    fn completion_external_registry_keeps_workspace_when_current_cache_is_stale() {
        let mut state = new_test_state();
        let old_source = "module Local\n  def self.value = 1\nend\n";
        insert_analyzed_test_file(&mut state, "local.rb", old_source);

        assert!(
            state
                .workspace_state
                .display_can_skip_workspace_context("local.rb")
        );
        let edited_source = "Provider.new.value\n";
        let external =
            TydaLsp::external_registry_for_completion(&mut state, "local.rb", edited_source);

        assert!(external.is_some());
    }

    #[test]
    fn code_lens_title_matches_inserted_signature() {
        assert_eq!(code_lens_title("-> [1, 2]"), "#: -> [1, 2]");
        assert_eq!(code_lens_title("-> [ ]"), "#: -> [ ]");
    }

    #[test]
    fn apply_content_changes_supports_incremental_insert_and_delete() {
        let source = "class A\n  def foo\n    1\n  end\nend\n";
        let inserted = apply_content_changes(
            source,
            &[TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(0, 0), Position::new(0, 0))),
                range_length: None,
                text: "\n".to_string(),
            }],
        )
        .expect("inserted source");
        assert_eq!(inserted, "\nclass A\n  def foo\n    1\n  end\nend\n");

        let deleted = apply_content_changes(
            &inserted,
            &[TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(0, 0), Position::new(1, 0))),
                range_length: None,
                text: String::new(),
            }],
        )
        .expect("deleted source");
        assert_eq!(deleted, source);
    }

    #[test]
    fn code_lens_range_for_method_anchors_to_method_name() {
        let source = "class A\n  def foo(x)\n    x\n  end\nend\n";
        let sig = MethodSig {
            name: "foo".to_string(),
            params: vec![Param {
                name: "x".to_string(),
                param_type: Type::Untyped,
                kind: ParamKind::Required,
            }],
            return_type: Type::Untyped,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: Some(SourceLocation { line: 2, column: 2 }),
            is_private: false,
        };

        let range = code_lens_range_for_method(source, &sig).expect("code lens range");
        assert_eq!(range.start, Position::new(1, 6));
        assert_eq!(range.end, Position::new(1, 9));
    }

    #[test]
    fn collapse_accessor_pair_merges_reader_and_writer_into_optional_arg() {
        let loc = Some(crate::types::SourceLocation { line: 2, column: 2 });
        let getter = MethodSig {
            name: "name".to_string(),
            params: Vec::new(),
            return_type: crate::types::Type::String,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: true,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc,
            is_private: false,
        };
        let setter = MethodSig {
            name: "name=".to_string(),
            params: vec![Param {
                name: "name".to_string(),
                param_type: crate::types::Type::String,
                kind: crate::types::ParamKind::Required,
            }],
            return_type: crate::types::Type::String,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: true,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc,
            is_private: false,
        };

        let combined = collapse_accessor_pair(&getter, &setter).expect("collapsed accessor pair");
        assert_eq!(
            format_method_sig_for_lens_with_names(&combined, false),
            "(?String) -> String"
        );
    }

    #[test]
    fn collapse_accessor_code_lens_methods_deduplicates_accessor_pair() {
        let loc = Some(crate::types::SourceLocation { line: 2, column: 2 });
        let methods = vec![
            (
                "User".to_string(),
                MethodSig {
                    name: "name".to_string(),
                    params: Vec::new(),
                    return_type: crate::types::Type::String,
                    block: None,
                    sorbet_modifier_comments: Vec::new(),
                    is_singleton: false,
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    overloads: Vec::new(),
                    loc,
                    is_private: false,
                },
            ),
            (
                "User".to_string(),
                MethodSig {
                    name: "name=".to_string(),
                    params: vec![Param {
                        name: "name".to_string(),
                        param_type: crate::types::Type::String,
                        kind: crate::types::ParamKind::Required,
                    }],
                    return_type: crate::types::Type::String,
                    block: None,
                    sorbet_modifier_comments: Vec::new(),
                    is_singleton: false,
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    overloads: Vec::new(),
                    loc,
                    is_private: false,
                },
            ),
        ];

        let collapsed = collapse_accessor_code_lens_methods(methods);
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].1.name, "name");
        assert_eq!(
            format_method_sig_for_lens_with_names(&collapsed[0].1, false),
            "(?String) -> String"
        );
    }

    #[test]
    fn annotated_methods_do_not_show_code_lens() {
        let sig = MethodSig {
            name: "foo".to_string(),
            params: Vec::new(),
            return_type: crate::types::Type::Integer,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: true,
            rbs_inline_annotated: true,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            sig_annotated: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        };
        assert!(!should_show_code_lens(&sig));
    }

    #[test]
    fn sig_annotated_methods_do_not_show_code_lens() {
        let source = concat!(
            "class Sample\n",
            "  extend T::Sig\n",
            "  sig { returns(Integer) }\n",
            "  def foo\n",
            "    1\n",
            "  end\n",
            "end\n",
        );
        let loader = stdlib_loader();
        let registry =
            crate::parser::analyze_source_with_file_path(source, None, &loader, "app/sample.rb");
        let methods = registry.methods_for_file("app/sample.rb");
        let (_class_name, sig) = methods
            .iter()
            .find(|(_class_name, sig)| sig.name == "foo")
            .expect("sig-annotated method");

        assert!(!sig.rbs_annotated);
        assert!(!sig.rbs_inline_annotated);
        assert!(sig.sig_annotated);
        assert!(!should_show_code_lens(sig));
    }

    #[test]
    fn inserted_rbs_comment_hides_code_lens_after_reanalysis() {
        let source = "class Sample\n  def foo\n    1\n  end\nend\n";
        let updated = insert_signature_comment(source, 1, "-> Integer").expect("updated");
        let loader = stdlib_loader();
        let registry =
            crate::parser::analyze_source_with_file_path(&updated, None, &loader, "app/sample.rb");
        let methods = registry.methods_for_file("app/sample.rb");
        let (_class_name, sig) = methods
            .iter()
            .find(|(_class_name, sig)| sig.name == "foo")
            .expect("inline-annotated method");

        assert!(sig.rbs_inline_annotated);
        assert!(!should_show_code_lens(sig));
    }

    #[test]
    fn cached_analysis_reanalysis_hides_code_lens() {
        let source = "class Sample\n  def foo\n    1\n  end\nend\n";
        let updated = insert_signature_comment(source, 1, "-> Integer").expect("updated");
        let loader = stdlib_loader();
        let analysis = crate::analysis::analyze_cached_file_with_deps(
            &updated,
            None,
            Some(&loader),
            Some("app/sample.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let methods = analysis.methods_for_file("app/sample.rb");
        let (_class_name, sig) = methods
            .iter()
            .find(|(_class_name, sig)| sig.name == "foo")
            .expect("inline-annotated method");

        assert!(sig.rbs_inline_annotated);
        assert!(!should_show_code_lens(sig));
    }

    #[test]
    fn external_rbs_method_still_shows_code_lens() {
        let source = concat!(
            "class User\n",
            "  def name\n",
            "    \"Alice\"\n",
            "  end\n",
            "end\n",
        );
        let mut rbs_registry = crate::registry::TypeRegistry::new();
        crate::rbs::import::load_rbs_string(
            "class User\n  def name: -> String\nend\n",
            &mut rbs_registry,
        );

        let loader = stdlib_loader();
        let registry = crate::parser::analyze_source_with_file_path(
            source,
            Some(&rbs_registry),
            &loader,
            "app/user.rb",
        );
        let methods = registry.methods_for_file("app/user.rb");
        let (_class_name, sig) = methods
            .iter()
            .find(|(_class_name, sig)| sig.name == "name")
            .expect("user-defined method backed by external rbs");

        assert!(!sig.rbs_file_source);
        assert!(!sig.rbs_inline_annotated);
        assert!(!sig.sig_annotated);
        assert!(should_show_code_lens(sig));
    }

    #[test]
    fn insert_signature_comment_preserves_indent() {
        let source = "class Sample\n  def foo\n    1\n  end\nend\n";
        let updated = insert_signature_comment(source, 1, "-> Integer").expect("updated");
        assert_eq!(
            updated,
            "class Sample\n  #: -> Integer\n  def foo\n    1\n  end\nend\n"
        );
    }

    #[test]
    fn attr_reader_line_does_not_support_signature_comment() {
        let source = "class Sample\n  attr_reader :name\nend\n";
        assert!(!source_line_supports_signature_comment(source, 1));
    }

    #[test]
    fn def_line_supports_signature_comment() {
        let source = "class Sample\n  def foo\n    1\n  end\nend\n";
        assert!(source_line_supports_signature_comment(source, 1));
    }

    #[test]
    fn wrapped_def_line_supports_signature_comment() {
        let source = "class Sample\n  memoize def foo\n    1\n  end\nend\n";
        assert!(source_line_supports_signature_comment(source, 1));
    }

    #[test]
    fn fallback_code_lens_methods_include_unannotated_def_lines() {
        let source = "class Sample\n  def foo(x)\n";
        let methods =
            fallback_code_lens_methods_from_source(source, &std::collections::HashSet::new());
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].1.name, "foo");
        assert_eq!(
            methods[0].1.loc,
            Some(SourceLocation { line: 2, column: 6 })
        );
    }

    #[test]
    fn fallback_code_lens_methods_include_wrapped_def_lines() {
        let source = "class Sample\n  public def foo(x)\n";
        let methods =
            fallback_code_lens_methods_from_source(source, &std::collections::HashSet::new());
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].1.name, "foo");
        assert_eq!(
            methods[0].1.loc,
            Some(SourceLocation {
                line: 2,
                column: 13
            })
        );
    }

    #[test]
    fn fallback_code_lens_methods_skip_rbs_annotated_def_lines() {
        let source = "class Sample\n  #: -> Integer\n  def foo\n";
        let methods =
            fallback_code_lens_methods_from_source(source, &std::collections::HashSet::new());
        assert!(methods.is_empty());
    }

    #[test]
    fn fallback_code_lens_methods_skip_atrbs_annotated_def_lines() {
        let source = "class Sample\n  # @rbs value: String\n  def foo(value)\n";
        let methods =
            fallback_code_lens_methods_from_source(source, &std::collections::HashSet::new());
        assert!(methods.is_empty());
    }

    #[test]
    fn fallback_code_lens_methods_skip_trailing_rbs_return_annotation() {
        let source = "class Sample\n  def foo #: String\n";
        let methods =
            fallback_code_lens_methods_from_source(source, &std::collections::HashSet::new());
        assert!(methods.is_empty());
    }

    #[test]
    fn hover_contents_prefers_callable_signature() {
        let contents = format_hover_body(&crate::analysis::HoverResult {
            name: "each".to_string(),
            ty: crate::types::Type::Array(Some(Box::new(crate::types::Type::Integer))),
            display_rbs: Some(
                "each: () -> Enumerator[Integer, Array[Integer]]\n    | () { (Integer item) -> void } -> Array[Integer]"
                    .to_string(),
            ),
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        });
        assert_eq!(
            contents,
            "[Tyda] () -> Enumerator[Integer, Array[Integer]]\n    | () { (Integer item) -> void } -> Array[Integer]"
        );
    }

    #[test]
    fn hover_contents_formats_method_definition_as_type() {
        let contents = format_hover_body(&crate::analysis::HoverResult {
            name: "tag_is_usable".to_string(),
            ty: crate::types::Type::Nil,
            display_rbs: Some("-> nil".to_string()),
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        });
        assert_eq!(contents, "[Tyda] -> nil");
    }

    #[test]
    fn hover_contents_falls_back_to_name_and_type() {
        let contents = format_hover_body(&crate::analysis::HoverResult {
            name: "value".to_string(),
            ty: crate::types::Type::LiteralInteger(1),
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        });
        assert_eq!(contents, "[Tyda] 1");
    }

    #[test]
    fn hover_contents_compresses_redundant_constant_name_and_type() {
        let contents = format_hover_body(&crate::analysis::HoverResult {
            name: "A::B".to_string(),
            ty: crate::types::Type::Class(Sym::new("A::B")),
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        });
        assert_eq!(contents, "[Tyda] A::B");
    }

    #[test]
    fn hover_contents_use_rbs_language_string() {
        let contents = format_hover_contents(&crate::analysis::HoverResult {
            name: "value".to_string(),
            ty: crate::types::Type::Integer,
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        });
        match contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(language_string.value, "[Tyda] Integer");
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[test]
    fn hover_contents_append_resolved_type_params() {
        let contents = format_hover_body(&crate::analysis::HoverResult {
            name: "each".to_string(),
            ty: crate::types::Type::Array(Some(Box::new(crate::types::Type::Integer))),
            display_rbs: Some(
                "each: () -> Enumerator[Integer, Array[Integer]]\n    | () { (Integer item) -> void } -> Array[Integer]"
                    .to_string(),
            ),
            type_params: vec![("Elem".to_string(), crate::types::Type::Integer)],
            can_enrich_from_workspace: false,
            unresolved_method: None,
        });
        assert_eq!(
            contents,
            "[Tyda] () -> Enumerator[Integer, Array[Integer]]\n    | () { (Integer item) -> void } -> Array[Integer]\n# type params: Elem = Integer"
        );
    }

    #[test]
    fn hover_contents_renders_todo_as_untyped() {
        let contents = format_hover_body(&crate::analysis::HoverResult {
            name: "value".to_string(),
            ty: crate::types::Type::Todo,
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        });
        assert_eq!(contents, "[Tyda] untyped");
    }

    #[test]
    fn hover_contents_renders_nested_todo_as_untyped() {
        let contents = format_hover_body(&crate::analysis::HoverResult {
            name: "args".to_string(),
            ty: crate::types::Type::Array(Some(Box::new(crate::types::Type::Todo))),
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        });
        assert_eq!(contents, "[Tyda] Array[untyped]");
    }

    #[test]
    fn hover_without_type_holes_does_not_need_workspace_fallback() {
        let hover = crate::analysis::HoverResult {
            name: "each".to_string(),
            ty: crate::types::Type::Array(Some(Box::new(crate::types::Type::Integer))),
            display_rbs: Some(
                "each: () -> Enumerator[Integer, Array[Integer]]\n    | () { (Integer item) -> void } -> Array[Integer]"
                    .to_string(),
            ),
            type_params: vec![("Elem".to_string(), crate::types::Type::Integer)],
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };
        assert!(!hover_needs_workspace_fallback(&hover));
    }

    #[test]
    fn hover_with_todo_still_needs_workspace_fallback() {
        let hover = crate::analysis::HoverResult {
            name: "value".to_string(),
            ty: crate::types::Type::Todo,
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };
        assert!(hover_needs_workspace_fallback(&hover));
    }

    #[test]
    fn hover_at_returns_callable_signature_for_bare_method_receiver_chain() {
        let source = concat!(
            "def foo\n",
            "  [1, 2]\n",
            "end\n",
            "\n",
            "foo.each do |x|\n",
            "  x\n",
            "end\n",
        );
        let loader = stdlib_loader();
        let hover = crate::analysis::hover_at(source, None, &loader, "app/sample.rb", 5, 4)
            .expect("hover for each");

        assert_eq!(hover.name, "each");
        let display_rbs = hover.display_rbs.expect("callable signature");
        assert!(display_rbs.contains("each:"));
        assert!(display_rbs.contains("Integer element"));
    }

    #[test]
    fn target_ruby_path_defaults_to_all_ruby_files() {
        assert!(is_target_ruby_path(None, "/tmp/sample.rb"));
        assert!(!is_target_ruby_path(None, "/tmp/sample.rbs"));
    }

    #[test]
    fn target_ruby_path_respects_analysis_unit_roots() {
        let roots = vec![PathBuf::from("/workspace/app")];
        assert!(is_target_ruby_path(Some(&roots), "/workspace/app/foo.rb"));
        assert!(!is_target_ruby_path(
            Some(&roots),
            "/workspace/other/user.rb"
        ));
    }

    #[test]
    fn strip_jsonc_comments_preserves_strings() {
        let input = r#"{
  // comment
  "analysis_unit_dirs": ["app"],
  "url": "https://example.test//keep"
}"#;
        let stripped = strip_jsonc_comments(input);
        assert!(stripped.contains("\"analysis_unit_dirs\": [\"app\"]"));
        assert!(stripped.contains("\"https://example.test//keep\""));
        assert!(!stripped.contains("// comment"));
    }

    #[test]
    fn scan_roots_reads_analysis_unit_dirs_from_typeprof_config() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("app")).expect("app dir");
        std::fs::create_dir_all(root.join("other")).expect("other dir");
        std::fs::write(
            root.join("typeprof.conf.jsonc"),
            r#"{
  // comment
  "analysis_unit_dirs": ["app"]
}"#,
        )
        .expect("write config");

        let roots = scan_roots_from_typeprof_config(root).expect("scan roots");
        assert_eq!(roots, vec![root.join("app")]);
    }

    #[test]
    fn load_typeprof_config_reads_dsl_tokens() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(
            root.join("typeprof.conf.jsonc"),
            r#"{
  "dsl": ["+aasm", "-protobuf"]
}"#,
        )
        .expect("write config");

        let config = load_typeprof_config(root)
            .expect("load config")
            .expect("config present");
        assert_eq!(
            config.dsl,
            Some(vec!["+aasm".to_string(), "-protobuf".to_string()])
        );
    }

    #[test]
    fn collect_rb_files_from_roots_limits_scan_scope() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let sample = root.join("app");
        let other = root.join("other");
        std::fs::create_dir_all(&sample).expect("sample dir");
        std::fs::create_dir_all(&other).expect("other dir");
        std::fs::write(sample.join("sample.rb"), "class Sample; end\n").expect("sample rb");
        std::fs::write(other.join("other.rb"), "class Other; end\n").expect("other rb");

        let files = collect_rb_files_from_roots(&[sample]);
        assert_eq!(files, vec![root.join("app/sample.rb")]);
    }

    #[test]
    fn collect_workspace_scan_files_skips_known_files_on_incremental_rescan() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let app = root.join("app");
        let extra = root.join("extra");
        let root_path = root.to_path_buf();
        std::fs::create_dir_all(&app).expect("app dir");
        std::fs::create_dir_all(&extra).expect("extra dir");
        let known = app.join("known.rb");
        let pending = app.join("pending.rb");
        let open_only = app.join("open_only.rb");
        std::fs::write(&known, "class Known; end\n").expect("known rb");
        std::fs::write(&pending, "class Pending; end\n").expect("pending rb");
        std::fs::write(&open_only, "class OpenOnly; end\n").expect("open rb");
        std::fs::write(extra.join("ignored.rb"), "class Ignored; end\n").expect("ignored rb");

        // With a delta present, never re-stat every known file (the file watcher would otherwise burn CPU constantly on large workspaces).
        let files = collect_workspace_scan_files(
            std::slice::from_ref(&root_path),
            std::slice::from_ref(&known),
            std::slice::from_ref(&pending),
            &HashMap::from([(
                open_only.to_string_lossy().to_string(),
                "class OpenOnly; end\n".to_string(),
            )]),
            Some(std::slice::from_ref(&app)),
            false,
        );

        assert_eq!(files, vec![open_only, pending]);
    }

    /// The cold scan still covers the whole root even after a didOpen-first upsert (degrading to just the open set would hide unopened definitions).
    #[test]
    fn workspace_scan_walks_full_roots_when_state_has_preloaded_files() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let alpha = root.join("alpha.rb");
        let beta = root.join("beta.rb");
        std::fs::write(&alpha, "class Alpha\nend\n").expect("alpha rb");
        std::fs::write(&beta, "class Beta\nend\n").expect("beta rb");

        let loader = stdlib_loader();
        let alpha_path = alpha.to_string_lossy().to_string();
        let alpha_analysis = crate::analysis::analyze_cached_file_with_deps(
            "class Alpha\nend\n",
            None,
            Some(&loader),
            Some(&alpha_path),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = new_test_state();
        state.workspace_root = Some(root.to_path_buf());
        insert_test_cache(&mut state, &alpha_path, alpha_analysis);

        let state = Arc::new(Mutex::new(state));
        TydaLsp::scan_workspace_for_deps_shared(&state, 0);

        let state = state.lock().unwrap();
        let beta_path = beta.to_string_lossy().to_string();
        assert!(
            state.workspace_state.workspace_file(&beta_path).is_some(),
            "cold scan must cover files that were never opened"
        );
        assert!(state.workspace_fully_discovered);
    }

    #[test]
    fn should_commit_workspace_scan_matches_current_generation_only() {
        let mut state = new_test_state();
        state.workspace_scan_generation = 5;
        assert!(TydaLsp::should_commit_workspace_scan(&state, 5));
        assert!(!TydaLsp::should_commit_workspace_scan(&state, 4));
    }

    // Reproduces the race: a scan captures generation G, then a reload (or a
    // watched-file batch) resets/bumps the generation before the scan commits.
    // The stale scan must not mark `workspace_scanned` or publish diagnostics
    // computed against the environment that was just replaced.
    #[test]
    fn scan_workspace_for_deps_shared_discards_result_when_generation_is_stale() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("alpha.rb"), "class Alpha\nend\n").expect("alpha rb");

        let mut state = new_test_state();
        state.workspace_root = Some(root.to_path_buf());
        // Simulate a reload that happened after this scan started: the state's
        // generation (3) has moved past the one the scan thread captured (1).
        state.workspace_scan_generation = 3;
        state
            .workspace_state
            .mark_file_pending_scan("stale-pending.rb".to_string());
        let state = Arc::new(Mutex::new(state));

        TydaLsp::scan_workspace_for_deps_shared(&state, 1);

        let state = state.lock().unwrap();
        assert!(
            state
                .workspace_state
                .workspace_file(&root.join("alpha.rb").to_string_lossy())
                .is_none(),
            "a stale scan must not walk the workspace at all"
        );
        assert!(
            !state.workspace_fully_discovered,
            "a stale scan must not claim full discovery finished"
        );
        assert_eq!(
            state.workspace_state.pending_scan_files().count(),
            1,
            "a stale scan must not clear pending marks it never processed"
        );
    }

    #[test]
    fn scan_workspace_for_deps_shared_commits_when_generation_is_current() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("alpha.rb"), "class Alpha\nend\n").expect("alpha rb");

        let mut state = new_test_state();
        state.workspace_root = Some(root.to_path_buf());
        state.workspace_scan_generation = 7;
        let state = Arc::new(Mutex::new(state));

        TydaLsp::scan_workspace_for_deps_shared(&state, 7);

        let state = state.lock().unwrap();
        assert!(
            state
                .workspace_state
                .workspace_file(&root.join("alpha.rb").to_string_lossy())
                .is_some(),
            "a current-generation scan must walk and record the workspace"
        );
        assert!(state.workspace_fully_discovered);
    }

    #[test]
    fn collect_workspace_scan_files_force_full_ignores_known_files() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let app = root.join("app");
        let root_path = root.to_path_buf();
        std::fs::create_dir_all(&app).expect("app dir");
        let known = app.join("known.rb");
        let other = app.join("other.rb");
        std::fs::write(&known, "class Known; end\n").expect("known rb");
        std::fs::write(&other, "class Other; end\n").expect("other rb");

        // With `force_full`, scan the whole root even if there are open docs (degrading would miss unopened definitions).
        let files = collect_workspace_scan_files(
            std::slice::from_ref(&root_path),
            std::slice::from_ref(&known),
            &[],
            &HashMap::from([(
                known.to_string_lossy().to_string(),
                "class Known; end\n".to_string(),
            )]),
            None,
            true,
        );

        assert_eq!(files, vec![known, other]);
    }

    #[test]
    fn collect_workspace_scan_files_falls_back_to_known_when_delta_empty() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let app = root.join("app");
        let root_path = root.to_path_buf();
        std::fs::create_dir_all(&app).expect("app dir");
        let known = app.join("known.rb");
        std::fs::write(&known, "class Known; end\n").expect("known rb");

        // With no pending/open files, do a safety re-stat of the known set (a hedge against a missed watcher event).
        let files = collect_workspace_scan_files(
            std::slice::from_ref(&root_path),
            std::slice::from_ref(&known),
            &[],
            &HashMap::new(),
            Some(std::slice::from_ref(&app)),
            false,
        );

        assert_eq!(files, vec![known]);
    }

    #[test]
    fn select_bench_target_file_prefers_model_like_paths() {
        let files = vec![
            PathBuf::from("/tmp/project/app/controllers/projects_controller.rb"),
            PathBuf::from("/tmp/project/app/models/project.rb"),
            PathBuf::from("/tmp/project/lib/project_helper.rb"),
        ];

        let selected = choose_scan_benchmark_target(&files).expect("selected target");
        assert_eq!(
            selected,
            PathBuf::from("/tmp/project/app/models/project.rb")
        );
    }

    #[test]
    fn select_bench_target_file_prefers_account_model_when_present() {
        let files = vec![
            PathBuf::from("/tmp/project/app/models/user.rb"),
            PathBuf::from("/tmp/project/app/models/account.rb"),
            PathBuf::from("/tmp/project/app/models/project.rb"),
        ];

        let selected = choose_scan_benchmark_target(&files).expect("selected target");
        assert_eq!(
            selected,
            PathBuf::from("/tmp/project/app/models/account.rb")
        );
    }

    #[test]
    fn hover_workspace_registry_merges_cached_ruby_methods() {
        let loader = stdlib_loader();
        let tag_source = concat!(
            "class A\n",
            "  class << self\n",
            "    def foo(x)\n",
            "      x\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        let tag_analysis = crate::analysis::analyze_cached_file_with_deps(
            tag_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "a.rb", tag_analysis);

        let workspace_registry = build_hover_workspace_registry(&mut state, "collection.rb");
        let collection_source = "A.foo(\"x\")\n";
        let hover = crate::analysis::hover_at(
            collection_source,
            Some(&workspace_registry),
            &loader,
            "collection.rb",
            1,
            4,
        )
        .expect("cross-file hover");

        // The return value resolves through the param marker to the call site's String
        // (keeps the displayed param and return type consistent).
        assert_eq!(
            hover.display_rbs.as_deref(),
            Some("foo: (String x) -> String")
        );
    }

    #[test]
    fn hover_inherited_predicate_resolves_to_bool() {
        let loader = stdlib_loader();
        let base_source = concat!(
            "class Base\n",
            "  def initialize(detail)\n",
            "    @detail = detail\n",
            "  end\n",
            "\n",
            "  def has_detail?\n",
            "    !@detail.nil?\n",
            "  end\n",
            "end\n",
        );
        let base_analysis = crate::analysis::analyze_cached_file_with_deps(
            base_source,
            None,
            Some(&loader),
            Some("base.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "base.rb", base_analysis);

        let child_source = concat!(
            "class Child < Base\n",
            "  def items\n",
            "    has_detail? ? 1 : 2\n",
            "  end\n",
            "end\n",
        );
        let workspace_registry = build_hover_workspace_registry(&mut state, "child.rb");
        let hover = crate::analysis::hover_at(
            child_source,
            Some(&workspace_registry),
            &loader,
            "child.rb",
            3,
            4,
        )
        .expect("inherited predicate hover");

        assert_eq!(hover.name, "has_detail?");
        assert_eq!(hover.ty.to_string(), "bool");
    }

    #[test]
    fn hover_updates_when_dependency_changes() {
        let loader = stdlib_loader();
        let parent_v1 = concat!("class Parent\n", "  def value = 1\n", "end\n",);
        let parent_v2 = concat!("class Parent\n", "  def value = \"hello\"\n", "end\n",);
        let child_source = concat!(
            "class Child < Parent\n",
            "  def result\n",
            "    value\n",
            "  end\n",
            "end\n",
        );

        let parent_analysis_v1 = crate::analysis::analyze_cached_file_with_deps(
            parent_v1,
            None,
            Some(&loader),
            Some("parent.rb"),
            AnalysisOptions::default(),
        );
        let mut state = new_test_state();
        state.workspace_state.upsert_file(
            "parent.rb".into(),
            crate::workspace_state::hash_content(parent_v1),
            parent_analysis_v1.0,
            parent_analysis_v1.1,
        );

        let workspace_reg = build_hover_workspace_registry(&mut state, "child.rb");
        let hover1 = crate::analysis::hover_at(
            child_source,
            Some(&workspace_reg),
            &loader,
            "child.rb",
            3,
            4,
        )
        .expect("hover before update");
        assert_eq!(hover1.name, "value");
        assert_eq!(hover1.ty.to_string(), "1");

        let parent_analysis_v2 = crate::analysis::analyze_cached_file_with_deps(
            parent_v2,
            None,
            Some(&loader),
            Some("parent.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "parent.rb".into(),
            crate::workspace_state::hash_content(parent_v2),
            parent_analysis_v2.0,
            parent_analysis_v2.1,
        );

        let workspace_reg2 = build_hover_workspace_registry(&mut state, "child.rb");
        let hover2 = crate::analysis::hover_at(
            child_source,
            Some(&workspace_reg2),
            &loader,
            "child.rb",
            3,
            4,
        )
        .expect("hover after update");
        assert_eq!(hover2.name, "value");
        assert_eq!(hover2.ty.to_string(), "\"hello\"");
    }

    #[test]
    fn hover_cross_file_method_return_resolved_like_cli() {
        let loader = stdlib_loader();
        let provider_source = concat!(
            "class Provider\n",
            "  def greeting\n",
            "    \"hello\"\n",
            "  end\n",
            "end\n",
        );
        let provider_analysis = crate::analysis::analyze_cached_file_with_deps(
            provider_source,
            None,
            Some(&loader),
            Some("provider.rb"),
            AnalysisOptions::default(),
        );
        let mut state = new_test_state();
        state.workspace_state.upsert_file(
            "provider.rb".into(),
            crate::workspace_state::hash_content(provider_source),
            provider_analysis.0,
            provider_analysis.1,
        );

        let consumer_source = concat!(
            "class Consumer\n",
            "  def run\n",
            "    Provider.new.greeting\n",
            "  end\n",
            "end\n",
        );

        let workspace_reg = build_hover_workspace_registry(&mut state, "consumer.rb");
        let hover = crate::analysis::hover_at(
            consumer_source,
            Some(&workspace_reg),
            &loader,
            "consumer.rb",
            3,
            18,
        )
        .expect("cross-file method return hover");
        assert_eq!(hover.name, "greeting");
        assert!(
            !matches!(
                hover.ty,
                crate::types::Type::Untyped | crate::types::Type::Todo
            ),
            "cross-file method return should be resolved, got {}",
            hover.ty,
        );
    }

    #[test]
    fn hover_workspace_registry_resolves_cross_file_constant() {
        let loader = stdlib_loader();
        let tag_manager_source = concat!(
            "require 'singleton'\n",
            "module A\n",
            "  class B\n",
            "    include Singleton\n",
            "  end\n",
            "end\n",
        );
        let tag_manager_analysis = crate::analysis::analyze_cached_file_with_deps(
            tag_manager_source,
            None,
            Some(&loader),
            Some("a_b.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "a_b.rb", tag_manager_analysis);

        let workspace_registry = build_hover_workspace_registry(&mut state, "collection.rb");
        let source = "A::B.instance\n";
        let parent_hover = crate::analysis::hover_at(
            source,
            Some(&workspace_registry),
            &loader,
            "collection.rb",
            1,
            0,
        )
        .expect("cross-file parent constant hover");
        assert_eq!(parent_hover.name, "A");
        assert_eq!(parent_hover.ty.to_string(), "singleton(A)");

        let hover = crate::analysis::hover_at(
            source,
            Some(&workspace_registry),
            &loader,
            "collection.rb",
            1,
            3,
        )
        .expect("cross-file constant hover");

        assert_eq!(hover.name, "A::B");
        assert_eq!(hover.ty.to_string(), "singleton(A::B)");
        assert!(hover.display_rbs.is_none());
    }

    #[test]
    fn hover_resolves_cross_file_hash_constant_through_include() {
        // rack: a bare constant reached via include hovers as a Hash type (prevents a regression to `singleton(...)`).
        let loader = stdlib_loader();
        let utils_source = concat!(
            "module Rack\n",
            "  module Utils\n",
            "    STATUS_WITH_NO_ENTITY_BODY = { 200 => true }\n",
            "  end\n",
            "end\n",
        );
        let utils_analysis = crate::analysis::analyze_cached_file_with_deps(
            utils_source,
            None,
            Some(&loader),
            Some("rack/utils.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "rack/utils.rb", utils_analysis);

        let workspace_registry = build_hover_workspace_registry(&mut state, "rack/content_type.rb");

        let cases = &[
            (
                "bare via include",
                "module Rack\n  class ContentType\n    include Rack::Utils\n    def call\n      STATUS_WITH_NO_ENTITY_BODY\n    end\n  end\nend\n",
                5usize,
                6usize,
                "STATUS_WITH_NO_ENTITY_BODY",
            ),
            (
                "relative Utils::CONST",
                "module Rack\n  class Deflater\n    def call\n      Utils::STATUS_WITH_NO_ENTITY_BODY\n    end\n  end\nend\n",
                4,
                13,
                "Utils::STATUS_WITH_NO_ENTITY_BODY",
            ),
            (
                "fully qualified Rack::Utils::CONST",
                "class Lint\n  def call\n    Rack::Utils::STATUS_WITH_NO_ENTITY_BODY\n  end\nend\n",
                3,
                17,
                "Rack::Utils::STATUS_WITH_NO_ENTITY_BODY",
            ),
        ];
        for (label, source, line, col, expected_name) in cases {
            let hover = crate::analysis::hover_at(
                source,
                Some(&workspace_registry),
                &loader,
                "rack/content_type.rb",
                *line,
                *col,
            )
            .unwrap_or_else(|| panic!("{label}: hover missing"));
            assert_eq!(hover.name, *expected_name, "{label}: hover name");
            assert_eq!(
                hover.ty.to_string(),
                "Hash[200, true]",
                "{label}: hover type should be the Hash literal, not singleton"
            );
        }
    }

    #[test]
    fn hover_resolves_cross_file_constant_from_namespace() {
        let loader = stdlib_loader();
        let mailer_source = concat!(
            "class UserMailer\n",
            "  def warning(user, w) = \"sent\"\n",
            "end\n",
        );
        let mailer_analysis = crate::analysis::analyze_cached_file_with_deps(
            mailer_source,
            None,
            Some(&loader),
            Some("app/mailers/user_mailer.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "app/mailers/user_mailer.rb", mailer_analysis);

        let workspace_registry =
            build_hover_workspace_registry(&mut state, "app/models/admin/action.rb");
        let source = concat!(
            "module Admin\n",
            "  class Action\n",
            "    def notify\n",
            "      UserMailer.warning(nil, nil)\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        let hover = crate::analysis::hover_at(
            source,
            Some(&workspace_registry),
            &loader,
            "app/models/admin/action.rb",
            4,
            6,
        )
        .expect("hover for UserMailer inside Admin namespace");

        assert_eq!(hover.name, "UserMailer");
        assert_eq!(hover.ty.to_string(), "singleton(UserMailer)");
    }

    #[test]
    fn hover_workspace_registry_resolves_singleton_instance_signature() {
        let loader = stdlib_loader();
        let tag_manager_source = concat!(
            "require 'singleton'\n",
            "module A\n",
            "  class B\n",
            "    include Singleton\n",
            "  end\n",
            "end\n",
        );
        let tag_manager_analysis = crate::analysis::analyze_cached_file_with_deps(
            tag_manager_source,
            None,
            Some(&loader),
            Some("a_b.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "a_b.rb", tag_manager_analysis);

        let workspace_registry = build_hover_workspace_registry(&mut state, "collection.rb");
        let source = "A::B.instance\n";
        let hover = crate::analysis::hover_at(
            source,
            Some(&workspace_registry),
            &loader,
            "collection.rb",
            1,
            5,
        )
        .expect("singleton instance hover");

        assert_eq!(hover.name, "instance");
        assert_eq!(hover.ty.to_string(), "A::B");
        assert_eq!(hover.display_rbs.as_deref(), Some("instance: -> A::B"));
    }

    #[test]
    fn hover_workspace_registry_resolves_extend_self_module_helper() {
        let loader = stdlib_loader();
        let helper_source = concat!(
            "module Helper\n",
            "  extend self\n",
            "\n",
            "  def greet\n",
            "    \"hello\"\n",
            "  end\n",
            "end\n",
        );
        let helper_analysis = crate::analysis::analyze_cached_file_with_deps(
            helper_source,
            None,
            Some(&loader),
            Some("lib/helper.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = new_test_state();
        insert_test_cache(&mut state, "lib/helper.rb", helper_analysis);

        let workspace_registry = build_hover_workspace_registry(&mut state, "app/models/use.rb");
        let consumer_source = concat!(
            "class Use\n",
            "  def call\n",
            "    Helper.greet\n",
            "  end\n",
            "end\n",
        );
        let hover = crate::analysis::hover_at(
            consumer_source,
            Some(&workspace_registry),
            &loader,
            "app/models/use.rb",
            3,
            11,
        )
        .expect("extend self helper hover");

        assert_eq!(hover.name, "greet");
        assert_eq!(hover.ty.to_string(), "\"hello\"");
        assert_eq!(hover.display_rbs.as_deref(), Some("greet: -> \"hello\""));
    }

    #[test]
    fn hover_workspace_registry_resolves_module_function_helper() {
        let loader = stdlib_loader();
        let helper_source = concat!(
            "module Helper\n",
            "  module_function\n",
            "\n",
            "  def greet\n",
            "    \"hello\"\n",
            "  end\n",
            "end\n",
        );
        let helper_analysis = crate::analysis::analyze_cached_file_with_deps(
            helper_source,
            None,
            Some(&loader),
            Some("lib/helper.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = new_test_state();
        insert_test_cache(&mut state, "lib/helper.rb", helper_analysis);

        let workspace_registry = build_hover_workspace_registry(&mut state, "app/models/use.rb");
        let consumer_source = concat!(
            "class Use\n",
            "  def call\n",
            "    Helper.greet\n",
            "  end\n",
            "end\n",
        );
        let hover = crate::analysis::hover_at(
            consumer_source,
            Some(&workspace_registry),
            &loader,
            "app/models/use.rb",
            3,
            11,
        )
        .expect("module_function helper hover");

        assert_eq!(hover.name, "greet");
        assert_eq!(hover.ty.to_string(), "\"hello\"");
        assert_eq!(hover.display_rbs.as_deref(), Some("greet: -> \"hello\""));
    }

    #[test]
    fn hover_workspace_registry_prefers_singleton_mailer_proxy_signature() {
        let loader = stdlib_loader();
        let rails_options = AnalysisOptions {
            rails_mode: true,
            dsl_activation: DslActivation::with_rails_mode(true),
            project_versions: ProjectVersions::default(),
            project_root: None,
        };
        let mailer_source = concat!(
            "class DeviseMailer < Devise::Mailer\n",
            "  def reset_password_instructions(record, token, opts = {})\n",
            "    super\n",
            "  end\n",
            "end\n",
        );
        let mailer_analysis = crate::analysis::analyze_cached_file_with_deps(
            mailer_source,
            None,
            Some(&loader),
            Some("app/mailers/devise_mailer.rb"),
            rails_options.clone(),
        )
        .0;
        let mut state = new_test_state();
        state.rails_mode = true;
        state.dsl_activation = DslActivation::with_rails_mode(true);
        insert_test_cache(&mut state, "app/mailers/devise_mailer.rb", mailer_analysis);

        let workspace_registry = build_hover_workspace_registry(
            &mut state,
            "app/mailers/previews/devise_mailer_preview.rb",
        );
        let preview_source = concat!(
            "class DeviseMailerPreview < ActionMailer::Preview\n",
            "  def reset_password_instructions_preview\n",
            "    DeviseMailer.reset_password_instructions(nil, \"faketoken\", {})\n",
            "  end\n",
            "end\n",
        );
        let hover = crate::analysis::hover_at_with_analysis_options(
            preview_source,
            Some(&workspace_registry),
            &loader,
            "app/mailers/previews/devise_mailer_preview.rb",
            3,
            17,
            rails_options,
        )
        .expect("mailer class helper hover");

        assert_eq!(hover.name, "reset_password_instructions");
        assert_eq!(hover.ty.to_string(), "ActionMailer::MessageDelivery");
        assert!(
            hover
                .display_rbs
                .as_deref()
                .is_some_and(|sig| sig.ends_with("-> ActionMailer::MessageDelivery")),
            "expected mailer proxy signature, got {:?}",
            hover.display_rbs,
        );
    }

    #[test]
    fn hover_workspace_registry_resolves_intermediate_singleton_method_in_chain() {
        let loader = stdlib_loader();
        let provider_source = concat!(
            "class A\n",
            "  class << self\n",
            "    def pool\n",
            "      @pool ||= Pool.new\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        let provider_analysis = crate::analysis::analyze_cached_file_with_deps(
            provider_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "a.rb", provider_analysis);

        let workspace_registry = build_hover_workspace_registry(&mut state, "b.rb");
        let source = "A.pool.checkout\n";
        let hover =
            crate::analysis::hover_at(source, Some(&workspace_registry), &loader, "b.rb", 1, 3)
                .expect("intermediate singleton method hover");

        assert_eq!(hover.name, "pool");
        assert_eq!(hover.ty.to_string(), "Pool");
        assert_eq!(hover.display_rbs.as_deref(), Some("pool: -> Pool"));
    }

    #[test]
    fn cached_hover_with_workspace_registry_resolves_intermediate_singleton_method_in_chain() {
        let loader = stdlib_loader();
        let provider_source = concat!(
            "class A\n",
            "  class << self\n",
            "    def pool\n",
            "      @pool ||= Pool.new\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = "A.pool.checkout\n";
        let provider_analysis = crate::analysis::analyze_cached_file_with_deps(
            provider_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some("b.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "a.rb", provider_analysis);

        let workspace_registry = build_hover_workspace_registry(&mut state, "b.rb");
        let hover = consumer_analysis
            .hover_at(
                consumer_source,
                line_col_to_offset(consumer_source.as_bytes(), 1, 3).expect("pool byte offset"),
                &loader,
                Some(&workspace_registry),
            )
            .expect("cached intermediate singleton method hover");

        assert_eq!(hover.name, "pool");
        assert_eq!(hover.ty.to_string(), "Pool");
        assert_eq!(hover.display_rbs.as_deref(), Some("pool: -> Pool"));
    }

    #[test]
    fn display_reanalysis_prefers_fresh_hover_over_current_file_cache() {
        let loader = stdlib_loader();
        let provider_source = concat!(
            "class A\n",
            "  class << self\n",
            "    def foo(x)\n",
            "      x\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = "A.foo(1.0)\n";
        let provider_analysis = crate::analysis::analyze_cached_file_with_deps(
            provider_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let stale_consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some("b.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "a.rb", provider_analysis);
        insert_test_cache(&mut state, "b.rb", stale_consumer_analysis);

        let (_fresh_analysis, workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, "b.rb", consumer_source);
        let hover = crate::analysis::hover_at_with_analysis_options(
            consumer_source,
            Some(&workspace_registry),
            &loader,
            "b.rb",
            1,
            3,
            AnalysisOptions::default(),
        )
        .expect("fresh hover");

        assert_eq!(hover.name, "foo");
        // The return value resolves through the param marker to the call site's Float
        // (keeps the displayed param and return type consistent).
        assert_eq!(
            hover.display_rbs.as_deref(),
            Some("foo: (Float x) -> Float")
        );
    }

    #[test]
    fn display_reanalysis_prefers_fresh_code_lens_signature_over_current_file_cache() {
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(1.0)\n",
            "  end\n",
            "end\n",
        );
        let shared_analysis = crate::analysis::analyze_cached_file_with_deps(
            shared_source,
            None,
            Some(&loader),
            Some("shared.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        insert_test_cache(&mut state, "shared.rb", shared_analysis);
        insert_test_cache(&mut state, "a.rb", consumer_analysis);

        let (fresh_analysis, _workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, "shared.rb", shared_source);
        let (_class_name, method) = fresh_analysis
            .methods_for_file("shared.rb")
            .into_iter()
            .find(|(_class_name, method)| method.name == "foo")
            .expect("fresh method");

        assert_eq!(
            format_method_sig_for_lens_with_names(&method, true),
            "(Float x) -> Float"
        );
    }

    #[test]
    fn definition_line_method_sig_from_fresh_analysis_uses_same_file_callsites() {
        let loader = stdlib_loader();
        let source = "def foo(x)\n  x\nend\n\nfoo(1.0)\n";
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };

        let (analysis, _workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, "sample.rb", source);
        let param_offset = line_col_to_offset(source.as_bytes(), 1, 8).expect("param offset");
        let method_offset =
            method_name_offset_for_definition_line(source, param_offset).expect("method offset");
        let sig = analysis
            .method_definition_sig_at(method_offset, &loader, None)
            .expect("definition sig");
        assert_eq!(
            format_method_sig_for_lens_with_names(&sig, true),
            "(Float x) -> Float"
        );
        let hover = analysis
            .hover_at(source, param_offset, &loader, None)
            .expect("raw hover");
        assert_eq!(hover.name, "x");
        let enriched = enrich_hover_from_definition_context(
            &analysis,
            "sample.rb",
            source,
            param_offset,
            hover,
            &loader,
            &TypeRegistry::new(),
        );
        assert_eq!(enriched.ty.to_string(), "Float");
    }

    #[test]
    fn display_reanalysis_skips_workspace_for_self_contained_file() {
        let source = "module Ml\n  def self.table_name_prefix\n    'ml_'\n  end\nend\n";
        let unrelated = "class Other\n  def answer\n    42\n  end\nend\n";
        let mut state = new_test_state();
        let (analysis, deps) = crate::analysis::analyze_file_facts_with_deps(
            source,
            None,
            Some(&state.stdlib_loader),
            Some("ml.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "ml.rb".into(),
            crate::workspace_state::hash_content(source),
            analysis,
            deps,
        );
        let (other_analysis, other_deps) = crate::analysis::analyze_file_facts_with_deps(
            unrelated,
            None,
            Some(&state.stdlib_loader),
            Some("other.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "other.rb".into(),
            crate::workspace_state::hash_content(unrelated),
            other_analysis,
            other_deps,
        );

        let (_analysis, workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, "ml.rb", source);

        assert!(
            workspace_registry.class_names().is_empty(),
            "self-contained files should not materialize unrelated workspace classes"
        );
    }

    #[test]
    fn display_lru_cache_survives_tab_switch() {
        let provider_source = "class Provider\n  def greeting\n    \"hello\"\n  end\nend\n";
        let consumer_source = "class Consumer\n  def call\n    Provider.new.greeting\n  end\nend\n";
        let mut state = new_test_state();

        let (provider_analysis, provider_deps) = crate::analysis::analyze_file_facts_with_deps(
            provider_source,
            None,
            Some(&state.stdlib_loader),
            Some("provider.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "provider.rb".into(),
            crate::workspace_state::hash_content(provider_source),
            provider_analysis,
            provider_deps,
        );

        let (consumer_analysis, consumer_deps) = crate::analysis::analyze_file_facts_with_deps(
            consumer_source,
            None,
            Some(&state.stdlib_loader),
            Some("consumer.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "consumer.rb".into(),
            crate::workspace_state::hash_content(consumer_source),
            consumer_analysis,
            consumer_deps,
        );

        assert!(
            !state
                .workspace_state
                .display_can_skip_workspace_context("provider.rb")
        );
        assert!(
            !state
                .workspace_state
                .display_can_skip_workspace_context("consumer.rb")
        );

        let (provider_display, _) =
            TydaLsp::analyze_current_file_for_display(&mut state, "provider.rb", provider_source);
        assert_eq!(
            provider_display.registry().class_count(),
            1,
            "display snapshot should drop workspace preload copies"
        );
        assert!(provider_display.registry().has_class("Provider"));
        TydaLsp::analyze_current_file_for_display(&mut state, "consumer.rb", consumer_source);

        state.workspace_state.last_timings = crate::workspace_state::WorkspaceTimings::default();
        TydaLsp::analyze_current_file_for_display(&mut state, "provider.rb", provider_source);
        let timings = state.workspace_state.last_timings;
        assert_eq!(
            timings.registry_build,
            std::time::Duration::ZERO,
            "return-to-A should hit display LRU without rebuilding registry"
        );
        assert_eq!(
            timings.current_file_solve,
            std::time::Duration::ZERO,
            "return-to-A should hit display LRU without re-solving"
        );
    }

    #[test]
    fn display_ondemand_imports_inbound_call_sites() {
        let box_source = "class Box\n  def wrap(x)\n    x\n  end\nend\n";
        let caller_source = "Box.new.wrap(1)\n";
        let mut state = new_test_state();

        let (box_analysis, box_deps) = crate::analysis::analyze_file_facts_with_deps(
            box_source,
            None,
            Some(&state.stdlib_loader),
            Some("box.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "box.rb".into(),
            crate::workspace_state::hash_content(box_source),
            box_analysis,
            box_deps,
        );

        let (caller_analysis, caller_deps) = crate::analysis::analyze_file_facts_with_deps(
            caller_source,
            None,
            Some(&state.stdlib_loader),
            Some("caller.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "caller.rb".into(),
            crate::workspace_state::hash_content(caller_source),
            caller_analysis,
            caller_deps,
        );

        assert!(
            !state
                .workspace_state
                .display_can_skip_workspace_context("box.rb")
        );

        let (analysis, _) =
            TydaLsp::analyze_current_file_for_display(&mut state, "box.rb", box_source);
        assert_eq!(
            analysis.registry().lookup_method_return_type("Box", "wrap"),
            Some(crate::types::Type::Integer),
            "cross-file call sites must still refine params under OnDemand display solve"
        );
    }

    #[test]
    #[ignore = "display-scope pruning removed for correctness; any workspace change invalidates the display cache now"]
    fn display_reanalysis_reuses_cached_display_when_unrelated_file_changes() {
        let provider_source = "class Provider\n  def greeting\n    \"hello\"\n  end\nend\n";
        let consumer_source = "class Consumer\n  def call\n    Provider.new.greeting\n  end\nend\n";
        let mut state = new_test_state();

        let (provider_analysis, provider_deps) = crate::analysis::analyze_file_facts_with_deps(
            provider_source,
            None,
            Some(&state.stdlib_loader),
            Some("provider.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "provider.rb".into(),
            crate::workspace_state::hash_content(provider_source),
            provider_analysis,
            provider_deps,
        );

        let (consumer_analysis, consumer_deps) = crate::analysis::analyze_file_facts_with_deps(
            consumer_source,
            None,
            Some(&state.stdlib_loader),
            Some("consumer.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "consumer.rb".into(),
            crate::workspace_state::hash_content(consumer_source),
            consumer_analysis,
            consumer_deps,
        );

        for idx in 0..70 {
            let filler_source = format!("class Filler{idx}\nend\n");
            let file_name = format!("filler_{idx}.rb");
            let (analysis, deps) = crate::analysis::analyze_file_facts_with_deps(
                &filler_source,
                None,
                Some(&state.stdlib_loader),
                Some(&file_name),
                AnalysisOptions::default(),
            );
            state.workspace_state.upsert_file(
                file_name,
                crate::workspace_state::hash_content(&filler_source),
                analysis,
                deps,
            );
        }

        let (_analysis1, workspace_registry1) =
            TydaLsp::analyze_current_file_for_display(&mut state, "provider.rb", provider_source);

        let unrelated_source = "class FillerAlpha\n  def value\n    1\n  end\nend\n";
        let (unrelated_analysis, unrelated_deps) = crate::analysis::analyze_file_facts_with_deps(
            unrelated_source,
            None,
            Some(&state.stdlib_loader),
            Some("filler_alpha.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "filler_alpha.rb".into(),
            crate::workspace_state::hash_content(unrelated_source),
            unrelated_analysis,
            unrelated_deps,
        );

        let (_analysis2, workspace_registry2) =
            TydaLsp::analyze_current_file_for_display(&mut state, "provider.rb", provider_source);

        assert!(
            Arc::ptr_eq(&workspace_registry1, &workspace_registry2),
            "current-file display cache should survive unrelated workspace changes"
        );
    }

    #[test]
    fn display_reanalysis_reuses_workspace_registry_when_current_file_changes() {
        let provider_source = "class Provider\n  def greeting\n    \"hello\"\n  end\nend\n";
        let consumer_source = "class Consumer\n  def call\n    Provider.new.greeting\n  end\nend\n";
        let consumer_source_updated =
            "class Consumer\n  def call\n    Provider.new.greeting\n  end\nend\n# edit\n";
        let mut state = new_test_state();

        let (provider_analysis, provider_deps) = crate::analysis::analyze_file_facts_with_deps(
            provider_source,
            None,
            Some(&state.stdlib_loader),
            Some("provider.rb"),
            AnalysisOptions::default(),
        );
        state.workspace_state.upsert_file(
            "provider.rb".into(),
            crate::workspace_state::hash_content(provider_source),
            provider_analysis,
            provider_deps,
        );

        let (_analysis1, workspace_registry1) =
            TydaLsp::analyze_current_file_for_display(&mut state, "consumer.rb", consumer_source);
        let (_analysis2, workspace_registry2) = TydaLsp::analyze_current_file_for_display(
            &mut state,
            "consumer.rb",
            consumer_source_updated,
        );

        assert!(
            Arc::ptr_eq(&workspace_registry1, &workspace_registry2),
            "current-file edits should reuse the resolved workspace registry when context is unchanged"
        );
    }

    #[test]
    fn definition_hover_enrichment_uses_fresh_methods_for_module_source() {
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "a.rb", consumer_analysis);

        let (analysis, workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, "shared.rb", shared_source);
        let raw_hover = crate::analysis::hover_at_with_analysis_options(
            shared_source,
            Some(&workspace_registry),
            &loader,
            "shared.rb",
            2,
            6,
            AnalysisOptions::default(),
        )
        .expect("raw module definition hover");
        let enriched = enrich_hover_from_definition_context(
            &analysis,
            "shared.rb",
            shared_source,
            line_col_to_offset(shared_source.as_bytes(), 2, 6).expect("method offset"),
            raw_hover,
            &loader,
            &workspace_registry,
        );

        assert_eq!(
            enriched.display_rbs.as_deref(),
            Some("(String x, ?y: untyped, ?z: bool) -> String")
        );
    }

    #[test]
    fn definition_hover_enrichment_uses_fresh_methods_for_absolute_path() {
        let dir = tempdir().expect("tempdir");
        let loader = stdlib_loader();
        let shared_path = dir.path().join("shared.rb");
        let consumer_path = dir.path().join("a.rb");
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some(&consumer_path.to_string_lossy()),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(
            &mut state,
            &consumer_path.to_string_lossy(),
            consumer_analysis,
        );

        let shared_file_path = shared_path.to_string_lossy().to_string();
        let (analysis, workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, &shared_file_path, shared_source);
        let raw_hover = crate::analysis::hover_at_with_analysis_options(
            shared_source,
            Some(&workspace_registry),
            &loader,
            &shared_file_path,
            2,
            6,
            AnalysisOptions::default(),
        )
        .expect("raw hover");
        let enriched = enrich_hover_from_definition_context(
            &analysis,
            &shared_file_path,
            shared_source,
            line_col_to_offset(shared_source.as_bytes(), 2, 6).expect("offset"),
            raw_hover,
            &loader,
            &workspace_registry,
        );

        assert_eq!(
            enriched.display_rbs.as_deref(),
            Some("(String x) -> String")
        );
    }

    #[test]
    fn compare_definition_sig_sources_for_workspace_enriched_definition() {
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "a.rb", consumer_analysis);

        let (analysis, workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, "shared.rb", shared_source);
        let byte_offset = line_col_to_offset(shared_source.as_bytes(), 2, 6).expect("offset");
        let from_methods =
            method_sig_for_definition_line(&analysis, "shared.rb", shared_source, byte_offset)
                .expect("methods sig");
        let from_definition = analysis
            .method_definition_sig_at(
                method_name_offset_for_definition_line(shared_source, byte_offset)
                    .expect("method offset"),
                &loader,
                Some(&workspace_registry),
            )
            .expect("definition sig");

        assert_eq!(
            format_method_sig_for_lens_with_names(&from_methods, true),
            "(String x, ?y: untyped, ?z: bool) -> String"
        );
        assert_eq!(
            format_method_sig_for_lens_with_names(&from_definition, true),
            "(String x, ?y: untyped, ?z: bool) -> String"
        );
    }

    #[test]
    fn definition_sig_with_default_and_workspace_callsite_keeps_union() {
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x = 1)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "a.rb", consumer_analysis);

        let (analysis, workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, "shared.rb", shared_source);
        let method_offset = line_col_to_offset(shared_source.as_bytes(), 2, 6).expect("offset");
        let sig = analysis
            .method_definition_sig_at(method_offset, &loader, Some(&workspace_registry))
            .expect("definition sig");

        let display = format_method_sig_for_lens_with_names(&sig, true);
        assert!(display.contains("Integer | String") || display.contains("String | Integer"));
    }

    #[test]
    fn definition_param_hover_with_default_and_workspace_callsite_uses_union() {
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x = 1)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "a.rb", consumer_analysis);

        let (analysis, workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, "shared.rb", shared_source);
        let raw_hover = crate::analysis::hover_at_with_analysis_options(
            shared_source,
            Some(&workspace_registry),
            &loader,
            "shared.rb",
            2,
            10,
            AnalysisOptions::default(),
        )
        .expect("raw param hover");
        let enriched = enrich_hover_from_definition_context(
            &analysis,
            "shared.rb",
            shared_source,
            line_col_to_offset(shared_source.as_bytes(), 2, 10).expect("param offset"),
            raw_hover,
            &loader,
            &workspace_registry,
        );

        let rendered = format_hover_body(&enriched);
        assert!(rendered.contains("Integer | String") || rendered.contains("String | Integer"));
    }

    #[test]
    fn hover_workspace_registry_resolves_cross_file_method_definition_param() {
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let shared_analysis = crate::analysis::analyze_cached_file_with_deps(
            shared_source,
            None,
            Some(&loader),
            Some("shared.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "shared.rb", shared_analysis);
        insert_test_cache(&mut state, "a.rb", consumer_analysis.clone());

        let workspace_registry = build_hover_workspace_registry(&mut state, "shared.rb");
        let hover = crate::analysis::hover_at(
            shared_source,
            Some(&workspace_registry),
            &loader,
            "shared.rb",
            2,
            10,
        )
        .expect("cross-file definition param hover");

        assert_eq!(hover.name, "x");
        assert_eq!(hover.ty.to_string(), "String");
    }

    #[test]
    fn hover_workspace_registry_resolves_cross_file_method_definition_signature() {
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let shared_analysis = crate::analysis::analyze_cached_file_with_deps(
            shared_source,
            None,
            Some(&loader),
            Some("shared.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "shared.rb", shared_analysis);
        insert_test_cache(&mut state, "a.rb", consumer_analysis.clone());

        let workspace_registry = build_hover_workspace_registry(&mut state, "shared.rb");
        let hover = crate::analysis::hover_at(
            shared_source,
            Some(&workspace_registry),
            &loader,
            "shared.rb",
            2,
            6,
        )
        .expect("cross-file definition method hover");

        assert_eq!(hover.name, "foo");
        assert_eq!(
            hover.display_rbs.as_deref(),
            Some("(String x, ?y: untyped, ?z: bool) -> String")
        );
    }

    #[test]
    fn hover_workspace_registry_uses_pre_scanned_workspace_cache() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let app_models = root.join("app/models");
        let concerns_dir = app_models.join("concerns");
        std::fs::create_dir_all(&concerns_dir).expect("concerns dir");

        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let shared_path = concerns_dir.join("shared.rb");
        let consumer_path = app_models.join("a.rb");
        std::fs::write(&shared_path, shared_source).expect("write shared");
        std::fs::write(&consumer_path, consumer_source).expect("write consumer");

        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: Some(root.to_path_buf()),
            analysis_unit_roots: Some(vec![root.join("app")]),
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        let shared_uri = Url::from_file_path(&shared_path).expect("shared uri");
        state
            .documents
            .insert(shared_uri.clone(), shared_source.to_string());
        let shared_analysis_for_cache = crate::analysis::analyze_cached_file_with_deps(
            shared_source,
            None,
            Some(&state.stdlib_loader),
            Some(&shared_path.to_string_lossy()),
            AnalysisOptions {
                rails_mode: false,
                dsl_activation: DslActivation::default(),
                project_versions: ProjectVersions::default(),
                project_root: Some(root.to_path_buf()),
            },
        )
        .0;
        insert_test_cache(
            &mut state,
            shared_path.to_string_lossy().as_ref(),
            shared_analysis_for_cache,
        );
        let consumer_analysis_for_cache = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&state.stdlib_loader),
            Some(&consumer_path.to_string_lossy()),
            AnalysisOptions {
                rails_mode: false,
                dsl_activation: DslActivation::default(),
                project_versions: ProjectVersions::default(),
                project_root: Some(root.to_path_buf()),
            },
        )
        .0;
        insert_test_cache(
            &mut state,
            consumer_path.to_string_lossy().as_ref(),
            consumer_analysis_for_cache,
        );

        let workspace_registry =
            build_hover_workspace_registry(&mut state, &shared_path.to_string_lossy());
        let hover = crate::analysis::hover_at_with_analysis_options(
            shared_source,
            Some(&workspace_registry),
            &state.stdlib_loader,
            &shared_path.to_string_lossy(),
            2,
            6,
            AnalysisOptions {
                rails_mode: false,
                dsl_activation: DslActivation::default(),
                project_versions: ProjectVersions::default(),
                project_root: Some(root.to_path_buf()),
            },
        )
        .expect("workspace fallback hover");

        assert_eq!(
            hover.display_rbs.as_deref(),
            Some("(String x, ?y: untyped, ?z: bool) -> String")
        );
    }

    #[test]
    fn hover_workspace_registry_enriches_mixin_call_site_signature() {
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let shared_analysis = crate::analysis::analyze_cached_file_with_deps(
            shared_source,
            None,
            Some(&loader),
            Some("shared.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some("a.rb"),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(&mut state, "shared.rb", shared_analysis);
        insert_test_cache(&mut state, "a.rb", consumer_analysis.clone());

        let workspace_registry = build_hover_workspace_registry(&mut state, "a.rb");
        let byte_offset =
            line_col_to_offset(consumer_source.as_bytes(), 4, 4).expect("call-site byte offset");
        let cached_hover = consumer_analysis.hover_at(
            consumer_source,
            byte_offset,
            &loader,
            Some(&workspace_registry),
        );
        assert_eq!(
            cached_hover
                .as_ref()
                .and_then(|hover| hover.display_rbs.as_deref()),
            Some("foo: (String x, ?y: untyped, ?z: bool) -> untyped")
        );

        let hover = crate::analysis::hover_at(
            consumer_source,
            Some(&workspace_registry),
            &loader,
            "a.rb",
            4,
            4,
        )
        .expect("cross-file call-site hover");

        assert_eq!(hover.name, "foo");
        assert_eq!(
            hover.display_rbs.as_deref(),
            Some("foo: (String x, ?y: untyped, ?z: bool) -> untyped")
        );
    }

    #[test]
    fn hover_workspace_registry_enriches_mixin_call_site_signature_with_absolute_paths() {
        let dir = tempdir().expect("tempdir");
        let shared_path = dir.path().join("shared.rb");
        let consumer_path = dir.path().join("a.rb");
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let shared_analysis = crate::analysis::analyze_cached_file_with_deps(
            shared_source,
            None,
            Some(&loader),
            Some(&shared_path.to_string_lossy()),
            AnalysisOptions {
                rails_mode: false,
                dsl_activation: DslActivation::default(),
                project_versions: ProjectVersions::default(),
                project_root: Some(dir.path().to_path_buf()),
            },
        )
        .0;
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some(&consumer_path.to_string_lossy()),
            AnalysisOptions {
                rails_mode: false,
                dsl_activation: DslActivation::default(),
                project_versions: ProjectVersions::default(),
                project_root: Some(dir.path().to_path_buf()),
            },
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: Some(dir.path().to_path_buf()),
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(
            &mut state,
            shared_path.to_string_lossy().as_ref(),
            shared_analysis,
        );
        insert_test_cache(
            &mut state,
            consumer_path.to_string_lossy().as_ref(),
            consumer_analysis.clone(),
        );

        let workspace_registry =
            build_hover_workspace_registry(&mut state, &consumer_path.to_string_lossy());
        let byte_offset =
            line_col_to_offset(consumer_source.as_bytes(), 4, 4).expect("call-site byte offset");
        let cached_hover = consumer_analysis.hover_at(
            consumer_source,
            byte_offset,
            &loader,
            Some(&workspace_registry),
        );
        assert_eq!(
            cached_hover
                .as_ref()
                .and_then(|hover| hover.display_rbs.as_deref()),
            Some("foo: (String x, ?y: untyped, ?z: bool) -> untyped")
        );

        let mut enriched_workspace_registry = TypeRegistry::new();
        let consumer_path_string = consumer_path.to_string_lossy().to_string();
        let mut file_paths: Vec<_> = state
            .workspace_state
            .file_paths()
            .map(str::to_string)
            .collect();
        file_paths.sort();
        for file_path in file_paths {
            if file_path == consumer_path_string {
                continue;
            }
            let entry = state
                .workspace_state
                .workspace_file(&file_path)
                .expect("entry must exist for sorted path");
            entry
                .analysis
                .apply_to_registry(&mut enriched_workspace_registry);
        }
        consumer_analysis.apply_to_registry(&mut enriched_workspace_registry);
        enriched_workspace_registry.propagate_call_sites_for_hover();
        let enriched_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            Some(&enriched_workspace_registry),
            Some(&loader),
            Some(&consumer_path.to_string_lossy()),
            AnalysisOptions {
                rails_mode: false,
                dsl_activation: DslActivation::default(),
                project_versions: ProjectVersions::default(),
                project_root: Some(dir.path().to_path_buf()),
            },
        )
        .0;
        let mut enriched_analysis = enriched_analysis;
        enriched_analysis.merge_registry(consumer_analysis.registry());
        let enriched_hover = enriched_analysis.hover_at(
            consumer_source,
            byte_offset,
            &loader,
            Some(&workspace_registry),
        );
        assert_eq!(
            enriched_hover
                .as_ref()
                .and_then(|hover| hover.display_rbs.as_deref()),
            Some("foo: (String x, ?y: untyped, ?z: bool) -> untyped")
        );
    }

    #[test]
    fn hover_workspace_registry_enriches_mixin_call_site_without_project_root() {
        let dir = tempdir().expect("tempdir");
        let shared_path = dir.path().join("shared.rb");
        let consumer_path = dir.path().join("a.rb");
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let shared_analysis = crate::analysis::analyze_cached_file_with_deps(
            shared_source,
            None,
            Some(&loader),
            Some(&shared_path.to_string_lossy()),
            AnalysisOptions::default(),
        )
        .0;
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some(&consumer_path.to_string_lossy()),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(
            &mut state,
            shared_path.to_string_lossy().as_ref(),
            shared_analysis,
        );
        insert_test_cache(
            &mut state,
            consumer_path.to_string_lossy().as_ref(),
            consumer_analysis.clone(),
        );

        let workspace_registry =
            build_hover_workspace_registry(&mut state, &consumer_path.to_string_lossy());
        let byte_offset =
            line_col_to_offset(consumer_source.as_bytes(), 4, 4).expect("call-site byte offset");
        let cached_hover = consumer_analysis.hover_at(
            consumer_source,
            byte_offset,
            &loader,
            Some(&workspace_registry),
        );
        assert_eq!(
            cached_hover
                .as_ref()
                .and_then(|hover| hover.display_rbs.as_deref()),
            Some("foo: (String x, ?y: untyped, ?z: bool) -> untyped")
        );
    }

    #[test]
    fn choose_better_hover_result_prefers_fewer_untyped_params() {
        let primary = crate::analysis::HoverResult {
            name: "foo".to_string(),
            ty: Type::Untyped,
            display_rbs: Some("(untyped x, ?y: untyped, ?z: bool) -> untyped".to_string()),
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };
        let fallback = crate::analysis::HoverResult {
            name: "foo".to_string(),
            ty: Type::Untyped,
            display_rbs: Some("(String x, ?y: untyped, ?z: bool) -> untyped".to_string()),
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };

        let chosen = choose_better_hover_result(Some(primary), Some(fallback)).expect("chosen");
        assert_eq!(
            chosen.display_rbs.as_deref(),
            Some("(String x, ?y: untyped, ?z: bool) -> untyped")
        );
    }

    #[test]
    fn choose_better_hover_result_counts_untyped_by_signature_slots() {
        let primary = crate::analysis::HoverResult {
            name: "foo".to_string(),
            ty: Type::Untyped,
            display_rbs: Some("(untyped x, ?y: untyped, ?z: bool) -> untyped".to_string()),
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };
        let fallback = crate::analysis::HoverResult {
            name: "foo".to_string(),
            ty: Type::Untyped,
            display_rbs: Some(
                "(String x, ?y: (untyped | untyped), ?z: bool) -> untyped".to_string(),
            ),
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };

        let chosen = choose_better_hover_result(Some(primary), Some(fallback)).expect("chosen");
        assert_eq!(
            chosen.display_rbs.as_deref(),
            Some("(String x, ?y: (untyped | untyped), ?z: bool) -> untyped")
        );
    }

    #[test]
    fn choose_better_hover_result_prefers_signature_over_plain_untyped() {
        let primary = crate::analysis::HoverResult {
            name: "foo".to_string(),
            ty: Type::Untyped,
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };
        let fallback = crate::analysis::HoverResult {
            name: "foo".to_string(),
            ty: Type::Untyped,
            display_rbs: Some("(String x, ?y: untyped, ?z: bool) -> untyped".to_string()),
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };

        let chosen = choose_better_hover_result(Some(primary), Some(fallback)).expect("chosen");
        assert_eq!(
            chosen.display_rbs.as_deref(),
            Some("(String x, ?y: untyped, ?z: bool) -> untyped")
        );
    }

    #[test]
    fn choose_better_hover_result_prefers_richer_value_union() {
        let primary = crate::analysis::HoverResult {
            name: "x".to_string(),
            ty: Type::Integer,
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };
        let fallback = crate::analysis::HoverResult {
            name: "x".to_string(),
            ty: Type::Union(vec![Type::Integer, Type::String]),
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };

        let chosen = choose_better_hover_result(Some(primary), Some(fallback)).expect("chosen");
        assert_eq!(chosen.ty.to_string(), "Integer | String");
    }

    #[test]
    fn choose_better_hover_result_prefers_richer_signature_union() {
        let primary = crate::analysis::HoverResult {
            name: "foo".to_string(),
            ty: Type::Untyped,
            display_rbs: Some(
                "(?Integer x, ?y: ActiveSupport::Duration, ?z: bool) -> untyped".to_string(),
            ),
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };
        let fallback = crate::analysis::HoverResult {
            name: "foo".to_string(),
            ty: Type::Untyped,
            display_rbs: Some(
                "(?(Integer | String) x, ?y: ActiveSupport::Duration, ?z: bool) -> untyped"
                    .to_string(),
            ),
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        };

        let chosen = choose_better_hover_result(Some(primary), Some(fallback)).expect("chosen");
        assert_eq!(
            chosen.display_rbs.as_deref(),
            Some("(?(Integer | String) x, ?y: ActiveSupport::Duration, ?z: bool) -> untyped")
        );
    }

    #[test]
    fn is_better_code_lens_sig_prefers_richer_signature_union() {
        let primary = MethodSig {
            name: "foo".to_string(),
            params: vec![
                Param {
                    name: "x".to_string(),
                    param_type: Type::Integer,
                    kind: ParamKind::Optional,
                },
                Param {
                    name: "y".to_string(),
                    param_type: Type::Class(Sym::new("ActiveSupport::Duration")),
                    kind: ParamKind::KeywordOptional,
                },
            ],
            return_type: Type::Untyped,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        };
        let fallback = MethodSig {
            name: "foo".to_string(),
            params: vec![
                Param {
                    name: "x".to_string(),
                    param_type: Type::Union(vec![Type::Integer, Type::String]),
                    kind: ParamKind::Optional,
                },
                Param {
                    name: "y".to_string(),
                    param_type: Type::Class(Sym::new("ActiveSupport::Duration")),
                    kind: ParamKind::KeywordOptional,
                },
            ],
            return_type: Type::Untyped,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        };

        assert!(is_better_code_lens_sig(&fallback, &primary, false));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_resolves_bare_method_call_and_type_params() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source = concat!(
            "class Sample\n",
            "  #: -> Array[Integer]\n",
            "  def foo\n",
            "    [1, 2]\n",
            "  end\n",
            "  def bar\n",
            "    foo.each do |x|\n",
            "      x\n",
            "    end\n",
            "  end\n",
            "end\n",
        );

        let (mut service, mut socket) = initialize_lsp(None).await;
        let requests = open_document(&mut service, &mut socket, &uri, source).await;
        assert_has_code_lens_refresh(&requests);

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(2)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 6, "character": 8 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("hover decode");

        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(
                    language_string.value,
                    "[Tyda] () { (Integer element) -> void } -> Array[Integer]\n    | () -> Enumerator[Integer, Array[Integer]]\n# type params: E = Integer"
                );
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_type_definition_for_class_constant_returns_rb_and_rbs() {
        let dir = tempdir().expect("tempdir");
        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let sig_dir = dir.path().join("sig");
        std::fs::create_dir_all(&sig_dir).expect("create sig dir");
        std::fs::write(sig_dir.join("test.rbs"), "class Foo\nend\n").expect("write rbs");

        let uri = Url::from_file_path(dir.path().join("test.rb")).expect("file uri");
        let source = "class Foo\nend\n\nfoo = Foo.new\nfoo\n";
        std::fs::write(dir.path().join("test.rb"), source).expect("write ruby");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/typeDefinition")
                .id(91)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 3, "character": 6 }
                }))
                .finish(),
        )
        .await
        .expect("typeDefinition request")
        .expect("typeDefinition response");
        let locations: Vec<Location> =
            serde_json::from_value(response.result().cloned().expect("typeDefinition result"))
                .expect("decode typeDefinition");

        assert_eq!(locations.len(), 2);
        assert!(
            locations
                .iter()
                .any(|location| { location.uri.path().ends_with("/sig/test.rbs") })
        );
        assert!(
            locations
                .iter()
                .any(|location| { location.uri.path().ends_with("/test.rb") })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_method_call_returns_method_location() {
        let dir = tempdir().expect("tempdir");
        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let provider_uri = Url::from_file_path(dir.path().join("greeter.rb")).expect("file uri");
        let provider = concat!(
            "class Greeter\n",
            "  def greet\n",
            "    \"hi\"\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(dir.path().join("greeter.rb"), provider).expect("write provider");

        let uri = Url::from_file_path(dir.path().join("use.rb")).expect("file uri");
        let source = "Greeter.new.greet\n";
        std::fs::write(dir.path().join("use.rb"), source).expect("write consumer");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &provider_uri, provider).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/definition")
                .id(93)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 13 }
                }))
                .finish(),
        )
        .await
        .expect("definition request")
        .expect("definition response");
        let locations: Vec<Location> =
            serde_json::from_value(response.result().cloned().expect("definition result"))
                .expect("decode definition");

        assert_eq!(locations.len(), 1);
        let location = &locations[0];
        assert_eq!(location.uri, provider_uri);
        assert_eq!(location.range.start.line, 1);
        assert_eq!(location.range.start.character, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_constructor_new_returns_initialize() {
        let dir = tempdir().expect("tempdir");
        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let provider_uri = Url::from_file_path(dir.path().join("widget.rb")).expect("file uri");
        let provider = concat!(
            "class Widget\n",
            "  def initialize(name)\n",
            "    @name = name\n",
            "  end\n",
            "\n",
            "  def new\n",
            "    :instance_new\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(dir.path().join("widget.rb"), provider).expect("write provider");

        let uri = Url::from_file_path(dir.path().join("use.rb")).expect("file uri");
        let source = "Widget.new(\"x\")\n";
        std::fs::write(dir.path().join("use.rb"), source).expect("write consumer");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &provider_uri, provider).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let locations = request_definition_locations(&mut service, &uri, 0, 8, 931).await;
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, provider_uri);
        assert_eq!(locations[0].range.start.line, 1);
        assert_eq!(locations[0].range.start.character, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_constructor_new_prefers_singleton_new() {
        let dir = tempdir().expect("tempdir");
        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let provider_uri = Url::from_file_path(dir.path().join("widget.rb")).expect("file uri");
        let provider = concat!(
            "class Widget\n",
            "  def self.new(name)\n",
            "    allocate\n",
            "  end\n",
            "\n",
            "  def initialize(name)\n",
            "    @name = name\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(dir.path().join("widget.rb"), provider).expect("write provider");

        let uri = Url::from_file_path(dir.path().join("use.rb")).expect("file uri");
        let source = "Widget.new(\"x\")\n";
        std::fs::write(dir.path().join("use.rb"), source).expect("write consumer");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &provider_uri, provider).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let locations = request_definition_locations(&mut service, &uri, 0, 8, 932).await;
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, provider_uri);
        assert_eq!(locations[0].range.start.line, 1);
        assert_eq!(locations[0].range.start.character, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_constructor_new_uses_inherited_initialize() {
        let dir = tempdir().expect("tempdir");
        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let base_uri = Url::from_file_path(dir.path().join("base.rb")).expect("file uri");
        let base = concat!(
            "class Base\n",
            "  def initialize(name)\n",
            "    @name = name\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(dir.path().join("base.rb"), base).expect("write base");
        let child_uri = Url::from_file_path(dir.path().join("child.rb")).expect("file uri");
        let child = "class Child < Base\nend\n";
        std::fs::write(dir.path().join("child.rb"), child).expect("write child");

        let uri = Url::from_file_path(dir.path().join("use.rb")).expect("file uri");
        let source = "Child.new(\"x\")\n";
        std::fs::write(dir.path().join("use.rb"), source).expect("write consumer");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &base_uri, base).await;
        let _ = open_document(&mut service, &mut socket, &child_uri, child).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let locations = request_definition_locations(&mut service, &uri, 0, 7, 933).await;
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, base_uri);
        assert_eq!(locations[0].range.start.line, 1);
        assert_eq!(locations[0].range.start.character, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_define_method_and_send_names_returns_static_method() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("dynamic.rb")).expect("file uri");
        let source = concat!(
            "class Dynamic\n",
            "  define_method(:hello) { \"hi\" }\n",
            "  define_method(\"farewell\") { \"bye\" }\n",
            "  def greet = \"g\"\n",
            "  def call\n",
            "    send(:hello) # implicit\n",
            "    public_send(\"greet\") # public\n",
            "    try(:hello) # try implicit\n",
            "  end\n",
            "end\n",
            "\n",
            "Dynamic.new.hello\n",
            "Dynamic.new.farewell\n",
            "Dynamic.new.__send__(:greet) # explicit __send__\n",
            "Dynamic.new.send(:greet) # explicit send\n",
            "Dynamic.new.try!(:hello) # explicit try!\n",
        );
        std::fs::write(dir.path().join("dynamic.rb"), source).expect("write ruby");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        macro_rules! assert_definition {
            ($needle:expr, $line:expr, $character:expr, $id:expr) => {{
                let position = position_of(source, $needle);
                let locations =
                    request_definition_locations_at(&mut service, &uri, position, $id).await;
                assert_eq!(locations.len(), 1, "definition for {:?}", $needle);
                assert_eq!(locations[0].uri, uri, "definition uri for {:?}", $needle);
                assert_eq!(
                    locations[0].range.start.line, $line,
                    "definition line for {:?}",
                    $needle
                );
                assert_eq!(
                    locations[0].range.start.character, $character,
                    "definition character for {:?}",
                    $needle
                );
            }};
        }

        assert_definition!("hello) {", 1, 17, 934);
        assert_definition!("farewell\") {", 2, 17, 942);
        assert_definition!(":hello) # implicit", 1, 17, 935);
        assert_definition!("greet\") # public", 3, 2, 936);
        assert_definition!(":hello) # try implicit", 1, 17, 937);
        assert_definition!("hello\nDynamic.new.farewell", 1, 17, 938);
        assert_definition!("farewell\nDynamic.new.__send__", 2, 17, 943);
        assert_definition!(":greet) # explicit __send__", 3, 2, 939);
        assert_definition!(":greet) # explicit send", 3, 2, 940);
        assert_definition!(":hello) # explicit try!", 1, 17, 941);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_attr_reader_and_writer_returns_attr_symbol() {
        let dir = tempdir().expect("tempdir");
        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let provider_uri = Url::from_file_path(dir.path().join("profile.rb")).expect("file uri");
        let provider = concat!(
            "class Profile\n",
            "  attr_reader :name, :title\n",
            "  attr_accessor :age, :score\n",
            "  attr_writer :token, :secret\n",
            "end\n",
        );
        std::fs::write(dir.path().join("profile.rb"), provider).expect("write provider");

        let uri = Url::from_file_path(dir.path().join("use.rb")).expect("file uri");
        let source = concat!(
            "class Use\n",
            "  def self.run\n",
            "    profile = Profile.new\n",
            "    profile.name\n",
            "    profile.title\n",
            "    profile.age = 42\n",
            "    profile.score = 7\n",
            "    profile.token = \"secret\"\n",
            "    profile.secret = \"hidden\"\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(dir.path().join("use.rb"), source).expect("write consumer");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &provider_uri, provider).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let reader_locations = request_definition_locations(&mut service, &uri, 3, 12, 94).await;
        assert_eq!(reader_locations.len(), 1);
        assert_eq!(reader_locations[0].uri, provider_uri);
        assert_eq!(reader_locations[0].range.start.line, 1);
        assert_eq!(reader_locations[0].range.start.character, 15);

        let title_locations = request_definition_locations(&mut service, &uri, 4, 13, 97).await;
        assert_eq!(title_locations.len(), 1);
        assert_eq!(title_locations[0].uri, provider_uri);
        assert_eq!(title_locations[0].range.start.line, 1);
        assert_eq!(title_locations[0].range.start.character, 22);

        let writer_locations = request_definition_locations(&mut service, &uri, 5, 12, 95).await;
        assert_eq!(writer_locations.len(), 1);
        assert_eq!(writer_locations[0].uri, provider_uri);
        assert_eq!(writer_locations[0].range.start.line, 2);
        assert_eq!(writer_locations[0].range.start.character, 17);

        let score_locations = request_definition_locations(&mut service, &uri, 6, 12, 98).await;
        assert_eq!(score_locations.len(), 1);
        assert_eq!(score_locations[0].uri, provider_uri);
        assert_eq!(score_locations[0].range.start.line, 2);
        assert_eq!(score_locations[0].range.start.character, 23);

        let attr_writer_locations =
            request_definition_locations(&mut service, &uri, 7, 12, 96).await;
        assert_eq!(attr_writer_locations.len(), 1);
        assert_eq!(attr_writer_locations[0].uri, provider_uri);
        assert_eq!(attr_writer_locations[0].range.start.line, 3);
        assert_eq!(attr_writer_locations[0].range.start.character, 15);

        let secret_locations = request_definition_locations(&mut service, &uri, 8, 12, 99).await;
        assert_eq!(secret_locations.len(), 1);
        assert_eq!(secret_locations[0].uri, provider_uri);
        assert_eq!(secret_locations[0].range.start.line, 3);
        assert_eq!(secret_locations[0].range.start.character, 23);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_on_attr_symbol_names_returns_generated_method_type() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("profile.rb")).expect("file uri");
        let source = concat!(
            "class Profile\n",
            "  attr_reader :name, :title\n",
            "  attr_accessor :age\n",
            "  attr_writer :token\n",
            "\n",
            "  def initialize\n",
            "    @name = \"Ada\"\n",
            "    @title = \"Dr.\"\n",
            "    @age = 20\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(dir.path().join("profile.rb"), source).expect("write ruby");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let name_hover = request_hover(&mut service, &uri, 1, 15, 944).await;
        assert_eq!(hover_language_value(name_hover), "[Tyda] -> \"Ada\"");

        let title_hover = request_hover(&mut service, &uri, 1, 22, 945).await;
        assert_eq!(hover_language_value(title_hover), "[Tyda] -> \"Dr.\"");

        let age_hover = request_hover(&mut service, &uri, 2, 17, 946).await;
        assert_eq!(hover_language_value(age_hover), "[Tyda] -> 20");

        let token_hover = request_hover(&mut service, &uri, 3, 15, 947).await;
        assert_eq!(
            hover_language_value(token_hover),
            "[Tyda] (untyped token) -> untyped"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_class_constant_returns_rb_and_rbs() {
        let dir = tempdir().expect("tempdir");
        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let sig_dir = dir.path().join("sig");
        std::fs::create_dir_all(&sig_dir).expect("create sig dir");
        std::fs::write(sig_dir.join("test.rbs"), "class Foo\nend\n").expect("write rbs");

        let uri = Url::from_file_path(dir.path().join("test.rb")).expect("file uri");
        let source = "class Foo\nend\n\nfoo = Foo.new\nfoo\n";
        std::fs::write(dir.path().join("test.rb"), source).expect("write ruby");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/definition")
                .id(94)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 3, "character": 6 }
                }))
                .finish(),
        )
        .await
        .expect("definition request")
        .expect("definition response");
        let locations: Vec<Location> =
            serde_json::from_value(response.result().cloned().expect("definition result"))
                .expect("decode definition");

        assert_eq!(locations.len(), 2);
        assert!(
            locations
                .iter()
                .any(|location| { location.uri.path().ends_with("/sig/test.rbs") })
        );
        assert!(
            locations
                .iter()
                .any(|location| { location.uri.path().ends_with("/test.rb") })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_rbs_comment_type_returns_class_location() {
        let dir = tempdir().expect("tempdir");
        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let model_path = dir.path().join("model.rb");
        let model_source = concat!("module Container\n", "  class Item\n", "  end\n", "end\n",);
        std::fs::write(&model_path, model_source).expect("write model");

        let uri = Url::from_file_path(dir.path().join("service.rb")).expect("file uri");
        let source = concat!(
            "module Container\n",
            "  # @rbs item: Item\n",
            "  def self.wrap(item)\n",
            "    item\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(dir.path().join("service.rb"), source).expect("write service");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let locations = request_definition_locations(&mut service, &uri, 1, 16, 1020).await;
        assert_eq!(locations.len(), 1);
        assert!(locations[0].uri.path().ends_with("/model.rb"));
        assert_eq!(locations[0].range.start.line, 1);
        assert_eq!(locations[0].range.start.character, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_local_variable_returns_assignment_not_type() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("test.rb")).expect("file uri");
        let source = concat!(
            "class Foo\n",
            "end\n",
            "\n",
            "def build\n",
            "  value = Foo.new\n",
            "  value\n",
            "end\n",
        );
        std::fs::write(dir.path().join("test.rb"), source).expect("write ruby");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/definition")
                .id(95)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 5, "character": 2 }
                }))
                .finish(),
        )
        .await
        .expect("definition request")
        .expect("definition response");
        let locations: Vec<Location> =
            serde_json::from_value(response.result().cloned().expect("definition result"))
                .expect("decode definition");

        assert_eq!(locations.len(), 1);
        let location = &locations[0];
        assert_eq!(location.uri, uri);
        assert_eq!(location.range.start.line, 4);
        assert_eq!(location.range.start.character, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_parameter_returns_parameter_location() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("test.rb")).expect("file uri");
        let source = concat!("def use(name)\n", "  name\n", "end\n",);
        std::fs::write(dir.path().join("test.rb"), source).expect("write ruby");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/definition")
                .id(96)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 1, "character": 2 }
                }))
                .finish(),
        )
        .await
        .expect("definition request")
        .expect("definition response");
        let locations: Vec<Location> =
            serde_json::from_value(response.result().cloned().expect("definition result"))
                .expect("decode definition");

        assert_eq!(locations.len(), 1);
        let location = &locations[0];
        assert_eq!(location.uri, uri);
        assert_eq!(location.range.start.line, 0);
        assert_eq!(location.range.start.character, 8);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_for_value_constant_returns_constant_assignment() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("test.rb")).expect("file uri");
        let source = concat!(
            "module Gem\n",
            "  VERSION = \"1.0.0\"\n",
            "end\n",
            "Gem::VERSION\n",
        );
        std::fs::write(dir.path().join("test.rb"), source).expect("write ruby");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/definition")
                .id(97)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 3, "character": 5 }
                }))
                .finish(),
        )
        .await
        .expect("definition request")
        .expect("definition response");
        let locations: Vec<Location> =
            serde_json::from_value(response.result().cloned().expect("definition result"))
                .expect("decode definition");

        assert_eq!(locations.len(), 1);
        let location = &locations[0];
        assert_eq!(location.uri, uri);
        assert_eq!(location.range.start.line, 1);
        assert_eq!(location.range.start.character, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_real_rack_static_symbols() {
        let rack_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/rack/lib");
        if !rack_root.exists() {
            eprintln!("skipping: subject/rack/lib missing");
            return;
        }

        let driver_path = rack_root.join(format!(
            "_tyda_definition_driver_{}_.rb",
            std::process::id()
        ));
        let driver_source = concat!(
            "class Driver\n",
            "  def status_table\n",
            "    Rack::Utils::STATUS_WITH_NO_ENTITY_BODY\n",
            "  end\n",
            "  def call_escape(s)\n",
            "    Rack::Utils.escape(s)\n",
            "  end\n",
            "  def response_status\n",
            "    Rack::Response::STATUS_WITH_NO_ENTITY_BODY\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&driver_path, driver_source).expect("write driver");
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(driver_path.clone());

        let root_uri = Url::from_directory_path(&rack_root).expect("root uri");
        let driver_uri = Url::from_file_path(&driver_path).expect("driver uri");
        let utils_uri = Url::from_file_path(rack_root.join("rack/utils.rb")).expect("utils uri");
        let response_uri =
            Url::from_file_path(rack_root.join("rack/response.rb")).expect("response uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &driver_uri, driver_source).await;

        let locations = request_definition_locations(&mut service, &driver_uri, 2, 20, 980).await;
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, utils_uri);
        assert_eq!(locations[0].range.start.line, 653);
        assert_eq!(locations[0].range.start.character, 4);

        let locations = request_definition_locations(&mut service, &driver_uri, 5, 18, 981).await;
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, utils_uri);
        assert_eq!(locations[0].range.start.line, 39);
        assert_eq!(locations[0].range.start.character, 4);

        let locations = request_definition_locations(&mut service, &driver_uri, 8, 22, 982).await;
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, response_uri);
        assert_eq!(locations[0].range.start.line, 28);
        assert_eq!(locations[0].range.start.character, 4);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_real_rake_module_function() {
        let rake_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/rake/lib");
        if !rake_root.exists() {
            eprintln!("skipping: subject/rake/lib missing");
            return;
        }

        let driver_path = rake_root.join(format!(
            "_tyda_definition_driver_{}_.rb",
            std::process::id()
        ));
        let driver_source = concat!(
            "module Driver\n",
            "  def self.probe\n",
            "    Rake.original_dir\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&driver_path, driver_source).expect("write driver");
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(driver_path.clone());

        let root_uri = Url::from_directory_path(&rake_root).expect("root uri");
        let driver_uri = Url::from_file_path(&driver_path).expect("driver uri");
        let rake_module_uri =
            Url::from_file_path(rake_root.join("rake/rake_module.rb")).expect("module uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &driver_uri, driver_source).await;

        let locations = request_definition_locations(&mut service, &driver_uri, 2, 10, 990).await;
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, rake_module_uri);
        assert_eq!(locations[0].range.start.line, 22);
        assert_eq!(locations[0].range.start.character, 4);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_real_rubygems_constants_and_methods() {
        let rubygems_root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/rubygems/lib");
        if !rubygems_root.exists() {
            eprintln!("skipping: subject/rubygems/lib missing");
            return;
        }

        let driver_path = rubygems_root.join(format!(
            "_tyda_definition_driver_{}_.rb",
            std::process::id()
        ));
        let driver_source = concat!(
            "class Driver\n",
            "  def version_text\n",
            "    Gem::VERSION\n",
            "  end\n",
            "  def segments\n",
            "    Gem::Version.new(\"1.2.3\").segments\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&driver_path, driver_source).expect("write driver");
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(driver_path.clone());

        let root_uri = Url::from_directory_path(&rubygems_root).expect("root uri");
        let driver_uri = Url::from_file_path(&driver_path).expect("driver uri");
        let rubygems_uri =
            Url::from_file_path(rubygems_root.join("rubygems.rb")).expect("rubygems uri");
        let version_uri =
            Url::from_file_path(rubygems_root.join("rubygems/version.rb")).expect("version uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &driver_uri, driver_source).await;

        let locations = request_definition_locations(&mut service, &driver_uri, 2, 9, 1001).await;
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, rubygems_uri);
        assert_eq!(locations[0].range.start.line, 11);
        assert_eq!(locations[0].range.start.character, 2);

        let locations = request_definition_locations(&mut service, &driver_uri, 5, 32, 1002).await;
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri, version_uri);
        assert_eq!(locations[0].range.start.line, 322);
        assert_eq!(locations[0].range.start.character, 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_real_sample_methods_classes_and_constants() {
        let sample_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/sample");
        let sample_path = sample_root.join("sample.rb");
        if !sample_path.exists() {
            eprintln!("skipping: subject/sample/sample.rb missing");
            return;
        }

        let sample_source = std::fs::read_to_string(&sample_path).expect("read sample");
        let driver_dir = tempdir().expect("tempdir");
        let driver_path = driver_dir.path().join("driver.rb");
        let driver_source = concat!(
            "class Driver\n",
            "  def self.sample_const\n",
            "    Sample::CONST\n",
            "  end\n",
            "  def self.sample_bar\n",
            "    Sample.new.bar\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&driver_path, driver_source).expect("write driver");

        let sample_uri = Url::from_file_path(&sample_path).expect("sample uri");
        let driver_uri = Url::from_file_path(&driver_path).expect("driver uri");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &sample_uri, &sample_source).await;
        let _ = open_document(&mut service, &mut socket, &driver_uri, driver_source).await;

        let locations = request_definition_locations_at(
            &mut service,
            &sample_uri,
            position_of(&sample_source, "foo.map"),
            1010,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &sample_uri,
            position_of(&sample_source, "def foo"),
            "Sample#bar should jump to Sample#foo",
        );

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "Sample::CONST"),
            1011,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &sample_uri,
            position_of(&sample_source, "class Sample"),
            "Sample constant should jump to class Sample",
        );

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "CONST"),
            1012,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &sample_uri,
            position_of(&sample_source, "CONST ="),
            "Sample::CONST should jump to the constant assignment",
        );
        assert_no_definition_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "::CONST"),
            1015,
        )
        .await;

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "bar\n  end\nend"),
            1013,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &sample_uri,
            position_of(&sample_source, "def bar"),
            "Sample.new.bar should jump to Sample#bar",
        );

        assert_no_definition_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, ".bar"),
            1016,
        )
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_real_mastodon_version_module_functions() {
        let version_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("subject/mastodon/lib/mastodon/version.rb");
        if !version_path.exists() {
            eprintln!("skipping: subject/mastodon/lib/mastodon/version.rb missing");
            return;
        }

        let version_source = std::fs::read_to_string(&version_path).expect("read version");
        let driver_dir = tempdir().expect("tempdir");
        let driver_path = driver_dir.path().join("driver.rb");
        let driver_source = concat!(
            "module Driver\n",
            "  def self.version_text\n",
            "    Mastodon::Version.to_s\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&driver_path, driver_source).expect("write driver");

        let version_uri = Url::from_file_path(&version_path).expect("version uri");
        let driver_uri = Url::from_file_path(&driver_path).expect("driver uri");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &version_uri, &version_source).await;
        let _ = open_document(&mut service, &mut socket, &driver_uri, driver_source).await;

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "Version.to_s"),
            1020,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &version_uri,
            position_of(&version_source, "module Version"),
            "Mastodon::Version should jump to the Version module",
        );
        assert_no_definition_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "::Version"),
            1025,
        )
        .await;

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "to_s\n"),
            1021,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &version_uri,
            position_of(&version_source, "def to_s"),
            "Mastodon::Version.to_s should jump to the module_function definition",
        );

        let locations = request_definition_locations_at(
            &mut service,
            &version_uri,
            position_of(&version_source, "to_a.join"),
            1022,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &version_uri,
            position_of(&version_source, "def to_a"),
            "to_s should jump to the local to_a module function",
        );

        let locations = request_definition_locations_at(
            &mut service,
            &version_uri,
            position_of(&version_source, "major, minor"),
            1023,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &version_uri,
            position_of(&version_source, "def major"),
            "to_a should jump to the local major module function",
        );

        assert_no_definition_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, ".to_s"),
            1024,
        )
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_real_gitlab_alert_subset() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let builder_path = manifest_dir.join("subject/gitlab/lib/gitlab/data_builder/alert.rb");
        let alert_path = manifest_dir.join("subject/gitlab/app/models/alert_management/alert.rb");
        if !builder_path.exists() || !alert_path.exists() {
            eprintln!(
                "skipping: gitlab alert subject files missing: {} {}",
                builder_path.display(),
                alert_path.display()
            );
            return;
        }

        let builder_source = std::fs::read_to_string(&builder_path).expect("read builder");
        let alert_source = std::fs::read_to_string(&alert_path).expect("read alert");
        let driver_dir = tempdir().expect("tempdir");
        let driver_path = driver_dir.path().join("driver.rb");
        let driver_source = concat!(
            "module Driver\n",
            "  def self.alert_class\n",
            "    AlertManagement::Alert\n",
            "  end\n",
            "  def self.max_title_length\n",
            "    AlertManagement::Alert::TITLE_MAX_LENGTH\n",
            "  end\n",
            "  def self.reference_prefix\n",
            "    AlertManagement::Alert.reference_prefix\n",
            "  end\n",
            "  def self.register_event\n",
            "    AlertManagement::Alert.new.register_new_event!\n",
            "  end\n",
            "  def self.build_payload(alert)\n",
            "    Gitlab::DataBuilder::Alert.build(alert)\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&driver_path, driver_source).expect("write driver");

        let builder_uri = Url::from_file_path(&builder_path).expect("builder uri");
        let alert_uri = Url::from_file_path(&alert_path).expect("alert uri");
        let driver_uri = Url::from_file_path(&driver_path).expect("driver uri");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &builder_uri, &builder_source).await;
        let _ = open_document(&mut service, &mut socket, &alert_uri, &alert_source).await;
        let _ = open_document(&mut service, &mut socket, &driver_uri, driver_source).await;

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "Alert\n  end\n  def self.max_title_length"),
            1030,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &alert_uri,
            position_of(&alert_source, "class Alert"),
            "AlertManagement::Alert should jump to the class definition",
        );

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "TITLE_MAX_LENGTH"),
            1031,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &alert_uri,
            position_of(&alert_source, "TITLE_MAX_LENGTH ="),
            "AlertManagement::Alert::TITLE_MAX_LENGTH should jump to the constant",
        );

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(
                driver_source,
                "reference_prefix\n  end\n  def self.register_event",
            ),
            1032,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &alert_uri,
            position_of(&alert_source, "def self.reference_prefix"),
            "AlertManagement::Alert.reference_prefix should jump to the class method",
        );

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "register_new_event!"),
            1033,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &alert_uri,
            position_of(&alert_source, "def register_new_event!"),
            "AlertManagement::Alert.new.register_new_event! should jump to the instance method",
        );

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "Alert.build"),
            1034,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &builder_uri,
            position_of(&builder_source, "module Alert"),
            "Gitlab::DataBuilder::Alert should jump to the module definition",
        );
        assert_no_definition_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "::Alert.build"),
            1037,
        )
        .await;

        let locations = request_definition_locations_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, "build(alert)"),
            1035,
        )
        .await;
        assert_single_definition_location(
            &locations,
            &builder_uri,
            position_of(&builder_source, "def build"),
            "Gitlab::DataBuilder::Alert.build should jump to the extend-self method",
        );

        assert_no_definition_at(
            &mut service,
            &driver_uri,
            position_of(driver_source, ".build"),
            1036,
        )
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_type_definition_for_local_variable_returns_rb_and_rbs() {
        let dir = tempdir().expect("tempdir");
        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let sig_dir = dir.path().join("sig");
        std::fs::create_dir_all(&sig_dir).expect("create sig dir");
        std::fs::write(sig_dir.join("test.rbs"), "class Foo\nend\n").expect("write rbs");

        let uri = Url::from_file_path(dir.path().join("test.rb")).expect("file uri");
        let source = "class Foo\nend\n\nfoo = Foo.new\nfoo\n";
        std::fs::write(dir.path().join("test.rb"), source).expect("write ruby");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/typeDefinition")
                .id(92)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 4, "character": 0 }
                }))
                .finish(),
        )
        .await
        .expect("typeDefinition request")
        .expect("typeDefinition response");
        let locations: Vec<Location> =
            serde_json::from_value(response.result().cloned().expect("typeDefinition result"))
                .expect("decode typeDefinition");

        assert_eq!(locations.len(), 2);
        assert!(
            locations
                .iter()
                .any(|location| { location.uri.path().ends_with("/sig/test.rbs") })
        );
        assert!(
            locations
                .iter()
                .any(|location| { location.uri.path().ends_with("/test.rb") })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_rack_real_status_constant() {
        // subject/rack real-tree E2E: hovering `STATUS_WITH_NO_ENTITY_BODY` shows the Hash type from utils.rb.
        let subject_root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/rack/lib/rack");
        if !subject_root.exists() {
            eprintln!("skipping rack real-source test: subject/rack not present");
            return;
        }
        let content_type_path = subject_root.join("content_type.rb");
        let utils_path = subject_root.join("utils.rb");
        let Ok(content_type_source) = std::fs::read_to_string(&content_type_path) else {
            eprintln!("skipping: cannot read content_type.rb");
            return;
        };
        let Ok(utils_source) = std::fs::read_to_string(&utils_path) else {
            eprintln!("skipping: cannot read utils.rb");
            return;
        };

        let root_uri = Url::from_directory_path(subject_root.parent().unwrap().parent().unwrap())
            .expect("root uri");
        let content_type_uri = Url::from_file_path(&content_type_path).expect("content_type uri");
        let utils_uri = Url::from_file_path(&utils_path).expect("utils uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &utils_uri, &utils_source).await;
        let _ = open_document(
            &mut service,
            &mut socket,
            &content_type_uri,
            &content_type_source,
        )
        .await;

        // content_type.rb line 26 (1-based): `unless STATUS_WITH_NO_ENTITY_BODY.key?(status.to_i)`.
        // LSP protocol is 0-based.
        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(201)
                .params(serde_json::json!({
                    "textDocument": { "uri": content_type_uri },
                    "position": { "line": 25, "character": 15 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert!(
                    language_string.value.contains("Hash["),
                    "real Rack STATUS_WITH_NO_ENTITY_BODY hover should show Hash[...], got: {}",
                    language_string.value
                );
                assert!(
                    !language_string.value.contains("untyped\n"),
                    "hover should not leak a bare untyped, got: {}",
                    language_string.value
                );
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    async fn assert_lsp_hover_contains(
        root: &std::path::Path,
        files: &[(&str, &str)],
        target_file: &str,
        line: u32,
        character: u32,
        expected_substr: &str,
        label: &str,
    ) {
        for (name, body) in files {
            std::fs::write(root.join(name), body).expect("write file");
        }
        let root_uri = Url::from_directory_path(root).expect("root uri");
        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        for (name, body) in files {
            let uri = Url::from_file_path(root.join(name)).expect("file uri");
            let _ = open_document(&mut service, &mut socket, &uri, body).await;
        }
        let target_uri = Url::from_file_path(root.join(target_file)).expect("target uri");
        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(701)
                .params(serde_json::json!({
                    "textDocument": { "uri": target_uri },
                    "position": { "line": line, "character": character }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover = serde_json::from_value(
            response
                .result()
                .cloned()
                .unwrap_or_else(|| panic!("{label}: hover returned null")),
        )
        .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert!(
                    language_string.value.contains(expected_substr),
                    "{label}: expected hover to contain {expected_substr:?}, got: {}",
                    language_string.value
                );
            }
            other => panic!("{label}: unexpected hover contents: {other:?}"),
        }
    }

    /// Batch-verifies multiple cross-file hover patterns against the real rack tree (the driver's hover matches the definition site's type).
    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_real_rack_cross_file_patterns() {
        let rack_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/rack/lib");
        if !rack_root.exists() {
            eprintln!("skipping: subject/rack/lib missing");
            return;
        }

        // Places the driver inside subject/rack/lib so the scan also picks up the rack sources (cleaned up afterward).
        let driver_path = rack_root.join(format!("_tyda_driver_{}_.rb", std::process::id()));

        let driver_source = concat!(
            // line 0
            "class Driver\n",
            "  def status_table\n",
            // line 2: Hash[...]
            "    Rack::Utils::STATUS_WITH_NO_ENTITY_BODY\n",
            "  end\n",
            "  def key_check(status)\n",
            // line 5: bool
            "    Rack::Utils::STATUS_WITH_NO_ENTITY_BODY.key?(status)\n",
            "  end\n",
            "  def call_escape(s)\n",
            // line 8: Rack::Utils.escape takes string, returns String
            "    Rack::Utils.escape(s)\n",
            "  end\n",
            "  def content_type_instance\n",
            // line 11: returns Rack::ContentType instance
            "    Rack::ContentType.new(nil)\n",
            "  end\n",
            "  def builder_use\n",
            // line 14: Rack::Builder.app (class method)
            "    Rack::Builder.app\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&driver_path, driver_source).expect("write driver");
        // Ensure cleanup even if test assertions panic.
        struct DriverCleanup(std::path::PathBuf);
        impl Drop for DriverCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = DriverCleanup(driver_path.clone());

        let root_uri = Url::from_directory_path(&rack_root).expect("root uri");
        let driver_uri = Url::from_file_path(&driver_path).expect("driver uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &driver_uri, driver_source).await;

        let probes: &[(u32, u32, &str, &str)] = &[
            // Rack::Utils::STATUS_WITH_NO_ENTITY_BODY (col 17 = 'S')
            (2, 17, "Hash[", "qualified const path"),
            // .key? on the constant (col 45 inside 'key?')
            (5, 45, "bool", "method on qualified const"),
            // Rack::ContentType.new (col 23 inside 'new')
            (11, 23, "Rack::ContentType", "cross-file Class.new"),
        ];

        for (line, col, expected, label) in probes {
            let response = Service::call(
                &mut service,
                Request::build("textDocument/hover")
                    .id(800 + *line as i64)
                    .params(serde_json::json!({
                        "textDocument": { "uri": driver_uri },
                        "position": { "line": line, "character": col }
                    }))
                    .finish(),
            )
            .await
            .expect("hover req")
            .expect("hover resp");
            let hover: Hover = serde_json::from_value(
                response
                    .result()
                    .cloned()
                    .unwrap_or_else(|| panic!("{label}: null")),
            )
            .expect("decode");
            match hover.contents {
                HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
                    assert!(
                        ls.value.contains(expected),
                        "{label} @{line}:{col}: expected {expected:?}, got: {}",
                        ls.value
                    );
                }
                other => panic!("{label}: unexpected: {other:?}"),
            }
        }
    }

    /// A broad cross-file probe against the real rack tree (excludes stdlib to reliably catch tyda resolution bugs).
    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_real_rack_project_internal_patterns() {
        let rack_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/rack/lib");
        if !rack_root.exists() {
            eprintln!("skipping: subject/rack/lib missing");
            return;
        }

        let driver_path = rack_root.join(format!("_tyda_driver_{}_.rb", std::process::id()));
        let driver_source = concat!(
            // line 0
            "class Driver\n",
            "  def response_headers\n",
            // line 2: attr_reader :headers on Rack::Response → Headers
            "    Rack::Response.new.headers\n",
            "  end\n",
            "  def request_env(env)\n",
            // line 5: attr_reader :env via Rack::Request's include of Env
            "    Rack::Request.new(env).env\n",
            "  end\n",
            "  def response_status\n",
            // line 8: attr_accessor status on Rack::Response — 200 default
            "    Rack::Response.new.status\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&driver_path, driver_source).expect("write driver");
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(driver_path.clone());

        let root_uri = Url::from_directory_path(&rack_root).expect("root uri");
        let driver_uri = Url::from_file_path(&driver_path).expect("driver uri");
        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &driver_uri, driver_source).await;

        let probes: &[(u32, u32, &str, &str)] = &[
            // .headers on Rack::Response instance — col 23 inside 'headers'
            (2, 23, "Headers", "cross-file attr_reader :headers"),
            // Rack::Request#env: verifies method resolution (signature display) even with an untyped return.
            (5, 28, "->", "cross-file attr_reader via include"),
            // .status on Rack::Response — col 23 inside 'status' (default 200)
            (8, 23, "Integer", "cross-file attr_accessor default"),
        ];

        for (line, col, expected, label) in probes {
            let response = Service::call(
                &mut service,
                Request::build("textDocument/hover")
                    .id(1000 + *line as i64)
                    .params(serde_json::json!({
                        "textDocument": { "uri": driver_uri },
                        "position": { "line": line, "character": col }
                    }))
                    .finish(),
            )
            .await
            .expect("hover req")
            .expect("hover resp");
            let hover: Hover = serde_json::from_value(
                response
                    .result()
                    .cloned()
                    .unwrap_or_else(|| panic!("{label}: null at {line}:{col}")),
            )
            .expect("decode");
            match hover.contents {
                HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
                    assert!(
                        ls.value.contains(expected),
                        "{label} at {line}:{col} expected {expected:?}, got: {}",
                        ls.value
                    );
                }
                other => panic!("{label}: unexpected: {other:?}"),
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_real_rack_response_get_header_self() {
        // Verifies a method-call hover within the same file resolves (no token fallback).
        let rack_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/rack/lib");
        if !rack_root.exists() {
            return;
        }
        let root_uri = Url::from_directory_path(&rack_root).expect("root uri");
        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let path = rack_root.join("rack/response.rb");
        let source = std::fs::read_to_string(&path).expect("read response.rb");
        let uri = Url::from_file_path(&path).expect("uri");
        let _ = open_document(&mut service, &mut socket, &uri, &source).await;
        // Line 96 (1-based): `      CHUNKED == get_header(TRANSFER_ENCODING)`
        // LSP 0-based line 95. `get_header` starts at col 17.
        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(1100)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 95, "character": 17 }
                }))
                .finish(),
        )
        .await
        .expect("hover req")
        .expect("hover resp");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("result")).expect("decode");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
                // Must show it's resolved as a method call with known receiver
                // (Rack::Response), not just untyped fallback.
                assert!(
                    ls.value.contains("-> ") || ls.value.contains("get_header"),
                    "get_header hover should show method info, got: {}",
                    ls.value
                );
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_real_rack_class_body_setter_call() {
        // Verifies a class-body receiver call resolves to the attr_accessor setter (regression: unresolved-method without inference).
        let rack_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/rack/lib");
        if !rack_root.exists() {
            return;
        }
        let root_uri = Url::from_directory_path(&rack_root).expect("root uri");
        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let path = rack_root.join("rack/utils.rb");
        let source = std::fs::read_to_string(&path).expect("read utils.rb");
        let uri = Url::from_file_path(&path).expect("uri");
        let _ = open_document(&mut service, &mut socket, &uri, &source).await;
        // Line 35 (1-based): `    self.default_query_parser = QueryParser.make_default(32)`
        // LSP 0-based line 34. `default_query_parser` starts at col 9.
        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(1300)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 34, "character": 9 }
                }))
                .finish(),
        )
        .await
        .expect("hover req")
        .expect("hover resp");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("result")).expect("decode");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
                // Must NOT say "unresolved:" — the setter is defined right
                // above via `class << self; attr_accessor :default_query_parser`.
                assert!(
                    !ls.value.contains("unresolved:"),
                    "class-body self.x= hover should not show unresolved marker, got: {}",
                    ls.value
                );
            }
            other => panic!("unexpected hover: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_lambda_block_params_infer_from_call_sites() {
        // Verifies a lambda block param hover reflects the call-site type (String/Integer), with no token fallback.
        let rack_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/rack/lib");
        if !rack_root.exists() {
            return;
        }
        let root_uri = Url::from_directory_path(&rack_root).expect("root uri");
        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let path = rack_root.join("rack/query_parser.rb");
        let source = std::fs::read_to_string(&path).expect("read query_parser.rb");
        let uri = Url::from_file_path(&path).expect("uri");
        let _ = open_document(&mut service, &mut socket, &uri, &source).await;

        // Hover column positions for the lambda `|key, val|` (key at col 25, val at col 30).
        for (line, col, expected_substr, label) in [
            (45u32, 25u32, "RACK_QUERY_PARSER", "key param"),
            (45, 30, "4096", "val param"),
        ] {
            let response = Service::call(
                &mut service,
                Request::build("textDocument/hover")
                    .id(1200 + line as i64)
                    .params(serde_json::json!({
                        "textDocument": { "uri": uri },
                        "position": { "line": line, "character": col }
                    }))
                    .finish(),
            )
            .await
            .expect("hover req")
            .expect("hover resp");
            let hover: Hover = serde_json::from_value(
                response
                    .result()
                    .cloned()
                    .unwrap_or_else(|| panic!("{label}: null")),
            )
            .expect("decode");
            match hover.contents {
                HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
                    assert!(
                        ls.value.contains(expected_substr),
                        "{label} at {line}:{col} expected {expected_substr:?}, got: {}",
                        ls.value
                    );
                }
                other => panic!("{label}: unexpected: {other:?}"),
            }
        }
    }

    /// Cross-file type resolution for an identifier hover against the real rack tree (a real UX test).
    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_real_rack_in_place() {
        let rack_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("subject/rack/lib");
        if !rack_root.exists() {
            eprintln!("skipping: subject/rack/lib missing");
            return;
        }
        let root_uri = Url::from_directory_path(&rack_root).expect("root uri");
        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;

        let sites: &[(&str, u32, u32, &str, &str)] = &[
            // content_type.rb line 26 (0-based 25): `unless STATUS_WITH_NO_ENTITY_BODY.key?(status.to_i)`
            // bare STATUS_WITH_NO_ENTITY_BODY via include Rack::Utils
            (
                "rack/content_type.rb",
                25,
                15,
                "Hash[",
                "bare constant via include",
            ),
            // deflater.rb line 154 (0-based 153): `if Utils::STATUS_WITH_NO_ENTITY_BODY.key?(status.to_i) ||`
            (
                "rack/deflater.rb",
                153,
                16,
                "Hash[",
                "qualified Utils::CONST",
            ),
            // lint.rb line 793 (0-based 792): `if Rack::Utils::STATUS_WITH_NO_ENTITY_BODY.key? status.to_i`
            (
                "rack/lint.rb",
                792,
                31,
                "Hash[",
                "fully qualified Rack::Utils::CONST",
            ),
            // content_length.rb line 22 (0-based 21): `if !STATUS_WITH_NO_ENTITY_BODY.key?(status.to_i) &&`
            (
                "rack/content_length.rb",
                21,
                12,
                "Hash[",
                "bare constant in content_length",
            ),
        ];

        for (rel_path, line, col, expected, label) in sites {
            let path = rack_root.join(rel_path);
            let Ok(source) = std::fs::read_to_string(&path) else {
                panic!("{label}: cannot read {}", path.display());
            };
            let uri = Url::from_file_path(&path).expect("uri");
            let _ = open_document(&mut service, &mut socket, &uri, &source).await;
            let response = Service::call(
                &mut service,
                Request::build("textDocument/hover")
                    .id(900 + *line as i64)
                    .params(serde_json::json!({
                        "textDocument": { "uri": uri },
                        "position": { "line": line, "character": col }
                    }))
                    .finish(),
            )
            .await
            .expect("hover req")
            .expect("hover resp");
            let hover: Hover =
                serde_json::from_value(response.result().cloned().unwrap_or_else(|| {
                    panic!("{label}: hover returned null at {rel_path}:{line}:{col}")
                }))
                .expect("decode");
            match hover.contents {
                HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
                    assert!(
                        ls.value.contains(expected),
                        "{label} at {rel_path}:{line}:{col} expected {expected:?}, got: {}",
                        ls.value
                    );
                }
                other => panic!("{label}: unexpected: {other:?}"),
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_cross_file_method_call_returns_definition_type() {
        // Method defined in one file, called from another.
        let dir = tempdir().expect("tempdir");
        assert_lsp_hover_contains(
            dir.path(),
            &[
                (
                    "greeter.rb",
                    "class Greeter\n  def hello\n    \"hi\"\n  end\nend\n",
                ),
                (
                    "user.rb",
                    "class User\n  def call\n    Greeter.new.hello\n  end\nend\n",
                ),
            ],
            "user.rb",
            2,
            16, // line "    Greeter.new.hello" — `.hello` at col 16
            "\"hi\"",
            "cross-file method return",
        )
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_cross_file_attr_reader_returns_ivar_type() {
        let dir = tempdir().expect("tempdir");
        assert_lsp_hover_contains(
            dir.path(),
            &[
                (
                    "profile.rb",
                    "class Profile\n  attr_reader :name\n  def initialize\n    @name = \"Alice\"\n  end\nend\n",
                ),
                (
                    "consumer.rb",
                    "class Consumer\n  def call\n    Profile.new.name\n  end\nend\n",
                ),
            ],
            "consumer.rb",
            2, 16, // `Profile.new.name` — `.name` at col 16
            "\"Alice\"",
            "cross-file attr_reader",
        )
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_cross_file_superclass_method() {
        let dir = tempdir().expect("tempdir");
        assert_lsp_hover_contains(
            dir.path(),
            &[
                (
                    "base.rb",
                    "class Animal\n  def sound\n    \"generic\"\n  end\nend\n",
                ),
                ("dog.rb", "class Dog < Animal\nend\n"),
                (
                    "user.rb",
                    "class User\n  def call\n    Dog.new.sound\n  end\nend\n",
                ),
            ],
            "user.rb",
            2,
            12, // Dog.new.sound — .sound
            "\"generic\"",
            "cross-file superclass method",
        )
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_cross_file_module_method() {
        let dir = tempdir().expect("tempdir");
        assert_lsp_hover_contains(
            dir.path(),
            &[
                (
                    "greetable.rb",
                    "module Greetable\n  def hello = \"hi\"\nend\n",
                ),
                ("person.rb", "class Person\n  include Greetable\nend\n"),
                (
                    "check.rb",
                    "class Check\n  def call\n    Person.new.hello\n  end\nend\n",
                ),
            ],
            "check.rb",
            2,
            16, // .hello
            "\"hi\"",
            "cross-file module include method",
        )
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_cross_file_class_new_type() {
        let dir = tempdir().expect("tempdir");
        assert_lsp_hover_contains(
            dir.path(),
            &[
                (
                    "service.rb",
                    "class Greeter\n  def hello\n    \"hi\"\n  end\nend\n",
                ),
                (
                    "user.rb",
                    "class User\n  def call\n    Greeter.new\n  end\nend\n",
                ),
            ],
            "user.rb",
            2,
            12, // Greeter.new — `.new`
            "Greeter",
            "cross-file Class.new returns instance",
        )
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_cross_file_nested_constant() {
        let dir = tempdir().expect("tempdir");
        assert_lsp_hover_contains(
            dir.path(),
            &[
                (
                    "lib.rb",
                    "module Lib\n  module Inner\n    MAX = 42\n  end\nend\n",
                ),
                (
                    "user.rb",
                    "class User\n  def call\n    Lib::Inner::MAX\n  end\nend\n",
                ),
            ],
            "user.rb",
            2,
            16, // Lib::Inner::MAX
            "42",
            "cross-file nested constant path",
        )
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_cross_file_bare_constant_via_include() {
        // LSP hover E2E: a cross-file value reached via include must not become untyped (regression in the display pipeline's mixin chain-load).
        let dir = tempdir().expect("tempdir");
        let utils_path = dir.path().join("utils.rb");
        let consumer_path = dir.path().join("content_type.rb");
        let utils_source = concat!(
            "module Rack\n",
            "  module Utils\n",
            "    STATUS_WITH_NO_ENTITY_BODY = { 200 => true }\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "module Rack\n",
            "  class ContentType\n",
            "    include Rack::Utils\n",
            "    def call\n",
            "      STATUS_WITH_NO_ENTITY_BODY\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&utils_path, utils_source).expect("write utils");
        std::fs::write(&consumer_path, consumer_source).expect("write consumer");

        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let utils_uri = Url::from_file_path(&utils_path).expect("utils uri");
        let consumer_uri = Url::from_file_path(&consumer_path).expect("consumer uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &utils_uri, utils_source).await;
        let _ = open_document(&mut service, &mut socket, &consumer_uri, consumer_source).await;

        // Line 4 (0-based) "      STATUS_WITH_NO_ENTITY_BODY" — `S` at col 6.
        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(101)
                .params(serde_json::json!({
                    "textDocument": { "uri": consumer_uri },
                    "position": { "line": 4, "character": 6 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert!(
                    language_string.value.starts_with("[Tyda] Hash[200, true]"),
                    "bare constant via include should resolve to the Hash type, got: {}",
                    language_string.value
                );
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_updates_after_did_change() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source_v1 = "def foo(x)\n  x\nend\n\nfoo(1)\n";
        let source_v2 = "def foo(x)\n  x\nend\n\nfoo(1.0)\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let requests = open_document(&mut service, &mut socket, &uri, source_v1).await;
        assert_has_code_lens_refresh(&requests);

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(93)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 8 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(language_string.value, "[Tyda] Integer");
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }

        let _requests = change_document(&mut service, &mut socket, &uri, 2, source_v2).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(94)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 8 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(language_string.value, "[Tyda] Float");
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_tracks_incremental_content_updates() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source = concat!(
            "class User\n",
            "  #: (\"test\") -> void\n",
            "  def initialize(name)\n",
            "    @name = name\n",
            "  end\n",
            "\n",
            "  def name = @name\n",
            "\n",
            "  def greeting = \"hello, #{@name}\"\n",
            "end\n",
        );
        let param_offset = source.find("def initialize(name)").unwrap() + "def initialize(".len();
        let ivar_offset = source.find("@name").unwrap();
        let method_offset = source.find("  def name").unwrap() + "  def ".len();
        let greeting_offset = source.find("greeting").unwrap();
        let assignment_end = source.find("    @name = name").unwrap() + "    @name = name".len();
        let method_end = source.find("  def name = @name").unwrap() + "  def name = @name".len();
        let greeting_end = source.find("  def greeting = \"hello, #{@name}\"").unwrap()
            + "  def greeting = \"hello, #{@name}\"".len();
        let checks = [
            (
                source.find("User").unwrap(),
                source.find("class User").unwrap() + "class User".len(),
                "[Tyda] singleton(User)",
            ),
            (param_offset, assignment_end, "[Tyda] \"test\""),
            (ivar_offset, assignment_end, "[Tyda] \"test\""),
            (method_offset, method_end, "[Tyda] -> \"test\""),
            (greeting_offset, greeting_end, "[Tyda] -> \"hello, test\""),
        ];

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, "").await;
        for (index, &prefix_end) in [assignment_end, method_end, greeting_end]
            .iter()
            .enumerate()
        {
            let prefix = &source[..prefix_end];
            let _ =
                change_document(&mut service, &mut socket, &uri, index as i32 + 2, prefix).await;
            for &(offset, ready_end, expected) in &checks {
                if prefix_end < ready_end {
                    continue;
                }
                let position = byte_offset_to_lsp_position(source, offset);
                let hover = request_hover(
                    &mut service,
                    &uri,
                    position.line,
                    position.character,
                    index as i64 + 100,
                )
                .await;
                assert_eq!(
                    hover_language_value(hover),
                    expected,
                    "hover at source offset {offset} after prefix {prefix_end}: {prefix:?}"
                );
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_uses_live_concern_class_methods_after_did_change() {
        let dir = tempdir().expect("tempdir");
        let shared_uri =
            Url::from_file_path(dir.path().join("searchable.rb")).expect("shared file uri");
        let consumer_uri =
            Url::from_file_path(dir.path().join("article.rb")).expect("consumer file uri");
        let shared_v1 = "module Searchable\nend\n";
        let shared_v2 = concat!(
            "module Searchable\n",
            "  extend ActiveSupport::Concern\n",
            "\n",
            "  class_methods do\n",
            "    def search\n",
            "      \"live\"\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class Article\n",
            "  include Searchable\n",
            "end\n",
            "\n",
            "Article.search\n",
        );
        let search_pos = position_of(consumer_source, "search");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &shared_uri, shared_v1).await;
        let _ = open_document(&mut service, &mut socket, &consumer_uri, consumer_source).await;

        let _requests = change_document(&mut service, &mut socket, &shared_uri, 2, shared_v2).await;

        let hover = request_hover(
            &mut service,
            &consumer_uri,
            search_pos.line,
            search_pos.character,
            95,
        )
        .await;
        let value = hover_language_value(hover);
        assert!(
            value.contains("\"live\""),
            "updated Concern class_methods return type should be visible, got: {value}"
        );
    }

    /// The cold scan stays full-root even with a didOpen-first ordering (prevents unresolved definitions in unopened files).
    #[tokio::test(flavor = "current_thread")]
    async fn lsp_cold_scan_stays_full_when_did_open_precedes_initialized() {
        let dir = tempdir().expect("tempdir");
        let root_uri = Url::from_directory_path(dir.path()).expect("root uri");
        let provider = "class Alpha\n  def hi\n    1\n  end\nend\n";
        std::fs::write(dir.path().join("alpha.rb"), provider).expect("write provider");
        let consumer_uri = Url::from_file_path(dir.path().join("use.rb")).expect("file uri");
        let consumer = "Alpha.new.hi\nAlpha.new.nope\n";
        std::fs::write(dir.path().join("use.rb"), consumer).expect("write consumer");

        let (mut service, mut socket) = tower_lsp::LspService::new(TydaLsp::new);
        let response = Service::call(
            &mut service,
            Request::build("initialize")
                .id(1)
                .params(serde_json::json!({
                    "capabilities": {},
                    "rootUri": root_uri,
                }))
                .finish(),
        )
        .await
        .expect("initialize request")
        .expect("initialize response");
        assert!(response.is_ok(), "initialize failed: {response:?}");

        // Delivers didOpen before initialized.
        let _ = open_document(&mut service, &mut socket, &consumer_uri, consumer).await;
        let (_, _) = drive_request(
            &mut service,
            &mut socket,
            Request::build("initialized")
                .params(serde_json::json!({}))
                .finish(),
        )
        .await;

        // Hover synchronously waits for the scan to finish via ensure_workspace_scanned.
        let _ = request_hover(&mut service, &consumer_uri, 0, 10, 90).await;

        // The diagnostics published by the post-scan didChange are the final state.
        let requests = change_document(&mut service, &mut socket, &consumer_uri, 2, consumer).await;
        let diagnostics = diagnostics_notifications(&requests, &consumer_uri);
        let published = diagnostics.last().expect("publish diagnostics");
        let codes: Vec<String> = published
            .diagnostics
            .iter()
            .filter_map(|diag| match &diag.code {
                Some(NumberOrString::String(code)) => Some(code.clone()),
                _ => None,
            })
            .collect();
        assert!(
            !codes
                .iter()
                .any(|code| code == UNRESOLVED_CONSTANT_DIAGNOSTIC_CODE),
            "Alpha must resolve from the scanned workspace, got {published:?}"
        );
        let missing: Vec<_> = published
            .diagnostics
            .iter()
            .filter(|diag| {
                diag.code
                    == Some(NumberOrString::String(
                        MISSING_METHOD_DIAGNOSTIC_CODE.to_string(),
                    ))
            })
            .collect();
        assert_eq!(
            missing.len(),
            1,
            "only the real typo should remain, got {published:?}"
        );
        assert!(missing[0].message.contains("`nope`"), "{published:?}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_publishes_warning_diagnostic_for_missing_method() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source = "class Widget\nend\n\nWidget.new.missing\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let requests = open_document(&mut service, &mut socket, &uri, source).await;
        assert_has_code_lens_refresh(&requests);
        let diagnostics = diagnostics_notifications(&requests, &uri);
        let published = diagnostics.last().expect("publish diagnostics");

        assert_eq!(published.diagnostics.len(), 1);
        let diagnostic = &published.diagnostics[0];
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(
            diagnostic.code,
            Some(NumberOrString::String(
                MISSING_METHOD_DIAGNOSTIC_CODE.to_string()
            ))
        );
        assert_eq!(diagnostic.source.as_deref(), Some("Tyda"));
        assert_eq!(
            diagnostic.message,
            "Method `missing` not found for `Widget`"
        );
        assert_eq!(diagnostic.range.start, Position::new(3, 11));
        assert_eq!(diagnostic.range.end, Position::new(3, 18));

        let close_requests = close_document(&mut service, &mut socket, &uri).await;
        let close_diagnostics = diagnostics_notifications(&close_requests, &uri);
        let cleared = close_diagnostics
            .last()
            .expect("diagnostics clear on close");
        assert!(cleared.diagnostics.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_publishes_error_diagnostic_for_argument_type_mismatch() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source = "class A\n  #: (String) -> Integer\n  def foo(s)\n    s.length\n  end\nend\n\nA.new.foo(1)\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let requests = open_document(&mut service, &mut socket, &uri, source).await;
        let diagnostics = diagnostics_notifications(&requests, &uri);
        let published = diagnostics.last().expect("publish diagnostics");

        assert_eq!(published.diagnostics.len(), 1, "{published:?}");
        let diagnostic = &published.diagnostics[0];
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(
            diagnostic.code,
            Some(NumberOrString::String(
                ARGUMENT_TYPE_MISMATCH_DIAGNOSTIC_CODE.to_string()
            ))
        );
        assert_eq!(diagnostic.source.as_deref(), Some("Tyda"));
        assert!(
            diagnostic.message.contains("String"),
            "message should name the expected type: {}",
            diagnostic.message
        );
        // Squiggle is on the `1` argument, not the method name.
        assert_eq!(diagnostic.range.start, Position::new(7, 10));
        assert_eq!(diagnostic.range.end, Position::new(7, 11));

        let close_requests = close_document(&mut service, &mut socket, &uri).await;
        let close_diagnostics = diagnostics_notifications(&close_requests, &uri);
        let cleared = close_diagnostics
            .last()
            .expect("diagnostics clear on close");
        assert!(cleared.diagnostics.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_suppresses_diagnostics_with_line_ignore_comments() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("ignored.rb")).expect("file uri");
        let source = concat!(
            "class Widget\n",
            "  #: (String) -> Integer\n",
            "  def foo(s)\n",
            "    s.length\n",
            "  end\n",
            "end\n",
            "\n",
            "Widget.new.missing # tyda: ignore[missing_method]\n",
            "Widget.new.foo(1) # tyda: ignore[argument_type_mismatch]\n",
            "Widget.new.missing\n",
            "Widget.new.foo(1)\n",
        );

        let (mut service, mut socket) = initialize_lsp(None).await;
        let requests = open_document(&mut service, &mut socket, &uri, source).await;
        let diagnostics = diagnostics_notifications(&requests, &uri);
        let published = diagnostics.last().expect("publish diagnostics");

        assert_eq!(published.diagnostics.len(), 2, "{published:?}");
        assert!(published.diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    MISSING_METHOD_DIAGNOSTIC_CODE.to_string(),
                ))
                && diagnostic.range.start.line == 9
        }));
        assert!(published.diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    ARGUMENT_TYPE_MISMATCH_DIAGNOSTIC_CODE.to_string(),
                ))
                && diagnostic.range.start.line == 10
        }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_reports_unused_line_ignore_comments() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("unused_ignore.rb")).expect("file uri");
        let source = concat!(
            "class Widget\n",
            "  #: (String) -> Integer\n",
            "  def foo(s)\n",
            "    s.length\n",
            "  end\n",
            "end\n",
            "\n",
            "Widget.new.missing # tyda: ignore[argument_type_mismatch]\n",
            "Widget.new.foo(1) # tyda: ignore[missing_method]\n",
            "Widget.new.foo(\"ok\") # tyda: ignore\n",
        );

        let (mut service, mut socket) = initialize_lsp(None).await;
        let requests = open_document(&mut service, &mut socket, &uri, source).await;
        let diagnostics = diagnostics_notifications(&requests, &uri);
        let published = diagnostics.last().expect("publish diagnostics");

        assert_eq!(published.diagnostics.len(), 5, "{published:?}");
        assert!(published.diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    MISSING_METHOD_DIAGNOSTIC_CODE.to_string(),
                ))
        }));
        assert!(published.diagnostics.iter().any(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    ARGUMENT_TYPE_MISMATCH_DIAGNOSTIC_CODE.to_string(),
                ))
        }));
        assert_eq!(
            published
                .diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.code
                        == Some(NumberOrString::String(
                            UNUSED_IGNORE_DIAGNOSTIC_CODE.to_string(),
                        ))
                })
                .count(),
            3
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_clears_missing_method_diagnostic_after_definition_appears() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source_v1 = "class Widget\nend\n\nWidget.new.missing\n";
        let source_v2 = "class Widget\n  def missing\n    1\n  end\nend\n\nWidget.new.missing\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let requests = open_document(&mut service, &mut socket, &uri, source_v1).await;
        let published = diagnostics_notifications(&requests, &uri)
            .pop()
            .expect("initial diagnostics");
        assert_eq!(published.diagnostics.len(), 1);

        let requests = change_document(&mut service, &mut socket, &uri, 2, source_v2).await;
        assert_has_code_lens_refresh(&requests);
        let published = diagnostics_notifications(&requests, &uri)
            .pop()
            .expect("updated diagnostics");
        assert!(
            published.diagnostics.is_empty(),
            "method definition should clear diagnostics: {:?}",
            published.diagnostics
        );
    }

    /// Restores the debounce override even if the test panics.
    struct DiagnosticsDebounceOverride(u64);

    impl DiagnosticsDebounceOverride {
        fn set(ms: u64) -> Self {
            Self(DIAGNOSTICS_CHANGE_DEBOUNCE_TEST_MS.swap(ms, Ordering::SeqCst))
        }
    }

    impl Drop for DiagnosticsDebounceOverride {
        fn drop(&mut self) {
            DIAGNOSTICS_CHANGE_DEBOUNCE_TEST_MS.store(self.0, Ordering::SeqCst);
        }
    }

    async fn drain_client_requests(socket: &mut ClientSocket) -> Vec<Request> {
        let mut requests = Vec::new();
        while let Ok(Some(request)) =
            tokio::time::timeout(std::time::Duration::from_millis(80), socket.next()).await
        {
            respond_ok(socket, &request, serde_json::json!(null)).await;
            requests.push(request);
        }
        requests
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_debounces_did_change_diagnostics_to_the_latest_content() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source_v1 = "class Widget\nend\n\nWidget.new.missing\n";
        // An intermediate keystroke that would publish its own diagnostics undebounced.
        let source_v2 = "class Widget\nend\n\nWidget.new.missin\n";
        let source_v3 = "class Widget\n  def missing\n    1\n  end\nend\n\nWidget.new.missing\n";

        // Long enough that both changes land inside one window, short enough that a
        // concurrent test's 150ms drain still sees its own publish.
        let _debounce = DiagnosticsDebounceOverride::set(DIAGNOSTICS_CHANGE_DEBOUNCE_MS / 2);
        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source_v1).await;

        for (version, text) in [(2, source_v2), (3, source_v3)] {
            Service::call(
                &mut service,
                Request::build("textDocument/didChange")
                    .params(serde_json::json!({
                        "textDocument": { "uri": uri, "version": version },
                        "contentChanges": [{ "text": text }]
                    }))
                    .finish(),
            )
            .await
            .expect("didChange notification");
        }

        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        let requests = drain_client_requests(&mut socket).await;
        let published = diagnostics_notifications(&requests, &uri);
        assert_eq!(
            published.len(),
            1,
            "both changes should coalesce into one publish, got {published:?}"
        );
        assert!(
            published[0].diagnostics.is_empty(),
            "the last content defines `missing`, so diagnostics should clear: {:?}",
            published[0].diagnostics
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_missing_method_diagnostic_marks_literal_send_target() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source = "class Widget\nend\n\nWidget.new.send(:missing)\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let requests = open_document(&mut service, &mut socket, &uri, source).await;
        let published = diagnostics_notifications(&requests, &uri)
            .pop()
            .expect("publish diagnostics");
        assert_eq!(published.diagnostics.len(), 1);
        let diagnostic = &published.diagnostics[0];
        assert_eq!(
            diagnostic.message,
            "Method `missing` not found for `Widget`"
        );
        assert_eq!(diagnostic.range.start, Position::new(3, 17));
        assert_eq!(diagnostic.range.end, Position::new(3, 24));
    }

    // CLI parity: referencing a definition in an unopened file doesn't false-positive as unresolved/missing (resolved via the workspace registry).
    #[test]
    fn lsp_no_false_positive_for_cross_file_definition_reference() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let app = root.join("app");
        std::fs::create_dir_all(&app).expect("app dir");
        let provider_path = app.join("b.rb");
        let consumer_path = app.join("a.rb");
        let provider_source = "class B\n  def hello = 1\nend\n";
        let consumer_source = "class A\n  def call = B.new.hello\nend\n";
        std::fs::write(&provider_path, provider_source).expect("write b");
        std::fs::write(&consumer_path, consumer_source).expect("write a");

        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: Some(root.to_path_buf()),
            analysis_unit_roots: Some(vec![root.join("app")]),
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        // Only the provider is cached from the workspace scan; the consumer is
        // the current document under diagnosis.
        insert_analyzed_test_file(
            &mut state,
            provider_path.to_string_lossy().as_ref(),
            provider_source,
        );

        let consumer_uri = Url::from_file_path(&consumer_path).expect("consumer uri");
        let diagnostics = TydaLsp::diagnostics_for_document_with_state(
            &mut state,
            &consumer_uri,
            consumer_source,
        );
        assert!(
            diagnostics.is_empty(),
            "cross-file constant B and method hello must resolve from the workspace registry: {diagnostics:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_did_change_uses_last_full_content_change() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source_v1 = "def foo(x)\n  x\nend\n\nfoo(1)\n";
        let source_v2 = "def foo(x)\n  x\nend\n\nfoo(1.0)\n";
        let source_v3 = "def foo(x)\n  x\nend\n\nfoo(\"x\")\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source_v1).await;
        let _requests = change_document_raw(
            &mut service,
            &mut socket,
            &uri,
            2,
            vec![
                serde_json::json!({ "text": source_v2 }),
                serde_json::json!({ "text": source_v3 }),
            ],
        )
        .await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(98)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 8 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(language_string.value, "[Tyda] String");
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_codelens_tracks_incremental_newlines_and_deletions() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source = "class Sample\n  def foo\n    1\n  end\nend\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let initial_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(120)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("initial codeLens request")
        .expect("initial codeLens response");
        let initial_lenses: Vec<CodeLens> =
            serde_json::from_value(initial_lenses.result().cloned().expect("codeLens result"))
                .expect("decode initial codeLens");
        assert_eq!(initial_lenses.len(), 1);
        assert_eq!(initial_lenses[0].range.start, Position::new(1, 6));

        let requests = change_document_raw(
            &mut service,
            &mut socket,
            &uri,
            2,
            vec![serde_json::json!({
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 }
                },
                "text": "\n"
            })],
        )
        .await;
        assert_has_code_lens_refresh(&requests);

        let inserted_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(121)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("inserted codeLens request")
        .expect("inserted codeLens response");
        let inserted_lenses: Vec<CodeLens> =
            serde_json::from_value(inserted_lenses.result().cloned().expect("codeLens result"))
                .expect("decode inserted codeLens");
        assert_eq!(inserted_lenses.len(), 1);
        assert_eq!(inserted_lenses[0].range.start, Position::new(2, 6));

        let requests = change_document_raw(
            &mut service,
            &mut socket,
            &uri,
            3,
            vec![serde_json::json!({
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 1, "character": 0 }
                },
                "text": ""
            })],
        )
        .await;
        assert_has_code_lens_refresh(&requests);

        let deleted_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(122)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("deleted codeLens request")
        .expect("deleted codeLens response");
        let deleted_lenses: Vec<CodeLens> =
            serde_json::from_value(deleted_lenses.result().cloned().expect("codeLens result"))
                .expect("decode deleted codeLens");
        assert_eq!(deleted_lenses.len(), 1);
        assert_eq!(deleted_lenses[0].range.start, Position::new(1, 6));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_codelens_uses_source_fallback_for_unannotated_def_during_broken_edit() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source = "class Sample\n  def foo(x)\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(123)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("codeLens request")
        .expect("codeLens response");
        let lenses: Vec<CodeLens> =
            serde_json::from_value(response.result().cloned().expect("codeLens result"))
                .expect("decode codeLens");
        assert_eq!(lenses.len(), 1);
        assert_eq!(lenses[0].range.start, Position::new(1, 6));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_codelens_suppresses_only_method_with_value_corrupting_syntax_error() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        // `def b = "#{a}` is an unterminated string: its inferred signature is
        // garbage, so its codelens is suppressed; `a` keeps a clean one.
        let source = "class A\n  def a = 1\n\n  def b = \"#{a}\nend\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(125)
                .params(serde_json::json!({ "textDocument": { "uri": uri } }))
                .finish(),
        )
        .await
        .expect("codeLens request")
        .expect("codeLens response");
        let lenses: Vec<CodeLens> =
            serde_json::from_value(response.result().cloned().expect("codeLens result"))
                .expect("decode codeLens");
        let lines: Vec<u32> = lenses.iter().map(|l| l.range.start.line).collect();
        assert!(
            lines.contains(&1),
            "method `a` keeps its codelens: {lines:?}"
        );
        assert!(
            !lines.contains(&3),
            "broken method `b` codelens is suppressed: {lines:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_codelens_keeps_real_rake_empty_scope_path_literal() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("subject/rake");
        let scope_path = root.join("lib/rake/scope.rb");
        if !scope_path.exists() {
            eprintln!(
                "skipping: rake scope subject not found: {}",
                scope_path.display()
            );
            return;
        }
        let source = std::fs::read_to_string(&scope_path).expect("read rake scope");
        let root_uri = Url::from_directory_path(&root).expect("rake root uri");
        let scope_uri = Url::from_file_path(&scope_path).expect("scope uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &scope_uri, &source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(124)
                .params(serde_json::json!({
                    "textDocument": { "uri": scope_uri }
                }))
                .finish(),
        )
        .await
        .expect("codeLens request")
        .expect("codeLens response");
        let lenses: Vec<CodeLens> =
            serde_json::from_value(response.result().cloned().expect("codeLens result"))
                .expect("decode codeLens");
        let empty_scope_path = lenses
            .iter()
            .find(|lens| lens.range.start == Position::new(30, 10))
            .expect("EmptyScope#path code lens");
        let command = empty_scope_path.command.as_ref().expect("command");
        assert_eq!(command.title, "#: -> \"\"");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_definition_real_rake_inherited_attr_reader() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("subject/rake");
        let scope_path = root.join("lib/rake/scope.rb");
        let linked_list_path = root.join("lib/rake/linked_list.rb");
        if !scope_path.exists() || !linked_list_path.exists() {
            eprintln!(
                "skipping: rake subject files not found: {} {}",
                scope_path.display(),
                linked_list_path.display()
            );
            return;
        }
        let scope_source = std::fs::read_to_string(&scope_path).expect("read rake scope");
        let root_uri = Url::from_directory_path(&root).expect("rake root uri");
        let scope_uri = Url::from_file_path(&scope_path).expect("scope uri");
        let linked_list_uri = Url::from_file_path(&linked_list_path).expect("linked_list uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &scope_uri, &scope_source).await;

        let dot_response = Service::call(
            &mut service,
            Request::build("textDocument/definition")
                .id(1240)
                .params(serde_json::json!({
                    "textDocument": { "uri": scope_uri },
                    "position": { "line": 19, "character": 23 }
                }))
                .finish(),
        )
        .await
        .expect("definition request on dot")
        .expect("definition response on dot");
        assert!(
            dot_response.result().is_none_or(serde_json::Value::is_null),
            "dot before method name should not jump: {dot_response:?}"
        );

        for (id, character) in [(1241, 24), (1242, 25), (1243, 26), (1244, 28), (1245, 29)] {
            let locations =
                request_definition_locations(&mut service, &scope_uri, 19, character, id).await;
            assert_eq!(locations.len(), 1, "character {character}");
            assert_eq!(locations[0].uri, linked_list_uri, "character {character}");
            assert_eq!(locations[0].range.start.line, 7, "character {character}");
            assert_eq!(
                locations[0].range.start.character, 24,
                "character {character}"
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_after_did_close_uses_on_disk_source() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("sample.rb");
        let uri = Url::from_file_path(&path).expect("file uri");
        let source_on_disk = "def foo(x)\n  x\nend\n\nfoo(1)\n";
        let source_in_editor = "def foo(x)\n  x\nend\n\nfoo(1.0)\n";
        std::fs::write(&path, source_on_disk).expect("write ruby");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let requests = open_document(&mut service, &mut socket, &uri, source_in_editor).await;
        assert!(
            requests
                .iter()
                .any(|request| request.method() == "workspace/codeLens/refresh")
        );

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(95)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 8 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(language_string.value, "[Tyda] Float");
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }

        let requests = close_document(&mut service, &mut socket, &uri).await;
        let close_diagnostics = diagnostics_notifications(&requests, &uri);
        let cleared = close_diagnostics
            .last()
            .expect("diagnostics clear on close");
        assert!(cleared.diagnostics.is_empty());

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(96)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 8 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(language_string.value, "[Tyda] Integer");
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_reopen_after_close_uses_reopened_source() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("sample.rb");
        let uri = Url::from_file_path(&path).expect("file uri");
        let source_on_disk = "def foo(x)\n  x\nend\n\nfoo(1)\n";
        let source_first_open = "def foo(x)\n  x\nend\n\nfoo(1.0)\n";
        let source_reopened = "def foo(x)\n  x\nend\n\nfoo(\"x\")\n";
        std::fs::write(&path, source_on_disk).expect("write ruby");

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source_first_open).await;
        let _ = close_document(&mut service, &mut socket, &uri).await;

        let requests = open_document(&mut service, &mut socket, &uri, source_reopened).await;
        assert_has_code_lens_refresh(&requests);

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(99)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 8 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(language_string.value, "[Tyda] String");
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_open_document_overrides_pre_scanned_workspace_cache() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let path = root.join("sample.rb");
        let uri = Url::from_file_path(&path).expect("file uri");
        let source_on_disk = "def foo(x)\n  x\nend\n\nfoo(1)\n";
        let source_in_editor = "def foo(x)\n  x\nend\n\nfoo(1.0)\n";
        std::fs::write(&path, source_on_disk).expect("write ruby");

        let root_uri = Url::from_directory_path(root).expect("root uri");
        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(100)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 8 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(language_string.value, "[Tyda] Integer");
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }

        let requests = open_document(&mut service, &mut socket, &uri, source_in_editor).await;
        assert!(
            requests
                .iter()
                .any(|request| request.method() == "workspace/codeLens/refresh")
        );

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(101)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": 0, "character": 8 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("decode hover");
        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(language_string.value, "[Tyda] Float");
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_create_signature_refreshes_and_hides_codelens() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source = "class Sample\n  def foo\n    1\n  end\nend\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let requests = open_document(&mut service, &mut socket, &uri, source).await;
        assert_has_code_lens_refresh(&requests);

        let initial_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(3)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("codeLens request")
        .expect("codeLens response");
        let initial_lenses: Vec<CodeLens> =
            serde_json::from_value(initial_lenses.result().cloned().expect("codeLens result"))
                .expect("decode codeLens");
        assert_eq!(initial_lenses.len(), 1);

        let (execute_response, requests) = drive_request(
            &mut service,
            &mut socket,
            Request::build("workspace/executeCommand")
                .id(4)
                .params(serde_json::json!({
                    "command": "typeprof.createSignature",
                    "arguments": [uri.as_str(), 1, "-> Integer"]
                }))
                .finish(),
        )
        .await;
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method(), "workspace/applyEdit");
        assert_eq!(requests[1].method(), "workspace/codeLens/refresh");
        let execute_response = execute_response.expect("execute response");
        assert!(execute_response.is_ok());

        let updated_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(5)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("updated codeLens request")
        .expect("updated codeLens response");
        let updated_lenses: Vec<CodeLens> =
            serde_json::from_value(updated_lenses.result().cloned().expect("codeLens result"))
                .expect("decode updated codeLens");
        assert!(updated_lenses.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_create_signature_uses_latest_method_position_after_incremental_shift() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source = "class Sample\n  def foo\n    1\n  end\nend\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let initial_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(130)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("codeLens request")
        .expect("codeLens response");
        let initial_lenses: Vec<CodeLens> =
            serde_json::from_value(initial_lenses.result().cloned().expect("codeLens result"))
                .expect("decode codeLens");
        assert_eq!(initial_lenses.len(), 1);
        let initial_command = initial_lenses[0].command.clone().expect("command");

        let _ = change_document_raw(
            &mut service,
            &mut socket,
            &uri,
            2,
            vec![serde_json::json!({
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 }
                },
                "text": "\n"
            })],
        )
        .await;

        let (execute_response, requests) = drive_request(
            &mut service,
            &mut socket,
            Request::build("workspace/executeCommand")
                .id(131)
                .params(serde_json::json!({
                    "command": initial_command.command,
                    "arguments": initial_command.arguments
                }))
                .finish(),
        )
        .await;
        let execute_response = execute_response.expect("execute response");
        assert!(execute_response.is_ok());
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method(), "workspace/applyEdit");
        assert_eq!(requests[1].method(), "workspace/codeLens/refresh");
        let edit_params = requests[0].params().expect("applyEdit params");
        let edit_json = serde_json::to_value(edit_params).expect("params json");
        let edits = edit_json["edit"]["changes"][uri.as_str()]
            .as_array()
            .expect("uri edits");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0]["range"]["start"]["line"], serde_json::json!(2));

        let updated_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(132)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("updated codeLens request")
        .expect("updated codeLens response");
        let updated_lenses: Vec<CodeLens> =
            serde_json::from_value(updated_lenses.result().cloned().expect("codeLens result"))
                .expect("decode updated codeLens");
        assert!(updated_lenses.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_codelens_reappears_after_incremental_annotation_delete() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("sample.rb")).expect("file uri");
        let source = "class Sample\n  #: -> Integer\n  def foo\n    1\n  end\nend\n";

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let initial_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(133)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("initial codeLens request")
        .expect("initial codeLens response");
        let initial_lenses: Vec<CodeLens> =
            serde_json::from_value(initial_lenses.result().cloned().expect("codeLens result"))
                .expect("decode initial codeLens");
        assert!(initial_lenses.is_empty());

        let requests = change_document_raw(
            &mut service,
            &mut socket,
            &uri,
            2,
            vec![serde_json::json!({
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 2, "character": 0 }
                },
                "text": ""
            })],
        )
        .await;
        assert_has_code_lens_refresh(&requests);

        let updated_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(134)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("updated codeLens request")
        .expect("updated codeLens response");
        let updated_lenses: Vec<CodeLens> =
            serde_json::from_value(updated_lenses.result().cloned().expect("codeLens result"))
                .expect("decode updated codeLens");
        assert_eq!(updated_lenses.len(), 1);
        assert_eq!(updated_lenses[0].range.start, Position::new(1, 6));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_prefers_workspace_fallback_for_definition_signature() {
        let dir = tempdir().expect("tempdir");
        let shared_uri =
            Url::from_file_path(dir.path().join("shared.rb")).expect("shared file uri");
        let consumer_uri = Url::from_file_path(dir.path().join("a.rb")).expect("consumer file uri");
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &shared_uri, shared_source).await;
        let _ = open_document(&mut service, &mut socket, &consumer_uri, consumer_source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(20)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri },
                    "position": { "line": 1, "character": 6 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("hover decode");

        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.language, "rbs");
                assert_eq!(
                    language_string.value,
                    "[Tyda] (String x, ?y: untyped, ?z: bool) -> String"
                );
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_codelens_hides_annotated_methods_and_keeps_inferred_one() {
        let dir = tempdir().expect("tempdir");
        let uri = Url::from_file_path(dir.path().join("a.rb")).expect("file uri");
        let source = concat!(
            "class A\n",
            "  def foo(x)\n",
            "    1\n",
            "  end\n",
            "\n",
            "  #: (Integer) -> Integer\n",
            "  def bar(x)\n",
            "    1\n",
            "  end\n",
            "end\n",
            "\n",
            "A.new.foo(1.0)\n",
            "A.new.bar(1)\n",
        );

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &uri, source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(90)
                .params(serde_json::json!({
                    "textDocument": { "uri": uri }
                }))
                .finish(),
        )
        .await
        .expect("codeLens request")
        .expect("codeLens response");
        let lenses: Vec<CodeLens> =
            serde_json::from_value(response.result().cloned().expect("codeLens result"))
                .expect("decode codeLens");

        assert_eq!(lenses.len(), 1);
        let command = lenses[0].command.as_ref().expect("command");
        assert_eq!(command.title, "#: (Float) -> 1");
    }

    #[test]
    fn lsp_hover_prefers_workspace_fallback_for_call_site_signature() {
        let dir = tempdir().expect("tempdir");
        let shared_path = dir.path().join("shared.rb");
        let consumer_path = dir.path().join("a.rb");
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let shared_analysis = crate::analysis::analyze_cached_file_with_deps(
            shared_source,
            None,
            Some(&loader),
            Some(&shared_path.to_string_lossy()),
            AnalysisOptions::default(),
        )
        .0;
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some(&consumer_path.to_string_lossy()),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(
            &mut state,
            shared_path.to_string_lossy().as_ref(),
            shared_analysis,
        );
        insert_test_cache(
            &mut state,
            consumer_path.to_string_lossy().as_ref(),
            consumer_analysis.clone(),
        );

        let workspace_registry =
            build_hover_workspace_registry(&mut state, &consumer_path.to_string_lossy());
        let hover = consumer_analysis
            .hover_at(
                consumer_source,
                line_col_to_offset(consumer_source.as_bytes(), 4, 4)
                    .expect("call-site byte offset"),
                &loader,
                Some(&workspace_registry),
            )
            .expect("call-site hover");
        assert_eq!(
            format_hover_body(&hover),
            "[Tyda] (String x, ?y: untyped, ?z: bool) -> untyped"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_limits_targets_to_analysis_unit_dirs() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let sample_dir = root.join("app");
        let other_dir = root.join("other");
        std::fs::create_dir_all(&sample_dir).expect("sample dir");
        std::fs::create_dir_all(&other_dir).expect("other dir");
        std::fs::write(
            root.join("typeprof.conf.jsonc"),
            "{\n  \"analysis_unit_dirs\": [\"app\"]\n}\n",
        )
        .expect("write config");

        let sample_uri =
            Url::from_file_path(sample_dir.join("sample.rb")).expect("sample file uri");
        let other_uri = Url::from_file_path(other_dir.join("other.rb")).expect("other file uri");
        let sample_source = "class Sample\n  def foo\n    1\n  end\nend\n";
        let other_source = "class Other\n  def bar\n    2\n  end\nend\n";
        std::fs::write(sample_dir.join("sample.rb"), sample_source).expect("write sample");
        std::fs::write(other_dir.join("other.rb"), other_source).expect("write other");

        let root_uri = Url::from_directory_path(root).expect("root uri");
        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;

        let _ = open_document(&mut service, &mut socket, &other_uri, other_source).await;

        let sample_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(6)
                .params(serde_json::json!({
                    "textDocument": { "uri": sample_uri }
                }))
                .finish(),
        )
        .await
        .expect("sample codeLens request")
        .expect("sample codeLens response");
        let sample_lenses: Vec<CodeLens> = serde_json::from_value(
            sample_lenses
                .result()
                .cloned()
                .expect("sample codeLens result"),
        )
        .expect("decode sample codeLens");
        assert_eq!(sample_lenses.len(), 1);

        let other_lenses = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(7)
                .params(serde_json::json!({
                    "textDocument": { "uri": other_uri }
                }))
                .finish(),
        )
        .await
        .expect("other codeLens request")
        .expect("other codeLens response");
        let other_lenses: Vec<CodeLens> = serde_json::from_value(
            other_lenses
                .result()
                .cloned()
                .expect("other codeLens result"),
        )
        .expect("decode other codeLens");
        assert!(other_lenses.is_empty());

        let other_hover = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(8)
                .params(serde_json::json!({
                    "textDocument": { "uri": other_uri },
                    "position": { "line": 1, "character": 6 }
                }))
                .finish(),
        )
        .await
        .expect("other hover request")
        .expect("other hover response");
        assert_eq!(other_hover.result(), Some(&serde_json::Value::Null));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_definition_works_with_scanned_workspace_file() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let app_models = root.join("app/models");
        let concerns_dir = app_models.join("concerns");
        std::fs::create_dir_all(&concerns_dir).expect("concerns dir");
        std::fs::write(
            root.join("typeprof.conf.jsonc"),
            "{\n  \"analysis_unit_dirs\": [\"app\"]\n}\n",
        )
        .expect("write config");

        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(concerns_dir.join("shared.rb"), shared_source).expect("write shared");
        std::fs::write(app_models.join("a.rb"), consumer_source).expect("write consumer");

        let root_uri = Url::from_directory_path(root).expect("root uri");
        let shared_uri = Url::from_file_path(concerns_dir.join("shared.rb")).expect("shared uri");
        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &shared_uri, shared_source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(30)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri },
                    "position": { "line": 1, "character": 6 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("hover decode");

        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(
                    language_string.value,
                    "[Tyda] (String x, ?y: untyped, ?z: bool) -> String"
                );
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_codelens_prefers_workspace_fallback_for_definition_signature() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let app_models = root.join("app/models");
        let concerns_dir = app_models.join("concerns");
        std::fs::create_dir_all(&concerns_dir).expect("concerns dir");
        std::fs::write(
            root.join("typeprof.conf.jsonc"),
            "{\n  \"analysis_unit_dirs\": [\"app\"]\n}\n",
        )
        .expect("write config");

        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x, y: 15.minutes, z: true)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(concerns_dir.join("shared.rb"), shared_source).expect("write shared");
        std::fs::write(app_models.join("a.rb"), consumer_source).expect("write consumer");

        let root_uri = Url::from_directory_path(root).expect("root uri");
        let shared_uri = Url::from_file_path(concerns_dir.join("shared.rb")).expect("shared uri");
        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &shared_uri, shared_source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(31)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri }
                }))
                .finish(),
        )
        .await
        .expect("codeLens request")
        .expect("codeLens response");
        let lenses: Vec<CodeLens> =
            serde_json::from_value(response.result().cloned().expect("codeLens result"))
                .expect("decode codeLens");
        assert_eq!(lenses.len(), 1);
        let command = lenses[0].command.as_ref().expect("command");
        assert_eq!(
            command.title,
            "#: (String, ?y: untyped, ?z: bool) -> String"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_definition_keyword_params_use_defaults_and_callsites() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let concerns_dir = root.join("app/models/concerns");
        std::fs::create_dir_all(&concerns_dir).expect("concerns dir");
        let root_uri = Url::from_directory_path(root).expect("root uri");
        let shared_path = concerns_dir.join("shared.rb");
        let shared_source = concat!(
            "module Shared\n",
            "  #: (String x, ?Integer y, ?bool z) -> String\n",
            "  def foo(x, y: 1, z: true)\n",
            "    \"#{x}-#{y}\"\n",
            "    if z\n",
            "      x\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&shared_path, shared_source).expect("write shared");
        let shared_uri = Url::from_file_path(&shared_path).expect("shared uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &shared_uri, shared_source).await;

        let y_response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(32)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri },
                    "position": { "line": 2, "character": 13 }
                }))
                .finish(),
        )
        .await
        .expect("y hover request")
        .expect("y hover response");
        let y_hover: Hover =
            serde_json::from_value(y_response.result().cloned().expect("y hover result"))
                .expect("decode y hover");

        match y_hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.value, "[Tyda] Integer");
            }
            other => panic!("unexpected y hover contents: {other:?}"),
        }

        let z_response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(33)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri },
                    "position": { "line": 2, "character": 19 }
                }))
                .finish(),
        )
        .await
        .expect("z hover request")
        .expect("z hover response");
        let z_hover: Hover =
            serde_json::from_value(z_response.result().cloned().expect("z hover result"))
                .expect("decode z hover");

        match z_hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.value, "[Tyda] bool");
            }
            other => panic!("unexpected z hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_definition_default_values_keep_own_types() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let concerns_dir = root.join("app/models/concerns");
        std::fs::create_dir_all(&concerns_dir).expect("concerns dir");
        let root_uri = Url::from_directory_path(root).expect("root uri");
        let shared_path = concerns_dir.join("shared.rb");
        let shared_source = concat!(
            "module Shared\n",
            "  #: (String x, ?Integer y, ?bool z) -> String\n",
            "  def foo(x, y: 1, z: true)\n",
            "    \"#{x}-#{y}\"\n",
            "    if z\n",
            "      x\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&shared_path, shared_source).expect("write shared");
        let shared_uri = Url::from_file_path(&shared_path).expect("shared uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &shared_uri, shared_source).await;

        let y_default_response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(34)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri },
                    "position": { "line": 2, "character": 16 }
                }))
                .finish(),
        )
        .await
        .expect("y default hover request")
        .expect("y default hover response");
        let y_default_hover: Hover = serde_json::from_value(
            y_default_response
                .result()
                .cloned()
                .expect("y default hover result"),
        )
        .expect("decode y default hover");

        match y_default_hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.value, "[Tyda] 1");
            }
            other => panic!("unexpected y default hover contents: {other:?}"),
        }

        let z_default_response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(35)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri },
                    "position": { "line": 2, "character": 22 }
                }))
                .finish(),
        )
        .await
        .expect("z default hover request")
        .expect("z default hover response");
        let z_default_hover: Hover = serde_json::from_value(
            z_default_response
                .result()
                .cloned()
                .expect("z default hover result"),
        )
        .expect("decode z default hover");

        match z_default_hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.value, "[Tyda] true");
            }
            other => panic!("unexpected z default hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_returns_token_fallback_for_shared_call_arguments() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let concerns_dir = root.join("app/models/concerns");
        std::fs::create_dir_all(&concerns_dir).expect("concerns dir");
        let root_uri = Url::from_directory_path(root).expect("root uri");
        let shared_path = concerns_dir.join("shared.rb");
        let shared_source = concat!(
            "module Shared\n",
            "  def foo\n",
            "    x = 1.to_s\n",
            "    y = 1\n",
            "    \"#{x}-#{y}\"\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&shared_path, shared_source).expect("write shared");
        let shared_uri = Url::from_file_path(&shared_path).expect("shared uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &shared_uri, shared_source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(36)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri },
                    "position": { "line": 4, "character": 12 }
                }))
                .finish(),
        )
        .await
        .expect("arg hover request")
        .expect("arg hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("arg hover result"))
                .expect("decode arg hover");

        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.value, "[Tyda] 1");
            }
            other => panic!("unexpected arg hover contents: {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_resolves_shared_param_types_inside_body_tokens() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let concerns_dir = root.join("app/models/concerns");
        std::fs::create_dir_all(&concerns_dir).expect("concerns dir");
        let root_uri = Url::from_directory_path(root).expect("root uri");
        let shared_path = concerns_dir.join("shared.rb");
        let shared_source = concat!(
            "module Shared\n",
            "  def foo\n",
            "    x = 1.to_s\n",
            "    y = 1\n",
            "    \"#{x}-#{y}\"\n",
            "  end\n",
            "end\n",
        );
        std::fs::write(&shared_path, shared_source).expect("write shared");
        let shared_uri = Url::from_file_path(&shared_path).expect("shared uri");

        let (mut service, mut socket) = initialize_lsp(Some(root_uri)).await;
        let _ = open_document(&mut service, &mut socket, &shared_uri, shared_source).await;

        for (id, line, character, expected) in
            [(37, 4, 7, "[Tyda] String"), (38, 4, 12, "[Tyda] 1")]
        {
            let response = Service::call(
                &mut service,
                Request::build("textDocument/hover")
                    .id(id)
                    .params(serde_json::json!({
                        "textDocument": { "uri": shared_uri },
                        "position": { "line": line, "character": character }
                    }))
                    .finish(),
            )
            .await
            .expect("interpolation hover request")
            .expect("interpolation hover response");
            let hover: Hover = serde_json::from_value(
                response
                    .result()
                    .cloned()
                    .expect("interpolation hover result"),
            )
            .expect("decode interpolation hover");

            match hover.contents {
                HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                    assert_eq!(language_string.value, expected);
                }
                other => panic!("unexpected interpolation hover contents: {other:?}"),
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn lsp_hover_definition_optional_param_uses_workspace_union() {
        let dir = tempdir().expect("tempdir");
        let shared_uri =
            Url::from_file_path(dir.path().join("shared.rb")).expect("shared file uri");
        let consumer_uri = Url::from_file_path(dir.path().join("a.rb")).expect("consumer file uri");
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x = 1)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );

        let (mut service, mut socket) = initialize_lsp(None).await;
        let _ = open_document(&mut service, &mut socket, &shared_uri, shared_source).await;
        let _ = open_document(&mut service, &mut socket, &consumer_uri, consumer_source).await;

        let response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(40)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri },
                    "position": { "line": 1, "character": 10 }
                }))
                .finish(),
        )
        .await
        .expect("hover request")
        .expect("hover response");
        let hover: Hover =
            serde_json::from_value(response.result().cloned().expect("hover result"))
                .expect("hover decode");

        match hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert_eq!(language_string.value, "[Tyda] Integer | String");
            }
            other => panic!("unexpected hover contents: {other:?}"),
        }

        let method_response = Service::call(
            &mut service,
            Request::build("textDocument/hover")
                .id(41)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri },
                    "position": { "line": 1, "character": 6 }
                }))
                .finish(),
        )
        .await
        .expect("method hover request")
        .expect("method hover response");
        let method_hover: Hover = serde_json::from_value(
            method_response
                .result()
                .cloned()
                .expect("method hover result"),
        )
        .expect("method hover decode");

        match method_hover.contents {
            HoverContents::Scalar(MarkedString::LanguageString(language_string)) => {
                assert!(
                    language_string.value.contains("Integer | String")
                        || language_string.value.contains("String | Integer"),
                    "{}",
                    language_string.value
                );
            }
            other => panic!("unexpected method hover contents: {other:?}"),
        }

        let lens_response = Service::call(
            &mut service,
            Request::build("textDocument/codeLens")
                .id(42)
                .params(serde_json::json!({
                    "textDocument": { "uri": shared_uri }
                }))
                .finish(),
        )
        .await
        .expect("codeLens request")
        .expect("codeLens response");
        let lenses: Vec<CodeLens> =
            serde_json::from_value(lens_response.result().cloned().expect("codeLens result"))
                .expect("decode codeLens");
        assert_eq!(lenses.len(), 1);
        let command = lenses[0].command.as_ref().expect("command");
        assert!(
            command.title.contains("Integer | String")
                || command.title.contains("String | Integer")
        );
    }

    #[test]
    fn linked_codelens_signature_matches_hover_for_workspace_enriched_definition() {
        let dir = tempdir().expect("tempdir");
        let shared_path = dir.path().join("shared.rb");
        let consumer_path = dir.path().join("a.rb");
        let loader = stdlib_loader();
        let shared_source = concat!(
            "module Shared\n",
            "  def foo(x = 1)\n",
            "    x\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class A\n",
            "  include Shared\n",
            "  def bar\n",
            "    foo(\"x\")\n",
            "  end\n",
            "end\n",
        );
        let shared_analysis = crate::analysis::analyze_cached_file_with_deps(
            shared_source,
            None,
            Some(&loader),
            Some(&shared_path.to_string_lossy()),
            AnalysisOptions::default(),
        )
        .0;
        let consumer_analysis = crate::analysis::analyze_cached_file_with_deps(
            consumer_source,
            None,
            Some(&loader),
            Some(&consumer_path.to_string_lossy()),
            AnalysisOptions::default(),
        )
        .0;
        let mut state = LspState {
            documents: HashMap::new(),
            document_cache_updates_in_progress: HashMap::new(),
            stdlib_loader: stdlib_loader(),
            user_rbs: Arc::new(TypeRegistry::new()),
            workspace_root: None,
            analysis_unit_roots: None,
            signature_enabled: true,
            output_parameter_names: true,
            rails_mode: false,
            dsl_activation: DslActivation::default(),
            project_versions: ProjectVersions::default(),
            lazy_rbi_loader: None,
            type_file_classes: HashMap::new(),
            workspace_state: WorkspaceState::new(),
            workspace_scanned: false,
            workspace_scan_in_progress: false,
            workspace_scan_generation: 0,
            workspace_fully_discovered: false,
            type_env_generation: 0,
            cached_display_registry: Default::default(),
            cached_display: Default::default(),
        };
        insert_test_cache(
            &mut state,
            shared_path.to_string_lossy().as_ref(),
            shared_analysis.clone(),
        );
        insert_test_cache(
            &mut state,
            consumer_path.to_string_lossy().as_ref(),
            consumer_analysis,
        );

        let workspace_registry =
            build_hover_workspace_registry(&mut state, &shared_path.to_string_lossy());
        let (_class_name, method) = shared_analysis
            .methods_for_file(&shared_path.to_string_lossy())
            .into_iter()
            .find(|(_class_name, method)| method.name == "foo")
            .expect("shared method");
        let linked = resolve_code_lens_method_sig(
            &shared_analysis,
            shared_source,
            &method,
            &loader,
            &workspace_registry,
        )
        .expect("linked code lens signature");
        let hover = shared_analysis
            .hover_at(
                shared_source,
                method_definition_name_offset(shared_source, &method)
                    .expect("method definition offset"),
                &loader,
                Some(&workspace_registry),
            )
            .expect("method definition hover");
        let linked_display = crate::rbs::display::format_hover_callable_type(&linked);

        assert_eq!(hover.display_rbs.as_deref(), Some(linked_display.as_str()));
    }

    #[test]
    fn bench_display_analysis_mastodon_scale() {
        let subject_root = std::env::var_os("TYDA_LSP_BENCH_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("subject/gitlab/app")
            });
        if !subject_root.exists() {
            eprintln!(
                "skipping: lsp benchmark subject not found: {}",
                subject_root.display()
            );
            return;
        }
        let _bench_guard = mastodon_bench_guard();
        let loader = stdlib_loader();
        let project_root = subject_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| *name == "app")
            .and_then(|_| subject_root.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| subject_root.clone());
        let options = AnalysisOptions {
            project_root: Some(project_root),
            ..AnalysisOptions::default()
        };
        let user_rbs = TypeRegistry::new();

        let rb_files = crate::workspace_discovery::collect_rb_files_from_roots(
            std::slice::from_ref(&subject_root),
        );

        let t_scan = std::time::Instant::now();
        let mut state = new_test_state();
        for_each_workspace_scan_result(
            &rb_files,
            WorkspaceScanInputs {
                cached_entries: &HashMap::new(),
                open_docs: &HashMap::new(),
                user_rbs: &user_rbs,
                stdlib_loader: &loader,
                lazy_rbi_loader: None,
                options: &options,
            },
            || false,
            |result| {
                if let WorkspaceScanResult::Analyzed {
                    file_path,
                    content_hash,
                    analysis,
                    fingerprints,
                    file_deps,
                    on_disk_stamp,
                } = result
                {
                    state
                        .workspace_state
                        .upsert_scanned_file_with_stamp_and_fingerprints(
                            file_path,
                            content_hash,
                            analysis,
                            file_deps,
                            on_disk_stamp,
                            fingerprints,
                        );
                }
            },
        );
        state.workspace_state.warm_display_base_registry(&user_rbs);
        let scan_ms = t_scan.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "[bench] workspace scan: {:.0}ms ({} files)",
            scan_ms,
            rb_files.len()
        );

        let target_file = choose_scan_benchmark_target(&rb_files).expect("benchmark target file");
        let target_file = target_file.to_string_lossy().to_string();
        let target_source = std::fs::read_to_string(&target_file).expect("benchmark target source");

        let t_display = std::time::Instant::now();
        let (_analysis, _workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, &target_file, &target_source);
        let display_ms = t_display.elapsed().as_secs_f64() * 1000.0;
        eprintln!("[bench] first display analysis: {:.0}ms", display_ms);

        let t_cached = std::time::Instant::now();
        let (_analysis2, _workspace_registry2) =
            TydaLsp::analyze_current_file_for_display(&mut state, &target_file, &target_source);
        let cached_ms = t_cached.elapsed().as_secs_f64() * 1000.0;
        eprintln!("[bench] cached display analysis: {:.0}ms", cached_ms);

        let unrelated_ms = if let Some(unrelated_file) = rb_files
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .find(|path| {
                path != &target_file
                    && !state
                        .workspace_state
                        .display_scope_includes_file(&target_file, path)
            }) {
            let unrelated_source =
                std::fs::read_to_string(&unrelated_file).expect("benchmark unrelated source");
            let updated_unrelated_source =
                format!("{unrelated_source}\n# bench unrelated update\n");
            let user_rbs = Arc::clone(&state.user_rbs);
            let stdlib_loader = Arc::clone(&state.stdlib_loader);
            let lazy_rbi_loader = state.lazy_rbi_loader.clone();
            let options = TydaLsp::build_analysis_options(&state);
            let (analysis, file_deps) = analyze_file_facts_with_deps_and_rbi(
                &updated_unrelated_source,
                Some(user_rbs.as_ref()),
                Some(&stdlib_loader),
                lazy_rbi_loader.as_deref(),
                Some(&unrelated_file),
                options,
            );
            state.workspace_state.upsert_file(
                unrelated_file,
                crate::workspace_state::hash_content(&updated_unrelated_source),
                analysis,
                file_deps,
            );
            let t_unrelated = std::time::Instant::now();
            let (_analysis3, _workspace_registry3) =
                TydaLsp::analyze_current_file_for_display(&mut state, &target_file, &target_source);
            t_unrelated.elapsed().as_secs_f64() * 1000.0
        } else {
            0.0
        };
        eprintln!(
            "[bench] unrelated dirty display analysis: {:.0}ms",
            unrelated_ms
        );

        let updated_target_source = format!("{target_source}\n# bench dirty display\n");
        let t_dirty = std::time::Instant::now();
        let (_analysis4, _workspace_registry4) = TydaLsp::analyze_current_file_for_display(
            &mut state,
            &target_file,
            &updated_target_source,
        );
        let dirty_ms = t_dirty.elapsed().as_secs_f64() * 1000.0;
        eprintln!("[bench] dirty display analysis: {:.0}ms", dirty_ms);

        let running_on_ci = std::env::var_os("GITHUB_ACTIONS").is_some();
        let scan_max_ms = if cfg!(debug_assertions) {
            10000.0
        } else {
            1500.0
        };
        let max_ms = if running_on_ci { 3000.0 } else { 1000.0 };
        if std::env::var_os("TYDA_STRICT_PERF_TESTS").is_some() {
            assert!(
                scan_ms < scan_max_ms,
                "workspace scan took {scan_ms:.0}ms, expected < {scan_max_ms:.0}ms"
            );
            assert!(
                cached_ms < max_ms,
                "cached display analysis took {cached_ms:.0}ms, expected < {max_ms:.0}ms"
            );
            assert!(
                unrelated_ms < max_ms,
                "unrelated dirty display analysis took {unrelated_ms:.0}ms, expected < {max_ms:.0}ms"
            );
        } else {
            if scan_ms >= scan_max_ms {
                eprintln!(
                    "workspace scan exceeded report-only threshold: expected < {scan_max_ms:.0}ms, got {scan_ms:.0}ms"
                );
            }
            if cached_ms >= max_ms {
                eprintln!(
                    "cached display analysis exceeded report-only threshold: expected < {max_ms:.0}ms, got {cached_ms:.0}ms"
                );
            }
            if unrelated_ms >= max_ms {
                eprintln!(
                    "unrelated dirty display analysis exceeded report-only threshold: expected < {max_ms:.0}ms, got {unrelated_ms:.0}ms"
                );
            }
        }

        let default_skip = state
            .workspace_state
            .display_can_skip_workspace_context(&target_file);
        let default_basename = std::path::Path::new(&target_file)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&target_file);
        eprintln!("[probe] default_target={default_basename} skip_workspace={default_skip}");

        if let Some((file_a, file_b)) = choose_display_probe_files(&rb_files, &state) {
            let source_a = std::fs::read_to_string(&file_a).expect("probe file A source");
            let source_b = std::fs::read_to_string(&file_b).expect("probe file B source");
            clear_display_caches(&mut state);
            state.workspace_state.warm_display_base_registry(&user_rbs);
            probe_display_registry_build(&mut state, "file_a_cold", &file_a, &source_a);
            probe_display_registry_build(&mut state, "file_b_tab_switch", &file_b, &source_b);
            probe_display_registry_build(&mut state, "file_a_return", &file_a, &source_a);
            eprintln!(
                "[probe] m1_cached_registry_after_display={}",
                state.workspace_state.has_cached_registry()
            );
        }
    }

    #[test]
    fn bench_initialize_analysis_mastodon_scale() {
        let subject_root = std::env::var_os("TYDA_LSP_BENCH_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("subject/gitlab/app")
            });
        if !subject_root.exists() {
            eprintln!(
                "skipping: lsp initialize benchmark subject not found: {}",
                subject_root.display()
            );
            return;
        }
        let _bench_guard = mastodon_bench_guard();

        let project_root = subject_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| *name == "app")
            .and_then(|_| subject_root.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| subject_root.clone());
        let project_versions = ProjectVersions::detect(&project_root);
        let vendor_rbs_root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs");
        let stdlib_loader = Arc::new(LazyRbsLoader::for_ruby_version(
            vendor_rbs_root,
            project_versions.effective_ruby(),
        ));

        let initialize_started = std::time::Instant::now();
        let type_env_started = std::time::Instant::now();
        let loaded = load_workspace_type_environment(&project_root, &stdlib_loader);
        let type_env_ms = type_env_started.elapsed().as_secs_f64() * 1000.0;
        eprintln!("[bench] type env load: {:.0}ms", type_env_ms);

        let mut user_rbs = loaded.user_rbs;
        let lazy_rbi_loader = loaded.lazy_rbi_loader.map(Arc::new);
        let t_dsl = std::time::Instant::now();
        let mut dsl_activation = detect_dsl_activation(&project_root);
        if let Some(config) = load_typeprof_config(&project_root).ok().flatten()
            && let Some(tokens) = config.dsl
        {
            dsl_activation.apply_cli_spec(&tokens.join(","));
        }
        eprintln!(
            "[bench] dsl detect: {:.0}ms",
            t_dsl.elapsed().as_secs_f64() * 1000.0
        );
        let t_rails = std::time::Instant::now();
        let rails_mode = if dsl_activation.rails_mode_compat() {
            crate::rails::load_project_types_with_activation(
                &project_root,
                &mut user_rbs,
                &dsl_activation,
            )
        } else {
            false
        };
        eprintln!(
            "[bench] rails load: {:.0}ms",
            t_rails.elapsed().as_secs_f64() * 1000.0
        );
        user_rbs.shrink_to_fit_after_compact();
        let initialize_ms = initialize_started.elapsed().as_secs_f64() * 1000.0;
        eprintln!("[bench] initialize setup: {:.0}ms", initialize_ms);

        let options = AnalysisOptions {
            project_root: Some(project_root.clone()),
            ..AnalysisOptions::default()
        };
        let rb_files = crate::workspace_discovery::collect_rb_files_from_roots(
            std::slice::from_ref(&subject_root),
        );

        let mut state = new_test_state();
        state.stdlib_loader = Arc::clone(&stdlib_loader);
        state.user_rbs = Arc::new(user_rbs);
        state.workspace_root = Some(project_root);
        state.analysis_unit_roots =
            scan_roots_from_typeprof_config(&state.workspace_root.clone().expect("workspace root"));
        state.project_versions = project_versions;
        state.dsl_activation = dsl_activation;
        state.rails_mode = rails_mode;
        state.lazy_rbi_loader = lazy_rbi_loader.clone();

        let t_scan = std::time::Instant::now();
        for_each_workspace_scan_result(
            &rb_files,
            WorkspaceScanInputs {
                cached_entries: &HashMap::new(),
                open_docs: &HashMap::new(),
                user_rbs: state.user_rbs.as_ref(),
                stdlib_loader: state.stdlib_loader.as_ref(),
                lazy_rbi_loader: state.lazy_rbi_loader.as_deref(),
                options: &options,
            },
            || false,
            |result| {
                if let WorkspaceScanResult::Analyzed {
                    file_path,
                    content_hash,
                    analysis,
                    fingerprints,
                    file_deps,
                    on_disk_stamp,
                } = result
                {
                    state
                        .workspace_state
                        .upsert_scanned_file_with_stamp_and_fingerprints(
                            file_path,
                            content_hash,
                            analysis,
                            file_deps,
                            on_disk_stamp,
                            fingerprints,
                        );
                }
            },
        );
        let scan_ms = t_scan.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "[bench] workspace scan: {:.0}ms ({} files)",
            scan_ms,
            rb_files.len()
        );

        let target_file = choose_scan_benchmark_target(&rb_files).expect("benchmark target file");
        let target_file = target_file.to_string_lossy().to_string();
        let target_source = std::fs::read_to_string(&target_file).expect("benchmark target source");

        let t_display = std::time::Instant::now();
        let (_analysis, _workspace_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, &target_file, &target_source);
        let display_ms = t_display.elapsed().as_secs_f64() * 1000.0;
        eprintln!("[bench] first display analysis: {:.0}ms", display_ms);

        let total_ms = initialize_started.elapsed().as_secs_f64() * 1000.0;
        eprintln!("[bench] initialize total: {:.0}ms", total_ms);

        let running_on_ci = std::env::var_os("GITHUB_ACTIONS").is_some();
        let max_ms = if running_on_ci { 30000.0 } else { 20000.0 };
        assert!(
            total_ms < max_ms,
            "initialize benchmark took {total_ms:.0}ms, expected < {max_ms:.0}ms"
        );
    }

    /// Unix max RSS (for `bench_memory_breakdown`; a monotonic max — for the live value use `current_resident_size_bytes`).
    #[cfg(all(test, unix))]
    fn current_max_rss_bytes() -> u64 {
        unsafe {
            let mut usage: libc::rusage = std::mem::zeroed();
            if libc::getrusage(libc::RUSAGE_SELF, &mut usage) != 0 {
                return 0;
            }
            // macOS: ru_maxrss is in bytes. Linux: in KB.
            #[cfg(target_os = "macos")]
            let bytes = usage.ru_maxrss as u64;
            #[cfg(not(target_os = "macos"))]
            let bytes = (usage.ru_maxrss as u64) * 1024;
            bytes
        }
    }

    #[cfg(all(test, not(unix)))]
    fn current_max_rss_bytes() -> u64 {
        0
    }

    /// Live RSS (unlike max, this drops after freeing — used for steady-state attribution).
    #[cfg(test)]
    fn current_resident_bytes() -> u64 {
        #[cfg(target_os = "macos")]
        unsafe {
            #[repr(C)]
            struct MachTaskBasicInfo {
                virtual_size: u64,
                resident_size: u64,
                resident_size_max: u64,
                user_time: [i32; 2],
                system_time: [i32; 2],
                policy: i32,
                suspend_count: i32,
            }
            unsafe extern "C" {
                static mach_task_self_: u32;
                fn task_info(
                    target_task: u32,
                    flavor: u32,
                    task_info_out: *mut i32,
                    task_info_count: *mut u32,
                ) -> i32;
            }
            const MACH_TASK_BASIC_INFO: u32 = 20;
            let mut info: MachTaskBasicInfo = std::mem::zeroed();
            let mut count =
                (std::mem::size_of::<MachTaskBasicInfo>() / std::mem::size_of::<u32>()) as u32;
            let kr = task_info(
                mach_task_self_,
                MACH_TASK_BASIC_INFO,
                &mut info as *mut _ as *mut i32,
                &mut count,
            );
            if kr == 0 { info.resident_size } else { 0 }
        }
        #[cfg(not(target_os = "macos"))]
        {
            0
        }
    }

    /// phys_footprint (equivalent to Activity Monitor's value; drops after an mimalloc purge — used to confirm memory was returned).
    #[cfg(test)]
    fn current_phys_footprint_bytes() -> u64 {
        #[cfg(target_os = "macos")]
        unsafe {
            // Truncates `task_vm_info` at `phys_footprint` (the kernel only copies up to the requested count).
            #[repr(C)]
            struct TaskVmInfoPrefix {
                virtual_size: u64,
                region_count: i32,
                page_size: i32,
                resident_size: u64,
                resident_size_peak: u64,
                device: u64,
                device_peak: u64,
                internal: u64,
                internal_peak: u64,
                external: u64,
                external_peak: u64,
                reusable: u64,
                reusable_peak: u64,
                purgeable_volatile_pmap: u64,
                purgeable_volatile_resident: u64,
                purgeable_volatile_virtual: u64,
                compressed: u64,
                compressed_peak: u64,
                compressed_lifetime: u64,
                phys_footprint: u64,
            }
            unsafe extern "C" {
                static mach_task_self_: u32;
                fn task_info(
                    target_task: u32,
                    flavor: u32,
                    task_info_out: *mut i32,
                    task_info_count: *mut u32,
                ) -> i32;
            }
            const TASK_VM_INFO: u32 = 22;
            let mut info: TaskVmInfoPrefix = std::mem::zeroed();
            let mut count =
                (std::mem::size_of::<TaskVmInfoPrefix>() / std::mem::size_of::<u32>()) as u32;
            let kr = task_info(
                mach_task_self_,
                TASK_VM_INFO,
                &mut info as *mut _ as *mut i32,
                &mut count,
            );
            if kr == 0 { info.phys_footprint } else { 0 }
        }
        #[cfg(not(target_os = "macos"))]
        {
            0
        }
    }

    /// Memory-breakdown bench: prints a per-holder breakdown for RSS attribution (use alongside `/usr/bin/time -l`).
    #[test]
    fn bench_memory_breakdown_mastodon_scale() {
        let subject_root = std::env::var_os("TYDA_LSP_BENCH_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("subject/gitlab/app")
            });
        if !subject_root.exists() {
            eprintln!(
                "skipping: memory breakdown bench subject not found: {}",
                subject_root.display()
            );
            return;
        }
        let _bench_guard = mastodon_bench_guard();

        let project_root = subject_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| *name == "app")
            .and_then(|_| subject_root.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| subject_root.clone());
        let project_versions = ProjectVersions::detect(&project_root);
        let vendor_rbs_root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs");
        let stdlib_loader = Arc::new(LazyRbsLoader::for_ruby_version(
            vendor_rbs_root,
            project_versions.effective_ruby(),
        ));

        // `TYDA_BENCH_DISABLE_RBI=1` toggles the RBI A/B comparison (mimalloc's delayed OS return makes live RSS unreliable here).
        let disable_rbi = std::env::var_os("TYDA_BENCH_DISABLE_RBI").is_some();

        eprintln!(
            "[rss] start: {} MB (rbi_disabled={})",
            current_max_rss_bytes() / 1_048_576,
            disable_rbi,
        );

        let loaded = load_workspace_type_environment(&project_root, &stdlib_loader);
        let mut user_rbs = loaded.user_rbs;
        let lazy_rbi_loader = if disable_rbi {
            None
        } else {
            loaded.lazy_rbi_loader.map(Arc::new)
        };
        eprintln!(
            "[rss] after type env load: {} MB",
            current_max_rss_bytes() / 1_048_576
        );
        let mut dsl_activation = detect_dsl_activation(&project_root);
        if let Some(config) = load_typeprof_config(&project_root).ok().flatten()
            && let Some(tokens) = config.dsl
        {
            dsl_activation.apply_cli_spec(&tokens.join(","));
        }
        let rails_mode = if dsl_activation.rails_mode_compat() {
            crate::rails::load_project_types_with_activation(
                &project_root,
                &mut user_rbs,
                &dsl_activation,
            )
        } else {
            false
        };
        user_rbs.shrink_to_fit_after_compact();
        eprintln!(
            "[rss] after rails framework load: {} MB",
            current_max_rss_bytes() / 1_048_576
        );

        let options = AnalysisOptions {
            project_root: Some(project_root.clone()),
            ..AnalysisOptions::default()
        };
        let rb_files = crate::workspace_discovery::collect_rb_files_from_roots(
            std::slice::from_ref(&subject_root),
        );

        let mut state = new_test_state();
        state.stdlib_loader = Arc::clone(&stdlib_loader);
        state.user_rbs = Arc::new(user_rbs);
        state.workspace_root = Some(project_root);
        state.analysis_unit_roots =
            scan_roots_from_typeprof_config(&state.workspace_root.clone().expect("workspace root"));
        state.project_versions = project_versions;
        state.dsl_activation = dsl_activation;
        state.rails_mode = rails_mode;
        state.lazy_rbi_loader = lazy_rbi_loader.clone();

        let scan_started = std::time::Instant::now();
        for_each_workspace_scan_result(
            &rb_files,
            WorkspaceScanInputs {
                cached_entries: &HashMap::new(),
                open_docs: &HashMap::new(),
                user_rbs: state.user_rbs.as_ref(),
                stdlib_loader: state.stdlib_loader.as_ref(),
                lazy_rbi_loader: state.lazy_rbi_loader.as_deref(),
                options: &options,
            },
            || false,
            |result| {
                if let WorkspaceScanResult::Analyzed {
                    file_path,
                    content_hash,
                    analysis,
                    fingerprints,
                    file_deps,
                    on_disk_stamp,
                } = result
                {
                    state
                        .workspace_state
                        .upsert_scanned_file_with_stamp_and_fingerprints(
                            file_path,
                            content_hash,
                            analysis,
                            file_deps,
                            on_disk_stamp,
                            fingerprints,
                        );
                }
            },
        );

        let scan_ms = scan_started.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "[rss] after workspace scan ({} ms): {} MB",
            scan_ms as u64,
            current_max_rss_bytes() / 1_048_576
        );

        crate::reclaim_freed_memory(Some(lsp_analysis_pool()));
        eprintln!(
            "[rss] after post-scan reclaim: max={} MB resident={} MB footprint={} MB",
            current_max_rss_bytes() / 1_048_576,
            current_resident_bytes() / 1_048_576,
            current_phys_footprint_bytes() / 1_048_576,
        );

        // Force the workspace registry to materialize so the breakdown captures
        // the merged long-lived holder, not just file-local snapshots.
        let workspace_registry = state.workspace_state.workspace_registry(&state.user_rbs);
        eprintln!(
            "[rss] after workspace_registry materialize: max={} MB resident={} MB footprint={} MB",
            current_max_rss_bytes() / 1_048_576,
            current_resident_bytes() / 1_048_576,
            current_phys_footprint_bytes() / 1_048_576,
        );
        // Mirror the production post-rebuild reclaim (`build_hover_workspace_registry`)
        // so the bench's steady-state numbers reflect what an editor session sees.
        crate::reclaim_freed_memory(Some(lsp_analysis_pool()));
        eprintln!(
            "[rss] after post-rebuild reclaim: resident={} MB footprint={} MB",
            current_resident_bytes() / 1_048_576,
            current_phys_footprint_bytes() / 1_048_576,
        );

        // A single display analysis warms the hover cache (this runs before the file cache is dropped, matching the production path).
        let target_file = choose_scan_benchmark_target(&rb_files).expect("benchmark target file");
        let target_file = target_file.to_string_lossy().to_string();
        let target_source = std::fs::read_to_string(&target_file).expect("benchmark target source");
        let display_started = std::time::Instant::now();
        let (_analysis, _ws_registry) =
            TydaLsp::analyze_current_file_for_display(&mut state, &target_file, &target_source);
        let display_ms = display_started.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "[rss] after display analysis ({} ms, cache warm): {} MB",
            display_ms as u64,
            current_max_rss_bytes() / 1_048_576
        );

        let user_rbs_totals = state.user_rbs.breakdown_totals();
        let workspace_totals = workspace_registry.breakdown_totals();
        let workspace_file_count = state.workspace_state.file_count();
        let stdlib_cache_warm = state.stdlib_loader.cache_breakdown();
        let rbi_cache_warm = state
            .lazy_rbi_loader
            .as_ref()
            .map(|loader| loader.cache_breakdown())
            .unwrap_or_default();

        eprintln!("[breakdown] rb files scanned: {}", rb_files.len());
        eprintln!(
            "[breakdown] workspace_state files: {}",
            workspace_file_count
        );
        eprintln!(
            "[breakdown] user_rbs (preloaded RBS + Rails project types): classes={}, methods={}, ivars={}, call_sites={}, mixins={}, name_pool={}, rbs_overloads={}",
            user_rbs_totals.class_count,
            user_rbs_totals.method_count,
            user_rbs_totals.ivar_count + user_rbs_totals.singleton_ivar_count,
            user_rbs_totals.call_site_count,
            user_rbs_totals.mixin_count,
            user_rbs_totals.name_pool_count,
            user_rbs_totals.rbs_overload_count,
        );
        eprintln!(
            "[breakdown] workspace_registry (merged): classes={}, methods={}, ivars={}, call_sites={}, mixins={}, name_pool={}, rbs_overloads={}",
            workspace_totals.class_count,
            workspace_totals.method_count,
            workspace_totals.ivar_count + workspace_totals.singleton_ivar_count,
            workspace_totals.call_site_count,
            workspace_totals.mixin_count,
            workspace_totals.name_pool_count,
            workspace_totals.rbs_overload_count,
        );
        // Byte attribution: the merged registry is counted first (charging shared method bodies up front), then display/snapshot/user_rbs.
        {
            let mut seen = rustc_hash::FxHashSet::default();
            let mb = |bytes: usize| bytes as f64 / 1_048_576.0;
            let report = |label: &str, d: &crate::registry::RegistryDeepBytes| {
                eprintln!(
                    "[deep] {label}: total={:.1}MB bodies={:.1}MB (new={} shared_prior={}) call_sites={:.1}MB (n={}) containers={:.1}MB const_ivar={:.1}MB",
                    mb(d.total_bytes),
                    mb(d.method_body_bytes),
                    d.methods_new,
                    d.methods_shared_prior,
                    mb(d.call_site_bytes),
                    d.call_site_count,
                    mb(d.container_bytes),
                    mb(d.constant_ivar_bytes),
                );
            };
            report(
                "workspace_registry",
                &workspace_registry.deep_breakdown(&mut seen),
            );
            if let Some(base) = state.workspace_state.display_base_registry_for_breakdown() {
                report("display_base_registry", &base.deep_breakdown(&mut seen));
            }
            if let Some(cached) = state.cached_display_registry.most_recent() {
                report(
                    "cached_display_registry",
                    &cached.registry.deep_breakdown(&mut seen),
                );
            }
            if let Some(cached) = state.cached_display.most_recent() {
                let mut d = cached.registry.deep_breakdown(&mut seen);
                d.accumulate(&cached.analysis.deep_breakdown(&mut seen));
                report("cached_display(+snapshot)", &d);
            }
            report(
                "snapshots",
                &state.workspace_state.snapshots_deep_breakdown(&mut seen),
            );
            report("user_rbs", &state.user_rbs.deep_breakdown(&mut seen));
            let mut stdlib_cache_deep = crate::registry::RegistryDeepBytes::default();
            state.stdlib_loader.for_each_cached_registry(|registry| {
                stdlib_cache_deep.accumulate(&registry.deep_breakdown(&mut seen));
            });
            report("stdlib_cache", &stdlib_cache_deep);
            eprintln!(
                "[deep] dep_graph: {:.1}MB",
                mb(state.workspace_state.dep_graph_deep_bytes()),
            );
        }

        // Ground-truth attribution: drop each holder in turn and purge to observe the real RSS delta (run last, since it destroys state).
        {
            fn collect_and_resident() -> u64 {
                unsafe extern "C" {
                    fn mi_collect(force: bool);
                }
                // Frees on an analysis-pool worker go to that thread's own heap — RSS won't move unless every worker runs collect.
                lsp_analysis_pool().broadcast(|_| unsafe { mi_collect(true) });
                unsafe { mi_collect(true) };
                current_phys_footprint_bytes()
            }
            let r0 = collect_and_resident() / 1_048_576;
            // The display-analysis bindings pin the registry/snapshot — release them before dropping.
            drop(_analysis);
            drop(_ws_registry);
            state.cached_display.clear();
            state.cached_display_registry.clear();
            let r1 = collect_and_resident() / 1_048_576;
            state
                .workspace_state
                .drop_all_file_snapshots_for_breakdown();
            let r2 = collect_and_resident() / 1_048_576;
            drop(workspace_registry);
            state.workspace_state.drop_cached_registry_for_breakdown();
            let r3 = collect_and_resident() / 1_048_576;
            state.user_rbs = Arc::new(TypeRegistry::new());
            let r4 = collect_and_resident() / 1_048_576;
            state.workspace_state = crate::workspace_state::WorkspaceState::new();
            let r5 = collect_and_resident() / 1_048_576;
            // Temporary debug: bisect the stdlib cache breakdown by dropping it in stages
            if std::env::var_os("TYDA_BENCH_STDLIB_BISECT").is_some() {
                let cached = state.stdlib_loader.debug_take_cached_registries();
                let s0 = collect_and_resident() / 1_048_576;
                let mut owned: Vec<crate::registry::TypeRegistry> = Vec::new();
                let mut still_shared = 0usize;
                for arc in cached {
                    match Arc::try_unwrap(arc) {
                        Ok(registry) => owned.push(registry),
                        Err(_) => still_shared += 1,
                    }
                }
                let s1 = collect_and_resident() / 1_048_576;
                for registry in &mut owned {
                    registry.debug_drop_alias_maps();
                }
                let s2 = collect_and_resident() / 1_048_576;
                for registry in &mut owned {
                    registry.debug_drop_lookup_caches();
                }
                let s3 = collect_and_resident() / 1_048_576;
                for registry in &mut owned {
                    registry.debug_drop_annotated_params();
                }
                let s4 = collect_and_resident() / 1_048_576;
                for registry in &mut owned {
                    registry.debug_drop_method_bodies();
                }
                let s5 = collect_and_resident() / 1_048_576;
                for registry in &mut owned {
                    registry.debug_drop_constants_ivars();
                }
                let s6 = collect_and_resident() / 1_048_576;
                drop(owned);
                let s7 = collect_and_resident() / 1_048_576;
                eprintln!(
                    "[bisect] cache_map={}MB unwrap(loser={still_shared})={}MB alias_maps={}MB lookup_caches={}MB annotated_params={}MB method_bodies={}MB constants_ivars={}MB class_shells={}MB",
                    r5.saturating_sub(s0),
                    s0.saturating_sub(s1),
                    s1.saturating_sub(s2),
                    s2.saturating_sub(s3),
                    s3.saturating_sub(s4),
                    s4.saturating_sub(s5),
                    s5.saturating_sub(s6),
                    s6.saturating_sub(s7),
                );
            }
            state.stdlib_loader = Arc::new(LazyRbsLoader::new(std::path::PathBuf::new()));
            drop(stdlib_loader);
            let r6 = collect_and_resident() / 1_048_576;
            drop(state);
            let r7 = collect_and_resident() / 1_048_576;
            let (sym_count, sym_bytes) = crate::sym::interner_stats();
            eprintln!(
                "[true] baseline={r0}MB display_caches={}MB snapshots={}MB merged_registry={}MB user_rbs={}MB ws_state+depgraph={}MB stdlib_cache={}MB rest_of_state={}MB floor={r7}MB sym_interner={sym_count}syms/{:.1}MB",
                r0.saturating_sub(r1),
                r1.saturating_sub(r2),
                r2.saturating_sub(r3),
                r3.saturating_sub(r4),
                r4.saturating_sub(r5),
                r5.saturating_sub(r6),
                r6.saturating_sub(r7),
                sym_bytes as f64 / 1_048_576.0,
            );
        }
        eprintln!(
            "[breakdown] stdlib_loader.file_cache: parsed_files={}, classes={}, methods={}",
            stdlib_cache_warm.parsed_file_count,
            stdlib_cache_warm.class_count,
            stdlib_cache_warm.method_count,
        );
        eprintln!(
            "[breakdown] lazy_rbi_loader: indexed_classes={}, indexed_files={}, pending_files={}, parsed_files={}, classes={}, methods={}",
            rbi_cache_warm.indexed_class_count,
            rbi_cache_warm.indexed_file_count,
            rbi_cache_warm.pending_file_count,
            rbi_cache_warm.parsed_file_count,
            rbi_cache_warm.class_count,
            rbi_cache_warm.method_count,
        );
    }

    #[test]
    fn bench_workspace_rescan_mastodon_scale() {
        let subject_root = std::env::var_os("TYDA_LSP_BENCH_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("subject/gitlab/app")
            });
        if !subject_root.exists() {
            eprintln!(
                "skipping: lsp benchmark subject not found: {}",
                subject_root.display()
            );
            return;
        }
        let _bench_guard = mastodon_bench_guard();

        let loader = stdlib_loader();
        let project_root = subject_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| *name == "app")
            .and_then(|_| subject_root.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| subject_root.clone());
        let options = AnalysisOptions {
            project_root: Some(project_root),
            ..AnalysisOptions::default()
        };
        let user_rbs = TypeRegistry::new();
        let scan_roots = vec![subject_root];

        let mut state = new_test_state();
        let initial_scan_files =
            collect_workspace_scan_files(&scan_roots, &[], &[], &HashMap::new(), None, false);
        for_each_workspace_scan_result(
            &initial_scan_files,
            WorkspaceScanInputs {
                cached_entries: &HashMap::new(),
                open_docs: &HashMap::new(),
                user_rbs: &user_rbs,
                stdlib_loader: &loader,
                lazy_rbi_loader: None,
                options: &options,
            },
            || false,
            |result| {
                if let WorkspaceScanResult::Analyzed {
                    file_path,
                    content_hash,
                    analysis,
                    fingerprints,
                    file_deps,
                    on_disk_stamp,
                } = result
                {
                    state
                        .workspace_state
                        .upsert_scanned_file_with_stamp_and_fingerprints(
                            file_path,
                            content_hash,
                            analysis,
                            file_deps,
                            on_disk_stamp,
                            fingerprints,
                        );
                }
            },
        );

        let cached_entries: HashMap<String, WorkspaceScanCacheEntry> = state
            .workspace_state
            .file_paths()
            .filter_map(|path| {
                state.workspace_state.workspace_file(path).map(|entry| {
                    (
                        path.to_string(),
                        WorkspaceScanCacheEntry {
                            content_hash: entry.content_hash,
                            on_disk_stamp: entry.on_disk_stamp,
                        },
                    )
                })
            })
            .collect();
        let known_files: Vec<PathBuf> = state
            .workspace_state
            .file_paths()
            .map(PathBuf::from)
            .collect();
        let rescan_files = collect_workspace_scan_files(
            &scan_roots,
            &known_files,
            &[],
            &HashMap::new(),
            None,
            false,
        );

        let mut refreshed = 0usize;
        let t_rescan = std::time::Instant::now();
        for_each_workspace_scan_result(
            &rescan_files,
            WorkspaceScanInputs {
                cached_entries: &cached_entries,
                open_docs: &HashMap::new(),
                user_rbs: &user_rbs,
                stdlib_loader: &loader,
                lazy_rbi_loader: None,
                options: &options,
            },
            || false,
            |result| {
                if let WorkspaceScanResult::RefreshStamp { .. } = result {
                    refreshed += 1;
                }
            },
        );
        let rescan_ms = t_rescan.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "[bench] workspace rescan: {:.0}ms ({} known files, {} refreshed)",
            rescan_ms,
            known_files.len(),
            refreshed,
        );

        let rescan_max_ms = if cfg!(debug_assertions) { 800.0 } else { 120.0 };
        assert!(
            rescan_ms < rescan_max_ms,
            "workspace rescan took {rescan_ms:.0}ms, expected < {rescan_max_ms:.0}ms"
        );
    }
}
