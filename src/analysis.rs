use ruby_prism::{self as prism, Node};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::inference::{FileAnalysisSnapshot, InferenceEngine};
use crate::project::{DslActivation, ProjectVersions, detect_realtime_dsl_from_source};
use crate::rbs::display::user_facing_type;
use crate::rbs::inline::extract_inline_assertion_comments;
use crate::rbs::stdlib_loader::LazyRbsLoader;
use crate::registry::TypeRegistry;
use crate::sorbet::annotations::{
    extract_annotation_comments, extract_sig_source, extract_sorbet_comment_type_aliases,
    sorbet_comment_mode,
};
use crate::sorbet::comments::extract_sorbet_self_bind_comments;
use crate::sorbet::rbi::LazyRbiLoader;
use crate::types::Type;

/// 64MiB because a deep AST can exhaust the default worker stack (~2MiB); this is just a virtual reservation, so it's RSS-neutral.
pub const ANALYSIS_WORKER_STACK_SIZE: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DependencyCollection {
    Enabled,
    Disabled,
}

impl DependencyCollection {
    fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HoverSnapshotMode {
    Record,
    Skip,
}

impl HoverSnapshotMode {
    fn should_record(self) -> bool {
        matches!(self, Self::Record)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnnotatedBodyHoverMode {
    Enabled,
    Disabled,
}

impl AnnotatedBodyHoverMode {
    fn should_record(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueryAnalysisMode {
    HoverSnapshots,
    FileFactsOnly,
}

impl QueryAnalysisMode {
    fn hover_snapshot_mode(self) -> HoverSnapshotMode {
        match self {
            Self::HoverSnapshots => HoverSnapshotMode::Record,
            Self::FileFactsOnly => HoverSnapshotMode::Skip,
        }
    }

    fn annotated_body_hover_mode(self) -> AnnotatedBodyHoverMode {
        match self {
            Self::HoverSnapshots => AnnotatedBodyHoverMode::Enabled,
            Self::FileFactsOnly => AnnotatedBodyHoverMode::Disabled,
        }
    }

    fn resolution_depth(self) -> ResolutionDepth {
        match self {
            Self::HoverSnapshots => ResolutionDepth::Full,
            Self::FileFactsOnly => ResolutionDepth::ExportedFactsOnly,
        }
    }

    fn should_compact(self) -> bool {
        matches!(self, Self::FileFactsOnly)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExternalRbsLoading {
    Preload,
    OnDemand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolutionDepth {
    Full,
    ExportedFactsOnly,
}

impl ResolutionDepth {
    fn keeps_only_exported_facts(self) -> bool {
        matches!(self, Self::ExportedFactsOnly)
    }
}

pub struct HoverResult {
    pub name: String,
    pub ty: Type,
    pub display_rbs: Option<String>,
    pub type_params: Vec<(String, Type)>,
    pub can_enrich_from_workspace: bool,
    pub unresolved_method: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AnalysisTimings {
    pub parse: Duration,
    pub comments: Duration,
    pub preload: Duration,
    pub definition_collection: Duration,
    pub build_subclass_index: Duration,
    pub finalize_pending_scoped_type_refs: Duration,
    pub resolve_subclass_method_refs: Duration,
    pub merge_alias_call_sites: Duration,
    pub parameter_reference_resolution: Duration,
    pub resolve_method_return_refs: Duration,
    pub backward_propagate: Duration,
    pub sync_module_function_mirrors: Duration,
    pub receiver_reference_preload: Duration,
    pub hover_snapshots: Duration,
    pub deps: Duration,
    pub into_file_analysis_snapshot: Duration,
}

impl AnalysisTimings {
    pub fn total(self) -> Duration {
        self.parse
            + self.comments
            + self.preload
            + self.definition_collection
            + self.build_subclass_index
            + self.finalize_pending_scoped_type_refs
            + self.resolve_subclass_method_refs
            + self.merge_alias_call_sites
            + self.parameter_reference_resolution
            + self.resolve_method_return_refs
            + self.backward_propagate
            + self.sync_module_function_mirrors
            + self.receiver_reference_preload
            + self.hover_snapshots
            + self.deps
            + self.into_file_analysis_snapshot
    }
}

#[derive(Clone, Debug, Default)]
pub struct AnalysisOptions {
    pub rails_mode: bool,
    pub dsl_activation: DslActivation,
    pub project_versions: ProjectVersions,
    pub project_root: Option<PathBuf>,
}

pub fn hover_at(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    file_path: &str,
    line: usize,
    column: usize,
) -> Option<HoverResult> {
    hover_at_with_options(
        source,
        rbs_registry,
        lazy_loader,
        file_path,
        line,
        column,
        false,
    )
}

pub fn hover_at_with_options(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    file_path: &str,
    line: usize,
    column: usize,
    rails_mode: bool,
) -> Option<HoverResult> {
    hover_at_with_analysis_options(
        source,
        rbs_registry,
        lazy_loader,
        file_path,
        line,
        column,
        AnalysisOptions {
            rails_mode,
            dsl_activation: DslActivation::with_rails_mode(rails_mode),
            project_versions: ProjectVersions::default(),
            project_root: None,
        },
    )
}

pub fn hover_at_with_analysis_options(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    file_path: &str,
    line: usize,
    column: usize,
    options: AnalysisOptions,
) -> Option<HoverResult> {
    let offset = line_col_to_offset(source.as_bytes(), line, column)?;
    let (mut engine, _timings) = build_engine_with_timings(AnalysisRequest {
        source,
        rbs_registry,
        lazy_loader: Some(lazy_loader),
        lazy_rbi_loader: None,
        file_path: Some(file_path),
        options: &options,
        dependency_collection: DependencyCollection::Disabled,
        hover_snapshot_mode: HoverSnapshotMode::Record,
        annotated_body_hover_mode: AnnotatedBodyHoverMode::Enabled,
        external_rbs_loading: ExternalRbsLoading::OnDemand,
        resolution_depth: ResolutionDepth::Full,
        rbi_declaration_source: false,
    });
    engine.resolve_pending_constant_definition_snapshots();
    engine.find_hover_at(source, offset)
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

pub fn analyze_source(source: &str) -> TypeRegistry {
    analyze_source_impl(source, None, None, None, None)
}

pub fn analyze_source_with_rbs(source: &str, rbs_registry: &TypeRegistry) -> TypeRegistry {
    analyze_source_impl(source, Some(rbs_registry), None, None, None)
}

pub fn analyze_source_with_lazy_rbs(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
) -> TypeRegistry {
    analyze_source_impl(source, rbs_registry, Some(lazy_loader), None, None)
}

/// RBI declaration source: an empty-body `def` is a declaration, not an implementation, so its return type becomes Untyped.
pub fn analyze_rbi_declaration_source_with_lazy_rbs(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
) -> TypeRegistry {
    let options = AnalysisOptions::default();
    let (engine, _timings) = build_engine_with_timings(AnalysisRequest {
        source,
        rbs_registry,
        lazy_loader: Some(lazy_loader),
        lazy_rbi_loader: None,
        file_path: None,
        options: &options,
        dependency_collection: DependencyCollection::Disabled,
        hover_snapshot_mode: HoverSnapshotMode::Skip,
        annotated_body_hover_mode: AnnotatedBodyHoverMode::Disabled,
        external_rbs_loading: ExternalRbsLoading::Preload,
        resolution_depth: ResolutionDepth::Full,
        rbi_declaration_source: true,
    });
    engine.into_registry()
}

pub fn analyze_source_with_lazy_rbs_rails(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    rails_mode: bool,
) -> TypeRegistry {
    let (registry, _deps) = analyze_source_inner(
        source,
        rbs_registry,
        Some(lazy_loader),
        None,
        None,
        AnalysisOptions {
            rails_mode,
            dsl_activation: DslActivation::with_rails_mode(rails_mode),
            project_versions: ProjectVersions::default(),
            project_root: None,
        },
        DependencyCollection::Disabled,
    );
    registry
}

pub fn analyze_source_with_file_path(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    file_path: &str,
) -> TypeRegistry {
    analyze_source_with_file_path_rails(source, rbs_registry, lazy_loader, None, file_path, false)
}

pub fn analyze_source_with_file_path_rails(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: &str,
    rails_mode: bool,
) -> TypeRegistry {
    analyze_source_with_file_path_rails_with_options(
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        AnalysisOptions {
            rails_mode,
            dsl_activation: DslActivation::with_rails_mode(rails_mode),
            project_versions: ProjectVersions::default(),
            project_root: None,
        },
    )
}

pub fn analyze_source_with_file_path_rails_with_options(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: &str,
    options: AnalysisOptions,
) -> TypeRegistry {
    let (analysis, _, _) = analyze_source_cached_with_deps_lazy(
        source,
        rbs_registry,
        Some(lazy_loader),
        lazy_rbi_loader,
        Some(file_path),
        options,
        false,
    );
    analysis.materialized_registry()
}

pub fn analyze_file_registry_with_options(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: &str,
    options: AnalysisOptions,
) -> TypeRegistry {
    analyze_file_registry_timed(
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options,
        false,
    )
    .0
}

pub fn analyze_source_with_file_path_rails_timed_lazy(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: &str,
    options: AnalysisOptions,
    lazy_rbs_merge: bool,
) -> (TypeRegistry, AnalysisTimings) {
    let (analysis, _, timings) = analyze_source_cached_with_deps_lazy(
        source,
        rbs_registry,
        Some(lazy_loader),
        lazy_rbi_loader,
        Some(file_path),
        options,
        lazy_rbs_merge,
    );
    (analysis.materialized_registry(), timings)
}

pub fn analyze_file_registry_timed(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: &str,
    options: AnalysisOptions,
    lazy_external_rbs: bool,
) -> (TypeRegistry, AnalysisTimings) {
    let (analysis, _, timings) = analyze_source_cached_with_deps_lazy(
        source,
        rbs_registry,
        Some(lazy_loader),
        lazy_rbi_loader,
        Some(file_path),
        options,
        lazy_external_rbs,
    );
    (analysis.materialized_registry(), timings)
}

pub fn analyze_compact_file_snapshot_timed(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: &str,
    options: AnalysisOptions,
    lazy_rbs_merge: bool,
) -> (FileAnalysisSnapshot, AnalysisTimings) {
    let (engine, timings) = build_compact_scan_engine(
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        &options,
        lazy_rbs_merge,
        HoverSnapshotMode::Skip,
    );
    (
        compact_file_snapshot(engine.into_file_analysis_snapshot()),
        timings,
    )
}

/// Target scan for CLI `--diagnostics` after the definitions-only skeleton exists.
/// Records hover / arg-check / unresolved-constant sites so judgment does not
/// Full-reanalyze. Keeps file context (dsl activation, path) for the replay engine.
pub fn analyze_cli_diagnostic_target_snapshot_timed(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: &str,
    options: AnalysisOptions,
    lazy_rbs_merge: bool,
) -> (FileAnalysisSnapshot, AnalysisTimings) {
    let (engine, timings) = build_compact_scan_engine(
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        &options,
        lazy_rbs_merge,
        HoverSnapshotMode::Record,
    );
    let mut snapshot = engine.into_file_analysis_snapshot();
    // Keep loc-less synthetic DSL (e.g. Draper `method_missing`) and drop
    // workspace classes pulled in via OnDemand.
    snapshot.compact_current_pass_facts();
    (snapshot, timings)
}

#[allow(clippy::too_many_arguments)]
fn build_compact_scan_engine<'a>(
    source: &'a str,
    rbs_registry: Option<&'a TypeRegistry>,
    lazy_loader: &'a LazyRbsLoader,
    lazy_rbi_loader: Option<&'a LazyRbiLoader>,
    file_path: &'a str,
    options: &'a AnalysisOptions,
    lazy_rbs_merge: bool,
    hover_snapshot_mode: HoverSnapshotMode,
) -> (InferenceEngine<'a>, AnalysisTimings) {
    build_engine_with_timings(AnalysisRequest {
        source,
        rbs_registry,
        lazy_loader: Some(lazy_loader),
        lazy_rbi_loader,
        file_path: Some(file_path),
        options,
        dependency_collection: DependencyCollection::Disabled,
        hover_snapshot_mode,
        annotated_body_hover_mode: AnnotatedBodyHoverMode::Disabled,
        external_rbs_loading: if lazy_rbs_merge {
            ExternalRbsLoading::OnDemand
        } else {
            ExternalRbsLoading::Preload
        },
        resolution_depth: ResolutionDepth::ExportedFactsOnly,
        rbi_declaration_source: false,
    })
}

pub fn analyze_definitions_only_snapshot_timed(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: &str,
    options: AnalysisOptions,
) -> (FileAnalysisSnapshot, AnalysisTimings) {
    let (engine, timings) = build_engine_with_timings(AnalysisRequest {
        source,
        rbs_registry,
        lazy_loader: Some(lazy_loader),
        lazy_rbi_loader,
        file_path: Some(file_path),
        options: &options,
        dependency_collection: DependencyCollection::Disabled,
        hover_snapshot_mode: HoverSnapshotMode::Skip,
        annotated_body_hover_mode: AnnotatedBodyHoverMode::Disabled,
        // Merging per-file external closures for context files would delay real RBI loading across the whole workspace.
        external_rbs_loading: ExternalRbsLoading::OnDemand,
        // Only the definition skeleton is needed, so local method-ref / backward / receiver-preload are skipped.
        resolution_depth: ResolutionDepth::ExportedFactsOnly,
        rbi_declaration_source: false,
    });
    let mut snapshot = compact_file_snapshot(engine.into_file_analysis_snapshot());
    snapshot.strip_method_body_summary();
    (snapshot, timings)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaygroundResult {
    pub rbs: String,
    pub diagnostics: Vec<crate::diagnostics::TypeDiagnostic>,
    pub hovers: Vec<PlaygroundHover>,
    pub code_lens: Vec<PlaygroundCodeLens>,
    pub ruby_syntax_error: bool,
    pub rbs_syntax_error: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaygroundHover {
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub name: String,
    pub display: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaygroundCodeLens {
    pub line: u32,
    pub signature: String,
}

pub fn playground_analyze(
    source: &str,
    user_rbs_text: &str,
    lazy_loader: &LazyRbsLoader,
    file_path: &str,
) -> PlaygroundResult {
    let ruby_syntax_error = prism::parse(source.as_bytes()).errors().next().is_some();

    let mut rbs_registry = TypeRegistry::new();
    let (user_rbs, rbs_syntax_error) = if user_rbs_text.trim().is_empty() {
        (None, false)
    } else if crate::rbs::import::rbs_parses(user_rbs_text) {
        crate::rbs::import::load_rbs_string(user_rbs_text, &mut rbs_registry);
        (Some(&rbs_registry), false)
    } else {
        (None, true)
    };

    let options = AnalysisOptions::default();
    let (snapshot, _, _timings) = analyze_source_for_display(
        source,
        user_rbs,
        Some(lazy_loader),
        None,
        Some(file_path),
        options,
    );

    let rbs = crate::rbs::render::render_rbs_for_file(snapshot.registry(), file_path);

    let methods = snapshot.methods_for_file(file_path);

    let method_def_lines: Vec<u32> = methods
        .iter()
        .filter_map(|(_class, sig)| sig.loc.map(|loc| loc.line))
        .collect();
    let suppressor = SyntaxErrorSuppressor::new(source, &method_def_lines);

    let mut diagnostics = crate::diagnostics::method_call_diagnostics(
        &snapshot,
        source,
        file_path,
        lazy_loader,
        None,
        user_rbs,
    );
    if suppressor.is_active() {
        diagnostics.retain(|diag| !suppressor.suppresses_line(diag.line));
    }

    let code_lens = methods
        .iter()
        .filter_map(|(_class, sig)| {
            // Hide codelens for already-annotated methods, same as the LSP (it disappears once `#:` is inserted).
            if sig.rbs_inline_annotated || sig.sig_annotated {
                return None;
            }
            let loc = sig.loc?;
            if suppressor.suppresses_def_line(loc.line) {
                return None;
            }
            Some(PlaygroundCodeLens {
                line: loc.line,
                signature: crate::rbs::display::format_method_sig_for_lens_with_names(sig, false),
            })
        })
        .collect();

    let spans: Vec<(usize, usize, String)> = snapshot
        .hover_index
        .snapshots
        .iter()
        .map(|snap| (snap.start, snap.end, snap.name.clone()))
        .collect();
    let mut hovers = Vec::new();
    for (start, end, name) in spans {
        if start >= end {
            continue;
        }
        let (line, column) = offset_to_line_col(source, start);
        if suppressor.suppresses_line(line) {
            continue;
        }
        let Some(result) = snapshot.hover_at(source, start, lazy_loader, user_rbs) else {
            continue;
        };
        let display = format_hover_body(&result);
        if display.is_empty() {
            continue;
        }
        let (end_line, end_column) = offset_to_line_col(source, end);
        hovers.push(PlaygroundHover {
            line,
            column,
            end_line,
            end_column,
            name,
            display,
        });
    }

    PlaygroundResult {
        rbs,
        diagnostics,
        hovers,
        code_lens,
        ruby_syntax_error,
        rbs_syntax_error,
    }
}

/// Shared by LSP / playground: strips the leading `name: ` from `display_rbs` and returns just the body.
pub fn format_hover_body(hover_result: &HoverResult) -> String {
    let ty_str = user_facing_type(&hover_result.ty).to_string();
    let body = hover_result
        .display_rbs
        .as_ref()
        .map(|display_rbs| {
            display_rbs
                .strip_prefix(&format!("{}: ", hover_result.name))
                .unwrap_or(display_rbs)
                .to_string()
        })
        .unwrap_or(ty_str);
    let mut lines = vec![format!("[Tyda] {body}")];
    if let Some(type_params) = format_hover_type_params(&hover_result.type_params) {
        lines.push(format!("# type params: {type_params}"));
    }
    if hover_result.display_rbs.is_none()
        && let Some(ref method) = hover_result.unresolved_method
    {
        lines.push(format!("# unresolved: {method}"));
    }
    lines.join("\n")
}

fn format_hover_type_params(type_params: &[(String, Type)]) -> Option<String> {
    if type_params.is_empty() {
        return None;
    }
    Some(
        type_params
            .iter()
            .map(|(name, ty)| format!("{name} = {}", user_facing_type(ty)))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// Suppresses codelens / diagnostics / hover only for methods whose value is broken by a syntax error (shared by LSP / playground).
pub struct SyntaxErrorSuppressor {
    method_def_lines: Vec<u32>,
    broken_def_lines: std::collections::HashSet<u32>,
}

impl SyntaxErrorSuppressor {
    pub fn new(source: &str, method_def_lines: &[u32]) -> Self {
        let mut sorted: Vec<u32> = method_def_lines.to_vec();
        sorted.sort_unstable();
        sorted.dedup();

        let mut broken = std::collections::HashSet::new();
        let parsed = prism::parse(source.as_bytes());
        for error in parsed.errors() {
            // An unclosed `def` / EOF means the input is mid-edit and the partial AST is still valid — avoids codelens flicker.
            if Self::is_incremental_structural_error(error.message()) {
                continue;
            }
            let offset = error.location().start_offset();
            let (line, _) = offset_to_line_col(source, offset);
            if let Some(&def) = sorted.iter().rev().find(|&&d| d <= line) {
                broken.insert(def);
            }
        }

        Self {
            method_def_lines: sorted,
            broken_def_lines: broken,
        }
    }

    /// Structural errors that don't break the value, e.g. an unclosed `def` / EOF.
    fn is_incremental_structural_error(message: &str) -> bool {
        message.contains("expected an `end`") || message.contains("unexpected end-of-input")
    }

    pub fn is_active(&self) -> bool {
        !self.broken_def_lines.is_empty()
    }

    pub fn suppresses_def_line(&self, def_line: u32) -> bool {
        self.broken_def_lines.contains(&def_line)
    }

    pub fn suppresses_line(&self, line: u32) -> bool {
        match self.method_def_lines.iter().rev().find(|&&d| d <= line) {
            Some(&def) => self.broken_def_lines.contains(&def),
            None => false,
        }
    }
}

fn offset_to_line_col(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut column = 0u32;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            return (line, column);
        }
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    (line, column)
}

pub fn analyze_source_with_deps(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    file_path: Option<&str>,
) -> (TypeRegistry, crate::dep_graph::FileDeps) {
    analyze_source_inner(
        source,
        rbs_registry,
        lazy_loader,
        None,
        file_path,
        AnalysisOptions::default(),
        DependencyCollection::Enabled,
    )
}

pub fn analyze_source_with_deps_rails(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    file_path: Option<&str>,
    rails_mode: bool,
) -> (TypeRegistry, crate::dep_graph::FileDeps) {
    analyze_source_with_deps_rails_with_options(
        source,
        rbs_registry,
        lazy_loader,
        file_path,
        AnalysisOptions {
            rails_mode,
            dsl_activation: DslActivation::with_rails_mode(rails_mode),
            project_versions: ProjectVersions::default(),
            project_root: None,
        },
    )
}

pub fn analyze_source_with_deps_rails_with_options(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
) -> (TypeRegistry, crate::dep_graph::FileDeps) {
    analyze_source_inner(
        source,
        rbs_registry,
        lazy_loader,
        None,
        file_path,
        options,
        DependencyCollection::Enabled,
    )
}

pub fn analyze_source_cached_with_deps_rails_with_options(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
) -> (FileAnalysisSnapshot, crate::dep_graph::FileDeps) {
    analyze_cached_file_with_deps(source, rbs_registry, lazy_loader, file_path, options)
}

pub fn analyze_source_cached_with_deps_rails_with_options_and_rbi(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
) -> (FileAnalysisSnapshot, crate::dep_graph::FileDeps) {
    analyze_cached_file_with_deps_and_rbi(
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options,
    )
}

pub fn analyze_source_facts_with_deps_lazy(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
) -> (FileAnalysisSnapshot, crate::dep_graph::FileDeps) {
    analyze_file_facts_with_deps(source, rbs_registry, lazy_loader, file_path, options)
}

pub fn analyze_source_facts_with_deps_lazy_and_rbi(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
) -> (FileAnalysisSnapshot, crate::dep_graph::FileDeps) {
    analyze_file_facts_with_deps_and_rbi(
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options,
    )
}

pub fn analyze_file_facts_with_deps(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
) -> (FileAnalysisSnapshot, crate::dep_graph::FileDeps) {
    analyze_file_facts_with_deps_and_rbi(
        source,
        rbs_registry,
        lazy_loader,
        None,
        file_path,
        options,
    )
}

pub fn analyze_file_facts_with_deps_and_rbi(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
) -> (FileAnalysisSnapshot, crate::dep_graph::FileDeps) {
    analyze_file_facts_with_deps_and_rbi_with_options_ref(
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        &options,
    )
}

pub(crate) fn analyze_file_facts_with_deps_and_rbi_with_options_ref(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
    options: &AnalysisOptions,
) -> (FileAnalysisSnapshot, crate::dep_graph::FileDeps) {
    let (engine, _timings) = build_engine_with_timings(AnalysisRequest {
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options,
        dependency_collection: DependencyCollection::Enabled,
        hover_snapshot_mode: HoverSnapshotMode::Skip,
        annotated_body_hover_mode: AnnotatedBodyHoverMode::Disabled,
        external_rbs_loading: ExternalRbsLoading::OnDemand,
        // Workspace scan collects file-local facts only; deep local resolution is reintroduced by a later merge.
        resolution_depth: ResolutionDepth::ExportedFactsOnly,
        rbi_declaration_source: false,
    });
    let (analysis, deps) = engine.into_file_analysis_snapshot_and_deps();
    (compact_file_snapshot(analysis), deps)
}

/// CLI `--diagnostics` judgment from sites recorded against the workspace skeleton.
/// A fresh engine is still required — reusing a Full-resolution replay engine would
/// short-circuit lazy load (`method_call_diagnostics` contract).
pub fn cli_diagnostics_from_snapshot(
    snapshot: &crate::inference::FileAnalysisSnapshot,
    source: &str,
    file_path: &str,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    workspace_registry: Option<&TypeRegistry>,
) -> Vec<crate::diagnostics::TypeDiagnostic> {
    let diagnostics = crate::diagnostics::method_call_diagnostics(
        snapshot,
        source,
        file_path,
        lazy_loader,
        lazy_rbi_loader,
        workspace_registry,
    );

    suppress_diagnostics_in_broken_methods(
        diagnostics,
        snapshot,
        source,
        file_path,
        workspace_registry,
    )
}

/// Same as `cli_diagnostics_from_snapshot`, but moves the snapshot into the
/// fresh engine so CLI batch judgment does not clone the registry.
pub fn cli_diagnostics_from_snapshot_owned(
    snapshot: crate::inference::FileAnalysisSnapshot,
    source: &str,
    file_path: &str,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    workspace_registry: Option<&TypeRegistry>,
) -> Vec<crate::diagnostics::TypeDiagnostic> {
    let method_def_lines = diagnostic_method_def_lines(&snapshot, file_path, workspace_registry);
    let mut diagnostics = crate::diagnostics::method_call_diagnostics_owned(
        snapshot,
        source,
        file_path,
        lazy_loader,
        lazy_rbi_loader,
        workspace_registry,
    );
    let suppressor = SyntaxErrorSuppressor::new(source, &method_def_lines);
    if suppressor.is_active() {
        diagnostics.retain(|diag| !suppressor.suppresses_line(diag.line));
    }
    diagnostics
}

fn diagnostic_method_def_lines(
    snapshot: &crate::inference::FileAnalysisSnapshot,
    file_path: &str,
    workspace_registry: Option<&TypeRegistry>,
) -> Vec<u32> {
    // Loc lines live on the file registry; do not materialize method-body summary.
    if snapshot.registry().class_count() == 0 {
        workspace_registry
            .map(|registry| registry.methods_for_file(file_path))
            .unwrap_or_default()
    } else {
        snapshot.registry().methods_for_file(file_path)
    }
    .iter()
    .filter_map(|(_class, sig)| sig.loc.map(|loc| loc.line))
    .collect()
}

fn suppress_diagnostics_in_broken_methods(
    mut diagnostics: Vec<crate::diagnostics::TypeDiagnostic>,
    snapshot: &crate::inference::FileAnalysisSnapshot,
    source: &str,
    file_path: &str,
    workspace_registry: Option<&TypeRegistry>,
) -> Vec<crate::diagnostics::TypeDiagnostic> {
    let method_def_lines = diagnostic_method_def_lines(snapshot, file_path, workspace_registry);
    let suppressor = SyntaxErrorSuppressor::new(source, &method_def_lines);
    if suppressor.is_active() {
        diagnostics.retain(|diag| !suppressor.suppresses_line(diag.line));
    }
    diagnostics
}

/// CLI `--diagnostics` single-file replay (tests / fallback): workspace-visible
/// collection with hover sites, without annotated-body registry clone or Full
/// post-passes.
pub fn cli_diagnostics_for_source(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: &str,
    options: AnalysisOptions,
) -> Vec<crate::diagnostics::TypeDiagnostic> {
    let snapshot = {
        let (engine, _timings) = build_engine_with_timings(AnalysisRequest {
            source,
            rbs_registry,
            lazy_loader: Some(lazy_loader),
            lazy_rbi_loader,
            file_path: Some(file_path),
            options: &options,
            dependency_collection: DependencyCollection::Disabled,
            hover_snapshot_mode: HoverSnapshotMode::Record,
            annotated_body_hover_mode: AnnotatedBodyHoverMode::Disabled,
            external_rbs_loading: ExternalRbsLoading::OnDemand,
            resolution_depth: ResolutionDepth::ExportedFactsOnly,
            rbi_declaration_source: false,
        });
        let mut snapshot = engine.into_file_analysis_snapshot();
        snapshot.compact_current_pass_facts();
        snapshot
    };
    cli_diagnostics_from_snapshot(
        &snapshot,
        source,
        file_path,
        lazy_loader,
        lazy_rbi_loader,
        rbs_registry,
    )
}

pub fn analyze_source_cached_with_deps_lazy(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
    lazy_rbs_merge: bool,
) -> (
    FileAnalysisSnapshot,
    crate::dep_graph::FileDeps,
    AnalysisTimings,
) {
    let (engine, timings) = build_engine_with_timings(AnalysisRequest {
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options: &options,
        dependency_collection: DependencyCollection::Enabled,
        hover_snapshot_mode: HoverSnapshotMode::Record,
        annotated_body_hover_mode: AnnotatedBodyHoverMode::Enabled,
        external_rbs_loading: if lazy_rbs_merge {
            ExternalRbsLoading::OnDemand
        } else {
            ExternalRbsLoading::Preload
        },
        resolution_depth: ResolutionDepth::Full,
        rbi_declaration_source: false,
    });
    let into_file_analysis_snapshot_started = Instant::now();
    let (analysis, deps) = engine.into_file_analysis_snapshot_and_deps();
    let mut timings = timings;
    timings.into_file_analysis_snapshot = into_file_analysis_snapshot_started.elapsed();
    (analysis, deps, timings)
}

/// Full-resolution analysis shared by interactive displays and scenario tests.
pub fn analyze_source_for_display(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
) -> (
    FileAnalysisSnapshot,
    crate::dep_graph::FileDeps,
    AnalysisTimings,
) {
    analyze_source_cached_with_deps_lazy(
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options,
        true,
    )
}

pub fn analyze_cached_file_with_deps(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
) -> (FileAnalysisSnapshot, crate::dep_graph::FileDeps) {
    analyze_cached_file_with_deps_and_rbi(
        source,
        rbs_registry,
        lazy_loader,
        None,
        file_path,
        options,
    )
}

pub fn analyze_cached_file_with_deps_and_rbi(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
) -> (FileAnalysisSnapshot, crate::dep_graph::FileDeps) {
    let (analysis, deps, _timings) = analyze_source_cached_with_deps_lazy(
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options,
        false,
    );
    (analysis, deps)
}

pub(crate) fn analyze_file_for_query(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
    mode: QueryAnalysisMode,
) -> FileAnalysisSnapshot {
    let (engine, _timings) = build_engine_with_timings(AnalysisRequest {
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options: &options,
        dependency_collection: DependencyCollection::Disabled,
        hover_snapshot_mode: mode.hover_snapshot_mode(),
        annotated_body_hover_mode: mode.annotated_body_hover_mode(),
        external_rbs_loading: ExternalRbsLoading::OnDemand,
        resolution_depth: mode.resolution_depth(),
        rbi_declaration_source: false,
    });
    let analysis = engine.into_file_analysis_snapshot();
    if mode.should_compact() {
        compact_file_snapshot(analysis)
    } else {
        analysis
    }
}

fn analyze_source_inner(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
    dependency_collection: DependencyCollection,
) -> (TypeRegistry, crate::dep_graph::FileDeps) {
    let (engine, _timings) = build_engine_with_timings(AnalysisRequest {
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options: &options,
        dependency_collection,
        hover_snapshot_mode: HoverSnapshotMode::Skip,
        annotated_body_hover_mode: AnnotatedBodyHoverMode::Disabled,
        external_rbs_loading: ExternalRbsLoading::Preload,
        resolution_depth: ResolutionDepth::Full,
        rbi_declaration_source: false,
    });
    engine.into_registry_and_deps()
}

fn analyze_source_inner_with_timings(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
    options: AnalysisOptions,
    dependency_collection: DependencyCollection,
) -> ((TypeRegistry, crate::dep_graph::FileDeps), AnalysisTimings) {
    let (engine, timings) = build_engine_with_timings(AnalysisRequest {
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options: &options,
        dependency_collection,
        hover_snapshot_mode: HoverSnapshotMode::Skip,
        annotated_body_hover_mode: AnnotatedBodyHoverMode::Disabled,
        external_rbs_loading: ExternalRbsLoading::Preload,
        resolution_depth: ResolutionDepth::Full,
        rbi_declaration_source: false,
    });
    (engine.into_registry_and_deps(), timings)
}

struct AnalysisRequest<'a> {
    source: &'a str,
    rbs_registry: Option<&'a TypeRegistry>,
    lazy_loader: Option<&'a LazyRbsLoader>,
    lazy_rbi_loader: Option<&'a LazyRbiLoader>,
    file_path: Option<&'a str>,
    options: &'a AnalysisOptions,
    dependency_collection: DependencyCollection,
    hover_snapshot_mode: HoverSnapshotMode,
    annotated_body_hover_mode: AnnotatedBodyHoverMode,
    external_rbs_loading: ExternalRbsLoading,
    resolution_depth: ResolutionDepth,
    /// RBI declaration source: makes an empty-body `def`'s return value Untyped.
    rbi_declaration_source: bool,
}

fn compact_file_snapshot(mut analysis: FileAnalysisSnapshot) -> FileAnalysisSnapshot {
    analysis.compact_file_local_facts();
    analysis.strip_base_context();
    analysis
}

fn build_engine_with_timings<'a>(
    request: AnalysisRequest<'a>,
) -> (InferenceEngine<'a>, AnalysisTimings) {
    let AnalysisRequest {
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        options,
        dependency_collection,
        hover_snapshot_mode,
        annotated_body_hover_mode,
        external_rbs_loading,
        resolution_depth,
        rbi_declaration_source,
    } = request;
    let mut timings = AnalysisTimings::default();

    // Deeply pathological nesting is bounded-degraded via the fuel budget, while legitimately large files keep full accuracy.
    crate::inference::set_infer_node_fuel_budget(source.len());

    let parse_started = Instant::now();
    let parse_result = prism::parse(source.as_bytes());
    timings.parse = parse_started.elapsed();
    let root = parse_result.node();

    let mut engine = InferenceEngine::new();
    engine.set_record_hover_snapshots(hover_snapshot_mode.should_record());
    engine.set_record_annotated_method_body_hover_snapshots(
        annotated_body_hover_mode.should_record(),
    );
    engine.set_analyzing_rbi_declaration(rbi_declaration_source);
    engine.set_defer_unresolved_body_call_sites(!resolution_depth.keeps_only_exported_facts());
    let mut effective_dsl_activation = options.dsl_activation.clone();
    detect_realtime_dsl_from_source(source, &mut effective_dsl_activation);
    engine.set_dsl_activation(effective_dsl_activation);
    engine.set_rails_mode(options.rails_mode);
    engine.set_ruby_version(options.project_versions.ruby);
    engine.set_rails_version(options.project_versions.rails);
    if let Some(ref root) = options.project_root {
        engine.set_project_root(root.clone());
    }

    if let Some(fp) = file_path {
        engine.set_file_path(fp);
    }

    if let Some(rbs_reg) = rbs_registry {
        let preload_started = Instant::now();
        match external_rbs_loading {
            ExternalRbsLoading::Preload => engine.pre_load_rbs(rbs_reg),
            ExternalRbsLoading::OnDemand => engine.set_external_rbs(rbs_reg),
        }
        timings.preload = preload_started.elapsed();
    }

    if let Some(loader) = lazy_loader {
        engine.set_lazy_loader(loader);
    }

    if let Some(loader) = lazy_rbi_loader {
        engine.set_lazy_rbi_loader(loader);
    }

    let sorbet_mode = sorbet_comment_mode(options.project_root.as_deref(), file_path);
    if sorbet_mode {
        let type_aliases =
            extract_sorbet_comment_type_aliases(source, engine.registry().type_aliases());
        for (alias_name, ty) in type_aliases {
            engine.add_type_alias(&alias_name, ty);
        }
        engine.set_self_bind_comments(extract_sorbet_self_bind_comments(&parse_result));
    }
    let comments_started = Instant::now();
    let comments = extract_annotation_comments(&parse_result, sorbet_mode);
    timings.comments = comments_started.elapsed();
    // Common subset of rbs-inline / Sorbet — narrowing still works even in projects without `sorbet/config`.
    engine.set_inline_assertion_comments(extract_inline_assertion_comments(&parse_result));

    if let Node::ProgramNode { .. } = &root {
        let program = root.as_program_node().expect("root must be ProgramNode");
        let statements = program.statements();

        let mut pending_sig: Option<String> = None;
        let top_level_collection_started = Instant::now();
        for node in statements.body().iter() {
            if let Some(sig_source) = extract_sig_source(&node) {
                pending_sig = Some(sig_source);
                continue;
            }
            engine.collect_top_level_definitions_and_calls(
                &node,
                &parse_result,
                &comments,
                pending_sig.take(),
            );
        }
        engine.refresh_program_methods_after_collection(&program, &parse_result);
        timings.definition_collection = top_level_collection_started.elapsed();

        if !resolution_depth.keeps_only_exported_facts() {
            let build_subclass_index_started = Instant::now();
            engine.build_subclass_index();
            timings.build_subclass_index = build_subclass_index_started.elapsed();

            let finalize_pending_scoped_type_refs_started = Instant::now();
            engine.finalize_pending_scoped_type_refs();
            timings.finalize_pending_scoped_type_refs =
                finalize_pending_scoped_type_refs_started.elapsed();

            let resolve_subclass_method_refs_started = Instant::now();
            engine.resolve_subclass_method_refs();
            timings.resolve_subclass_method_refs = resolve_subclass_method_refs_started.elapsed();
        }

        if !resolution_depth.keeps_only_exported_facts() {
            let merge_alias_call_sites_started = Instant::now();
            engine.merge_alias_call_sites();
            timings.merge_alias_call_sites = merge_alias_call_sites_started.elapsed();
        }

        if engine.needs_parameter_reference_resolution() {
            let parameter_reference_resolution_started = Instant::now();
            engine.resolve_parameter_references_from_calls();
            timings.parameter_reference_resolution =
                parameter_reference_resolution_started.elapsed();
        }

        if !resolution_depth.keeps_only_exported_facts() {
            let resolve_method_return_refs_started = Instant::now();
            engine.resolve_method_return_refs();
            timings.resolve_method_return_refs = resolve_method_return_refs_started.elapsed();

            let backward_propagate_started = Instant::now();
            engine.backward_propagate(&program, &parse_result);
            timings.backward_propagate = backward_propagate_started.elapsed();

            let sync_module_function_mirrors_started = Instant::now();
            engine.sync_module_function_mirrors();
            timings.sync_module_function_mirrors = sync_module_function_mirrors_started.elapsed();

            let receiver_reference_preload_started = Instant::now();
            engine.preload_receiver_reference_types();
            timings.receiver_reference_preload = receiver_reference_preload_started.elapsed();
        }

        if hover_snapshot_mode.should_record() {
            let hover_snapshots_started = Instant::now();
            engine.collect_top_level_hovers_isolated(&program, &parse_result);
            timings.hover_snapshots = hover_snapshots_started.elapsed();
        }
    }

    if dependency_collection.is_enabled() {
        let deps_started = Instant::now();
        engine.finalize_deps();
        timings.deps = deps_started.elapsed();
    }
    (engine, timings)
}

fn analyze_source_impl(
    source: &str,
    rbs_registry: Option<&TypeRegistry>,
    lazy_loader: Option<&LazyRbsLoader>,
    lazy_rbi_loader: Option<&LazyRbiLoader>,
    file_path: Option<&str>,
) -> TypeRegistry {
    let ((registry, _deps), _timings) = analyze_source_inner_with_timings(
        source,
        rbs_registry,
        lazy_loader,
        lazy_rbi_loader,
        file_path,
        AnalysisOptions::default(),
        DependencyCollection::Disabled,
    );
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dep_graph::FileDeps;
    use crate::rbs::stdlib_loader::LazyRbsLoader;
    use crate::registry::TypeRegistry;
    use std::path::PathBuf;

    fn analyze_with_dependency_collection(
        source: &str,
        dependency_collection: DependencyCollection,
    ) -> FileDeps {
        let ((_registry, deps), _timings) = analyze_source_inner_with_timings(
            source,
            None,
            None,
            None,
            None,
            AnalysisOptions::default(),
            dependency_collection,
        );
        deps
    }

    #[test]
    fn hover_definition_lookup_does_not_change_semantic_facts() {
        let source = r#"
module Outer
  module Inner
    VALUE = 1
  end
end

ALIAS = Outer::Inner

def read = ALIAS::VALUE
"#;
        let loader =
            LazyRbsLoader::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core"));
        let analyze = |hover_snapshot_mode| {
            let options = AnalysisOptions::default();
            let (engine, _) = build_engine_with_timings(AnalysisRequest {
                source,
                rbs_registry: None,
                lazy_loader: Some(&loader),
                lazy_rbi_loader: None,
                file_path: Some("scenario.rb"),
                options: &options,
                dependency_collection: DependencyCollection::Disabled,
                hover_snapshot_mode,
                annotated_body_hover_mode: if hover_snapshot_mode.should_record() {
                    AnnotatedBodyHoverMode::Enabled
                } else {
                    AnnotatedBodyHoverMode::Disabled
                },
                external_rbs_loading: ExternalRbsLoading::OnDemand,
                resolution_depth: ResolutionDepth::Full,
                rbi_declaration_source: false,
            });
            engine.into_file_analysis_snapshot().materialized_registry()
        };
        let skip = analyze(HoverSnapshotMode::Skip);
        let record = analyze(HoverSnapshotMode::Record);
        assert_eq!(
            skip.class_data_for("Object")
                .and_then(|data| data.superclass.clone()),
            record
                .class_data_for("Object")
                .and_then(|data| data.superclass.clone())
        );
    }

    #[test]
    fn timings_capture_parse_and_definition_collection() {
        let ((_registry, _deps), timings) = analyze_source_inner_with_timings(
            "class Foo; def bar; 1; end; end",
            None,
            None,
            None,
            None,
            AnalysisOptions::default(),
            DependencyCollection::Disabled,
        );

        assert!(timings.parse > Duration::ZERO);
        assert!(timings.definition_collection > Duration::ZERO);
    }

    #[test]
    fn with_deps_collects_referenced_symbols() {
        let deps = analyze_with_dependency_collection(
            r#"
class Child
  #: -> Parent
  def foo
    nil
  end
end
"#,
            DependencyCollection::Enabled,
        );

        assert!(deps.referenced_symbols.contains("Parent"));
    }

    #[test]
    fn source_local_dsl_detection_collects_concern_class_methods() {
        let registry = analyze_source(
            r#"
module Searchable
  extend ActiveSupport::Concern

  class_methods do
    def search
      "live"
    end
  end
end
"#,
        );

        assert!(
            registry
                .lookup_method_def("Searchable", "search", true)
                .is_some(),
            "source-local ActiveSupport::Concern detection should collect class_methods definitions"
        );
    }

    #[test]
    fn without_deps_skips_dependency_collection() {
        let deps = analyze_with_dependency_collection(
            r#"
class Child
  #: -> Parent
  def foo
    nil
  end
end
"#,
            DependencyCollection::Disabled,
        );

        assert!(deps.referenced_symbols.is_empty());
    }

    #[test]
    fn query_analysis_mode_records_only_required_hover_snapshots() {
        let source = r#"
class Box
  def value
    1
  end
end

box = Box.new
box.value
"#;

        let hover_analysis = analyze_file_for_query(
            source,
            None,
            None,
            None,
            Some("query.rb"),
            AnalysisOptions::default(),
            QueryAnalysisMode::HoverSnapshots,
        );
        assert!(!hover_analysis.hover_index.snapshots.is_empty());

        let facts_analysis = analyze_file_for_query(
            source,
            None,
            None,
            None,
            Some("query.rb"),
            AnalysisOptions::default(),
            QueryAnalysisMode::FileFactsOnly,
        );
        assert!(facts_analysis.hover_index.snapshots.is_empty());
        assert!(facts_analysis.registry().class_data_for("Box").is_some());
    }

    #[test]
    fn file_facts_only_keeps_class_variable_only_classes() {
        let source = r#"
class Source
  @@value = "stored"
end
"#;

        let facts_analysis = analyze_file_for_query(
            source,
            None,
            None,
            None,
            Some("query.rb"),
            AnalysisOptions::default(),
            QueryAnalysisMode::FileFactsOnly,
        );

        assert_eq!(
            facts_analysis
                .registry()
                .lookup_class_variable_type("Source", "@@value"),
            Some(Type::LiteralString("stored".to_string()))
        );
    }

    #[test]
    fn definitions_only_merges_skeleton_without_call_sites() {
        let source = r#"
FOO = 1

module M
  def helper = 1
end

class Base
  def shared = 2
end

class A < Base
  include M
  attr_accessor :x, :y

  def call
    B.new.hello
  end
end
"#;
        let loader = playground_loader();
        let (snapshot, _timings) = analyze_definitions_only_snapshot_timed(
            source,
            None,
            &loader,
            None,
            "context.rb",
            AnalysisOptions::default(),
        );

        // The definition skeleton is still kept.
        assert!(
            snapshot
                .registry()
                .lookup_method_def("A", "x", false)
                .is_some(),
            "attr_accessor should define the reader"
        );
        assert!(
            snapshot
                .registry()
                .lookup_method_def("A", "y=", false)
                .is_some(),
            "attr_accessor should define the writer"
        );
        let a = snapshot.registry().class_data_for("A").expect("A defined");
        assert_eq!(a.superclass.as_deref(), Some("Base"), "superclass recorded");
        assert!(
            a.mixins.iter().any(|m| m.module_name.as_ref() == "M"),
            "mixin recorded"
        );
        assert!(
            snapshot
                .registry()
                .class_data_for("Base")
                .and_then(|d| d.methods.iter().find(|m| m.name == "shared"))
                .is_some(),
            "cross-class method definition recorded"
        );
        assert!(
            snapshot
                .registry()
                .lookup_constant_type("Object", "FOO")
                .is_some(),
            "constant recorded"
        );

        // Call-site data is not carried over (this is a skeleton scan for contexts that don't emit diagnostics).
        assert!(
            snapshot.method_body_summary.is_empty(),
            "definitions-only must not carry method-body call sites"
        );
    }

    #[test]
    fn diagnostic_replay_split_keeps_sites_without_cloning_facts() {
        let source = r#"
class A
  def call(x)
    x.to_s
  end
end
"#;
        let loader = playground_loader();
        let mut snapshot = analyze_cli_diagnostic_target_snapshot_timed(
            source,
            None,
            &loader,
            None,
            "target.rb",
            AnalysisOptions::default(),
            true,
        )
        .0;
        assert!(
            snapshot.registry().class_count() > 0,
            "target scan should keep current-pass facts"
        );
        assert!(
            !snapshot.hover_index.arg_check_sites.is_empty()
                || !snapshot.hover_index.snapshots.is_empty(),
            "target scan should record diagnostic sites"
        );

        let replay = snapshot.split_diagnostic_replay();
        assert_eq!(
            replay.registry().class_count(),
            0,
            "replay must not retain a registry clone"
        );
        assert!(
            !replay.hover_index.arg_check_sites.is_empty()
                || !replay.hover_index.snapshots.is_empty(),
            "replay keeps recorded sites"
        );
        assert!(
            snapshot.registry().class_count() > 0,
            "merge snapshot keeps file facts"
        );
        assert!(
            snapshot.hover_index.arg_check_sites.is_empty()
                && snapshot.hover_index.snapshots.is_empty(),
            "sites move to the replay, not a second copy"
        );
    }

    #[test]
    fn subclass_method_resolution_template_method() {
        let source = r#"
class Base
  def run
    name
  end
end

class Child < Base
  def name
    "world"
  end
end
"#;
        let registry = analyze_source_impl(source, None, None, None, None);
        let base_run = registry
            .class_data_for("Base")
            .and_then(|d| d.methods.iter().find(|m| m.name == "run"))
            .map(|m| m.raw_return_type.clone());
        assert_eq!(
            base_run,
            Some(Type::LiteralString("world".to_string())),
            "Base.run raw_return_type should be resolved via subclass"
        );
    }

    #[test]
    fn compact_cli_resolution_keeps_module_function_singleton_precision() {
        let source = concat!(
            "# frozen_string_literal: true\n",
            "\n",
            "module Authn\n",
            "  module ScopedUserExtractor\n",
            "    SCOPED_USER_REGEX = /\\Auser:(\\d+)\\z/\n",
            "\n",
            "    module_function\n",
            "\n",
            "    def extract_user_id_from_scopes(scopes)\n",
            "      matches = scopes.grep(SCOPED_USER_REGEX)\n",
            "      return unless matches.length == 1\n",
            "\n",
            "      matches[0][SCOPED_USER_REGEX, 1].to_i\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        let loader =
            LazyRbsLoader::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core"));
        let compact = analyze_compact_file_snapshot_timed(
            source,
            None,
            &loader,
            None,
            "authn/scoped_user_extractor.rb",
            AnalysisOptions::default(),
            true,
        )
        .0;

        let mut registry = TypeRegistry::new();
        compact.apply_to_registry(&mut registry);
        registry.apply_cli_resolution();

        let instance = registry
            .lookup_method_sig_exact(
                "Authn::ScopedUserExtractor",
                "extract_user_id_from_scopes",
                false,
            )
            .expect("instance module_function helper");
        let singleton = registry
            .lookup_method_sig_exact(
                "Authn::ScopedUserExtractor",
                "extract_user_id_from_scopes",
                true,
            )
            .expect("singleton module_function helper");

        assert_eq!(instance.return_type.to_string(), "Integer?");
        assert_eq!(singleton.return_type.to_string(), "Integer?");
    }

    #[test]
    fn compact_cli_resolution_propagates_explicit_receiver_calls_to_direct_static_helpers() {
        let loader =
            LazyRbsLoader::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core"));
        let builder_source = concat!(
            "module PayloadBuilder\n",
            "  extend self\n",
            "\n",
            "  def build(alert)\n",
            "    attrs(alert)\n",
            "  end\n",
            "\n",
            "  def attrs(alert)\n",
            "    { title: alert.title, count: alert.count }\n",
            "  end\n",
            "end\n",
        );
        let consumer_source = concat!(
            "class Alert\n",
            "  def title\n",
            "    \"critical\"\n",
            "  end\n",
            "\n",
            "  def count\n",
            "    1\n",
            "  end\n",
            "\n",
            "  def payload\n",
            "    PayloadBuilder.build(self)\n",
            "  end\n",
            "end\n",
        );
        let builder = analyze_compact_file_snapshot_timed(
            builder_source,
            None,
            &loader,
            None,
            "lib/payload_builder.rb",
            AnalysisOptions::default(),
            true,
        )
        .0;
        let consumer = analyze_compact_file_snapshot_timed(
            consumer_source,
            None,
            &loader,
            None,
            "app/models/alert.rb",
            AnalysisOptions::default(),
            true,
        )
        .0;

        let mut registry = TypeRegistry::new();
        builder.apply_to_registry(&mut registry);
        consumer.apply_to_registry(&mut registry);
        let payload_builder_sites = registry.get_call_sites("PayloadBuilder");
        assert!(
            payload_builder_sites
                .iter()
                .any(|site| site.method_name.as_ref() == "build" && site.method_is_singleton),
            "expected singleton call site on PayloadBuilder, got {payload_builder_sites:#?}"
        );
        assert_eq!(
            registry.resolve_method_call_owners("PayloadBuilder", "build", true),
            vec![("PayloadBuilder".to_string(), false)],
            "extend self owner resolution should map singleton call to instance method"
        );

        registry.apply_cli_resolution();
        let propagated_sites = registry.get_call_sites("PayloadBuilder");
        assert!(
            propagated_sites
                .iter()
                .any(|site| site.method_name.as_ref() == "build" && !site.method_is_singleton),
            "expected propagated instance call site after CLI resolution, got {propagated_sites:#?}"
        );
        let propagated_build_site = propagated_sites
            .iter()
            .find(|site| site.method_name.as_ref() == "build" && !site.method_is_singleton)
            .expect("propagated build site");
        assert_eq!(
            propagated_build_site.arg_types[0].to_string(),
            "Alert",
            "expected propagated build arg type to stay concrete"
        );

        let build = registry
            .lookup_method_sig_exact("PayloadBuilder", "build", false)
            .expect("builder signature");
        let attrs = registry
            .lookup_method_sig_exact("PayloadBuilder", "attrs", false)
            .expect("attrs signature");
        let payload = registry
            .lookup_method_sig_exact("Alert", "payload", false)
            .expect("payload signature");

        assert_eq!(build.params[0].param_type.to_string(), "Alert");
        assert_eq!(attrs.params[0].param_type.to_string(), "Alert");
        assert_eq!(
            build.return_type.to_string(),
            "{ title: \"critical\", count: Integer }"
        );
        assert_eq!(
            attrs.return_type.to_string(),
            "{ title: \"critical\", count: Integer }"
        );
        assert_eq!(
            payload.return_type.to_string(),
            "{ title: \"critical\", count: Integer }"
        );
    }

    #[test]
    fn compact_cli_resolution_resolves_cross_file_struct_prop_scoped_ref() {
        // Regression test: prop type args prefer lexical scope (nested over decoy), resolved by the post-merge finalize write-back.
        let loader = playground_loader();
        let decl_source = concat!(
            "class Foo\n",
            "end\n",
            "\n",
            "module A\n",
            "  class B < T::Struct\n",
            "    prop :foo, Foo\n",
            "  end\n",
            "end\n",
        );
        let nested_source = concat!(
            "module A\n",
            "  class B < T::Struct\n",
            "    class Foo < T::Struct\n",
            "      prop :x, Integer\n",
            "    end\n",
            "  end\n",
            "end\n",
        );
        let decl = analyze_compact_file_snapshot_timed(
            decl_source,
            None,
            &loader,
            None,
            "app/models/b.rb",
            AnalysisOptions::default(),
            true,
        )
        .0;
        let nested = analyze_compact_file_snapshot_timed(
            nested_source,
            None,
            &loader,
            None,
            "app/models/b/foo.rb",
            AnalysisOptions::default(),
            true,
        )
        .0;

        let mut registry = TypeRegistry::new();
        decl.apply_to_registry(&mut registry);
        nested.apply_to_registry(&mut registry);
        registry.apply_cli_resolution();

        let reader = registry
            .lookup_method_sig_exact("A::B", "foo", false)
            .expect("prop reader");
        let writer = registry
            .lookup_method_sig_exact("A::B", "foo=", false)
            .expect("prop writer");
        assert_eq!(
            reader.return_type.to_string(),
            "A::B::Foo",
            "reader must resolve to lexically-scoped nested Foo, not top-level Foo"
        );
        assert_eq!(
            writer.return_type.to_string(),
            "A::B::Foo",
            "writer return type must resolve to nested Foo"
        );
        assert_eq!(
            writer.params[0].param_type.to_string(),
            "A::B::Foo",
            "writer param type must resolve to nested Foo"
        );
    }

    fn playground_loader() -> LazyRbsLoader {
        LazyRbsLoader::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core"))
    }

    #[test]
    fn syntax_error_suppressor_flags_only_value_corrupting_method() {
        // `def b = "#{a}` is an unterminated string (value-corrupting); `a` is fine.
        let source = "class A\n  def a = 1\n\n  def b = \"#{a}\nend\n";
        let suppressor = SyntaxErrorSuppressor::new(source, &[2, 4]);
        assert!(suppressor.is_active());
        assert!(!suppressor.suppresses_def_line(2), "a must stay visible");
        assert!(suppressor.suppresses_def_line(4), "b must be suppressed");
        // A diagnostic on b's line is suppressed; one on a's line is not.
        assert!(suppressor.suppresses_line(4));
        assert!(!suppressor.suppresses_line(2));
    }

    #[test]
    fn syntax_error_suppressor_ignores_incremental_end_errors() {
        // Mid-typing: unclosed `def`/`class` should NOT suppress (no garbage).
        let source = "class Sample\n  def foo(x)\n";
        let suppressor = SyntaxErrorSuppressor::new(source, &[2]);
        assert!(!suppressor.is_active());
        assert!(!suppressor.suppresses_def_line(2));
    }

    #[test]
    fn playground_hover_for_method_call_is_not_double_prefixed() {
        let loader = playground_loader();
        let source = "class A\n  def a = rand(1) < 0.5 ? :a : \"2\"\n\n  def b\n    return 10 if a == :a\n    a\n  end\nend\n";
        let res = playground_analyze(source, "", &loader, "probe.rb");
        let call_hover = res
            .hovers
            .iter()
            .find(|h| h.name == "a" && h.line == 5)
            .expect("hover on the `a` call in the condition");
        // Mirrors the LSP/editor body exactly: no `a: a:` double prefix.
        assert_eq!(call_hover.display, "[Tyda] -> \"2\" | :a");
    }

    #[test]
    fn playground_mixed_map_inference_preserves_union_types() {
        let loader = playground_loader();
        let source = "class User\n  def ids = [1, 2, \"3\"].map { |n| n * 2 }\nend\n";
        let res = playground_analyze(source, "", &loader, "probe.rb");
        assert_eq!(
            res.rbs,
            "class User\n  def ids: -> Array[Integer | String]\nend\n"
        );
        assert_eq!(
            res.code_lens
                .iter()
                .map(|lens| (lens.line, lens.signature.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, "-> Array[Integer | String]")]
        );
        assert_eq!(
            res.hovers
                .iter()
                .filter(|h| h.name == "n")
                .map(|h| h.display.as_str())
                .collect::<Vec<_>>(),
            vec!["[Tyda] 1 | 2 | \"3\"", "[Tyda] 1 | 2 | \"3\""]
        );
    }

    #[test]
    fn playground_ivar_write_and_read_have_hovers() {
        let loader = playground_loader();
        let source = concat!(
            "class User\n",
            "  #: (String) -> void\n",
            "  def initialize(name)\n",
            "    @name = name\n",
            "  end\n",
            "\n",
            "  def name = @name\n",
            "end\n",
        );
        let res = playground_analyze(source, "", &loader, "probe.rb");
        assert_eq!(
            res.rbs,
            "class User\n  def initialize: (String name) -> void\n  def name: -> String\nend\n"
        );
        for (line, column) in [(4, 4), (7, 13)] {
            let hover = res
                .hovers
                .iter()
                .find(|h| h.line == line && h.column == column)
                .unwrap_or_else(|| panic!("expected hover at ({line}:{column})"));
            assert_eq!(hover.name, "@name");
            assert_eq!(hover.display, "[Tyda] String");
        }
        let method_hover = res
            .hovers
            .iter()
            .find(|h| h.line == 7 && h.column == 6)
            .expect("expected hover on the name method");
        assert_eq!(method_hover.name, "name");
        assert_eq!(method_hover.display, "[Tyda] -> String");
    }

    #[test]
    fn playground_suppresses_codelens_for_broken_method_only() {
        let loader = playground_loader();
        let source = "class A\n  def a = rand(1) < 0.5 ? :a : \"2\"\n\n  def b = \"#{a}\nend\n";
        let res = playground_analyze(source, "", &loader, "probe.rb");
        assert!(res.ruby_syntax_error);
        let lines: Vec<u32> = res.code_lens.iter().map(|c| c.line).collect();
        assert_eq!(
            lines,
            vec![2],
            "only the well-formed method `a` keeps its codelens; broken `b` is suppressed"
        );
    }

    #[test]
    fn playground_suppresses_hover_inside_broken_method_only() {
        let loader = playground_loader();
        let source = "class A\n  def a = rand(1) < 0.5 ? :a : \"2\"\n\n  def b = \"#{a}\nend\n";
        let res = playground_analyze(source, "", &loader, "probe.rb");
        // The broken method `b` (line 4) yields garbage types — no hover there.
        assert!(
            !res.hovers.iter().any(|h| h.line == 4),
            "no hover inside the broken method b (L4): {:?}",
            res.hovers
                .iter()
                .map(|h| (h.line, h.display.clone()))
                .collect::<Vec<_>>()
        );
        // The well-formed method `a` (line 2) still hovers normally.
        assert!(res.hovers.iter().any(|h| h.line == 2));
    }
}
