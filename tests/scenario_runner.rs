use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;
use tempfile::TempDir;
use walkdir::WalkDir;

use tyda::analysis::{
    AnalysisOptions, analyze_file_facts_with_deps, analyze_source_cached_with_deps_lazy,
};
use tyda::project::{DslActivation, DslLibrary, ProjectVersions};
use tyda::rails::load_project_types_with_activation;
use tyda::rbs::import::load_rbs_string;
use tyda::rbs::render::{RenderOptions, render_rbs_with_options};
use tyda::rbs::stdlib_loader::LazyRbsLoader;
use tyda::registry::TypeRegistry;
use tyda::scenario::{ScenarioConfig, Step, parse_scenario_file};
use tyda::sorbet::rbi::merge_rbi_source_into_registry;
use tyda::workspace_state::{WorkspaceState, hash_content};

fn normalize_whitespace(s: &str) -> String {
    s.lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

struct TestResult {
    file_name: String,
    case_name: String,
    step: usize,
    error: String,
}

struct PreparedStepContext {
    registry: TypeRegistry,
    _tempdir: Option<TempDir>,
}

fn run_scenario_file(
    path: &Path,
    stdlib_loader: &LazyRbsLoader,
    rails_mode: bool,
    dsl_mode: bool,
) -> FileRun {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));

    let file_name = path
        .strip_prefix("tests/scenarios/")
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let scenario_file = parse_scenario_file(&file_name, &content);
    let case_count = scenario_file.cases.len();
    let mut failures = Vec::new();
    let mut unexpected_passes = Vec::new();
    let mut known_open = 0usize;
    let path_known_issue = is_known_issue_path(&file_name);

    if scenario_file.cases.is_empty() {
        failures.push(TestResult {
            file_name: file_name.clone(),
            case_name: String::new(),
            step: 0,
            error: "No test cases found".to_string(),
        });
        return FileRun {
            case_count: 0,
            known_open: 0,
            failures,
            unexpected_passes,
        };
    }

    for case in &scenario_file.cases {
        let known_issue = path_known_issue || case.config.known_issue;
        for (i, step) in case.steps.iter().enumerate() {
            let prepared =
                prepare_step_context(&case.config, step, stdlib_loader, rails_mode, dsl_mode);
            let actual_rbs = render_rbs_with_options(
                &prepared.registry,
                RenderOptions {
                    include_synthetic_dsl_methods: case.config.include_synthetic_dsl_methods,
                },
            );

            let expected = normalize_whitespace(&step.expected_rbs);
            let actual = normalize_whitespace(&actual_rbs);

            if expected == actual {
                if known_issue {
                    unexpected_passes.push(TestResult {
                        file_name: file_name.clone(),
                        case_name: case.name.clone(),
                        step: i + 1,
                        error: "\nknown-issue case now matches expected RBS; move it out of known-issues/ or drop `known_issue: true`".to_string(),
                    });
                }
            } else if known_issue {
                known_open += 1;
            } else {
                failures.push(TestResult {
                    file_name: file_name.clone(),
                    case_name: case.name.clone(),
                    step: i + 1,
                    error: format!(
                        "\n=== Expected RBS ===\n{expected}\n\n=== Actual RBS ===\n{actual}"
                    ),
                });
            }
        }
    }

    FileRun {
        case_count,
        known_open,
        failures,
        unexpected_passes,
    }
}

fn is_known_issue_path(file_name: &str) -> bool {
    file_name == "known-issues" || file_name.starts_with("known-issues/")
}

struct FileRun {
    case_count: usize,
    known_open: usize,
    failures: Vec<TestResult>,
    unexpected_passes: Vec<TestResult>,
}

/// Prepare the current-file registry for one scenario step.
///
/// A scenario case is a tiny workspace: it builds a fresh [`WorkspaceState`]
/// per step, projects any context files into the shared workspace registry,
/// and analyzes the current file against that projection — the same
/// display-analysis contract the LSP uses (workspace projection as context,
/// current file solved at full depth). The `WorkspaceState` is dropped when the
/// step finishes.
fn prepare_step_context(
    config: &ScenarioConfig,
    step: &Step,
    stdlib_loader: &LazyRbsLoader,
    rails_mode: bool,
    dsl_mode: bool,
) -> PreparedStepContext {
    let external_rbs = build_external_registry(step, stdlib_loader);
    if step.project_files.is_empty() {
        return prepare_standalone_step_context(
            config,
            step,
            external_rbs,
            stdlib_loader,
            rails_mode,
            dsl_mode,
        );
    }

    prepare_project_backed_step_context(
        config,
        step,
        external_rbs,
        stdlib_loader,
        rails_mode,
        dsl_mode,
    )
}

fn build_external_registry(step: &Step, stdlib_loader: &LazyRbsLoader) -> Option<TypeRegistry> {
    let mut rbs_registry = if let Some(ref rbs_input) = step.rbs_input {
        let mut reg = TypeRegistry::new();
        load_rbs_string(rbs_input, &mut reg);
        Some(reg)
    } else {
        None
    };

    if let Some(ref rbi_input) = step.rbi_input {
        let reg = rbs_registry.get_or_insert_with(TypeRegistry::new);
        merge_rbi_source_into_registry(rbi_input, reg, stdlib_loader);
    }

    rbs_registry
}

fn prepare_standalone_step_context(
    config: &ScenarioConfig,
    step: &Step,
    external_rbs: Option<TypeRegistry>,
    stdlib_loader: &LazyRbsLoader,
    rails_mode: bool,
    dsl_mode: bool,
) -> PreparedStepContext {
    let source_path = "scenario.rb".to_string();
    let analysis_options = scenario_analysis_options(config, rails_mode, dsl_mode, None);
    // Empty workspace: the projection is the external RBS/RBI base resolved
    // through the shared backend, then used as the current-file context.
    let lazy_rbs_merge = external_rbs.is_none();
    let base = external_rbs.unwrap_or_default();
    let mut workspace_state = WorkspaceState::new();
    let projection = workspace_state.workspace_registry(&base);
    let (analysis, _, _) = analyze_source_cached_with_deps_lazy(
        &step.ruby_code,
        Some(&projection),
        Some(stdlib_loader),
        None,
        Some(&source_path),
        analysis_options,
        lazy_rbs_merge,
    );
    PreparedStepContext {
        registry: analysis.registry().clone(),
        _tempdir: None,
    }
}

fn prepare_project_backed_step_context(
    config: &ScenarioConfig,
    step: &Step,
    base_registry: Option<TypeRegistry>,
    stdlib_loader: &LazyRbsLoader,
    rails_mode: bool,
    dsl_mode: bool,
) -> PreparedStepContext {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("config")).expect("mkdir config");
    std::fs::create_dir_all(dir.path().join("db")).expect("mkdir db");
    std::fs::create_dir_all(dir.path().join("app/models")).expect("mkdir app/models");
    if !step
        .project_files
        .iter()
        .any(|file| file.path == "config/application.rb")
    {
        std::fs::write(
            dir.path().join("config/application.rb"),
            "module Dummy; class Application; end; end\n",
        )
        .expect("write application.rb");
    }
    for file in &step.project_files {
        let path = dir.path().join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir project file parent");
        }
        std::fs::write(path, &file.content).expect("write project file");
    }
    let source_path = dir.path().join("app/models/scenario.rb");
    std::fs::write(&source_path, &step.ruby_code).expect("write scenario.rb");

    let mut user_rbs = base_registry.unwrap_or_default();
    let base_options =
        scenario_analysis_options(config, rails_mode, dsl_mode, Some(dir.path().to_path_buf()));
    let activation = base_options.dsl_activation.clone();
    let project_versions = base_options.project_versions;
    let project_root = base_options.project_root.clone();
    let rails_enabled = load_project_types_with_activation(dir.path(), &mut user_rbs, &activation);

    let opts = AnalysisOptions {
        rails_mode: base_options.rails_mode || rails_enabled,
        dsl_activation: activation.clone(),
        project_versions,
        project_root: project_root.clone(),
    };

    // Project the context `.rb` files into the shared workspace registry, then
    // analyze the current file against that projection. This is the same
    // WorkspaceState backend the CLI and LSP use; the manual per-file merge loop
    // is gone.
    let mut workspace_state = WorkspaceState::new();
    for file in &step.project_files {
        if file.path.ends_with(".rb") {
            let file_path = dir.path().join(&file.path);
            let (snapshot, deps) = analyze_file_facts_with_deps(
                &file.content,
                Some(&user_rbs),
                Some(stdlib_loader),
                file_path.to_str(),
                opts.clone(),
            );
            workspace_state.upsert_file(
                file_path.to_string_lossy().into_owned(),
                hash_content(&file.content),
                snapshot,
                deps,
            );
        }
    }
    let projection = workspace_state.workspace_registry(&user_rbs);

    let (analysis, _, _) = analyze_source_cached_with_deps_lazy(
        &step.ruby_code,
        Some(&projection),
        Some(stdlib_loader),
        None,
        Some(source_path.to_str().expect("scenario path")),
        opts,
        false,
    );
    PreparedStepContext {
        registry: analysis.registry().clone(),
        _tempdir: Some(dir),
    }
}

fn scenario_analysis_options(
    config: &ScenarioConfig,
    rails_mode: bool,
    dsl_mode: bool,
    project_root: Option<PathBuf>,
) -> AnalysisOptions {
    let project_versions = ProjectVersions {
        ruby: config.ruby_version,
        rails: config.rails_version,
    };

    let rails_enabled = rails_mode || config.rails_version.is_some();
    let dsl_activation = if dsl_mode {
        let mut activation = DslActivation::default();
        activation
            .auto_detected
            .extend(DslLibrary::official_builtins().iter().copied());
        activation
    } else if rails_enabled {
        DslActivation::with_rails_mode(true)
    } else {
        DslActivation::default()
    };

    AnalysisOptions {
        rails_mode: if dsl_mode {
            dsl_activation.rails_mode_compat() || rails_enabled
        } else {
            rails_enabled
        },
        dsl_activation,
        project_versions,
        project_root,
    }
}

#[test]
fn test_all_scenarios() {
    let scenario_dir = Path::new("tests/scenarios");
    if !scenario_dir.exists() {
        panic!("Scenario directory not found: {}", scenario_dir.display());
    }

    let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");

    let filter = std::env::var("TYDA_SCENARIO_FILTER").ok();
    let md_files: Vec<PathBuf> = WalkDir::new(scenario_dir)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .map(|e| e.path().to_path_buf())
        .filter(|p| {
            filter.as_ref().is_none_or(|f| {
                p.strip_prefix(scenario_dir)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .contains(f)
            })
        })
        .collect();

    if let Some(f) = &filter {
        assert!(
            !md_files.is_empty(),
            "TYDA_SCENARIO_FILTER={f} matched no files under tests/scenarios/"
        );
        eprintln!("TYDA_SCENARIO_FILTER={f}: {} file(s)", md_files.len());
    }

    let total_files = md_files.len();
    let total_cases = AtomicUsize::new(0);
    let rails_dsl_dir = Path::new("rails").join("dsl");

    let all_results: Vec<FileRun> = md_files
        .par_iter()
        .map(|path| {
            let loader = LazyRbsLoader::new(core_dir.clone());
            let scenario_path = path.strip_prefix(scenario_dir).unwrap_or(path);
            let rails_mode = scenario_path.starts_with("rails");
            let dsl_mode = scenario_path.starts_with(&rails_dsl_dir);
            run_scenario_file(path, &loader, rails_mode, dsl_mode)
        })
        .collect();

    let mut all_failures: Vec<TestResult> = Vec::new();
    let mut unexpected_passes: Vec<TestResult> = Vec::new();
    let mut known_open = 0usize;
    for run in all_results {
        total_cases.fetch_add(run.case_count, Ordering::Relaxed);
        known_open += run.known_open;
        all_failures.extend(run.failures);
        unexpected_passes.extend(run.unexpected_passes);
    }

    let total_cases = total_cases.load(Ordering::Relaxed);

    if !all_failures.is_empty() || !unexpected_passes.is_empty() {
        all_failures.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        unexpected_passes.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        let mut msg = format!(
            "\n{} failure(s), {} unexpected known-issue pass(es) in {total_cases} cases across {total_files} files:\n",
            all_failures.len(),
            unexpected_passes.len()
        );
        for f in all_failures.iter().chain(unexpected_passes.iter()) {
            msg.push_str(&format!(
                "\n--- {} > {} (step {}) ---{}",
                f.file_name, f.case_name, f.step, f.error
            ));
        }
        panic!("{msg}");
    }

    assert!(total_cases > 0, "No test cases found");
    if known_open == 0 {
        eprintln!("{total_cases} cases in {total_files} files passed");
    } else {
        eprintln!(
            "{total_cases} cases in {total_files} files passed ({known_open} known-issue cases still open)"
        );
    }
}
