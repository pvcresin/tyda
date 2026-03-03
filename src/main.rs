use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::{Parser as ClapParser, Subcommand};
use rayon::prelude::*;

use tyda::analysis::{
    AnalysisOptions, AnalysisTimings, analyze_cli_diagnostic_target_snapshot_timed,
    analyze_compact_file_snapshot_timed, analyze_definitions_only_snapshot_timed,
    analyze_file_registry_timed,
};
use tyda::diagnostics::{
    TypeDiagnostic, TypeHoleSummary, build_scenario_seed, summarize_type_holes,
};
use tyda::inference::FileAnalysisSnapshot;
use tyda::project::{DslActivation, DslActivationSource, DslFamily, DslLibrary, ProjectVersions};
use tyda::rbs::render::{RenderOptions, render_rbs_to_writer_in_pool, render_rbs_with_options};
use tyda::rbs::stdlib_loader::LazyRbsLoader;
use tyda::rbs::workspace::{infer_workspace_root, load_cli_type_environment};
use tyda::sorbet::rbi::LazyRbiLoader;
use tyda::workspace_discovery::{RubyScanScope, collect_rb_files_from_roots_with_scope};

#[cfg(unix)]
fn max_rss_mb() -> f64 {
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) != 0 {
            return 0.0;
        }
        #[cfg(target_os = "macos")]
        let bytes = usage.ru_maxrss as f64;
        #[cfg(not(target_os = "macos"))]
        let bytes = usage.ru_maxrss as f64 * 1024.0;
        bytes / (1024.0 * 1024.0)
    }
}
#[cfg(not(unix))]
fn max_rss_mb() -> f64 {
    0.0
}

/// Live resident bytes, reported next to the `getrusage` high-water mark: a peak
/// alone cannot separate live data from freed-but-still-mapped pages.
#[cfg(target_os = "macos")]
fn current_rss_mb() -> f64 {
    unsafe {
        let mut info: libc::proc_taskinfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
        let written = libc::proc_pidinfo(
            std::process::id() as libc::c_int,
            libc::PROC_PIDTASKINFO,
            0,
            (&mut info as *mut libc::proc_taskinfo).cast(),
            size,
        );
        if written != size {
            return 0.0;
        }
        info.pti_resident_size as f64 / (1024.0 * 1024.0)
    }
}
#[cfg(all(unix, not(target_os = "macos")))]
fn current_rss_mb() -> f64 {
    let Ok(statm) = fs::read_to_string("/proc/self/statm") else {
        return 0.0;
    };
    let Some(pages) = statm
        .split_whitespace()
        .nth(1)
        .and_then(|field| field.parse::<f64>().ok())
    else {
        return 0.0;
    };
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as f64;
    pages * page_size / (1024.0 * 1024.0)
}
#[cfg(not(unix))]
fn current_rss_mb() -> f64 {
    0.0
}

#[derive(ClapParser)]
#[command(name = "tyda", about = "Fast Ruby type inference engine")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(
        long,
        short = 'v',
        global = true,
        help = "Print version (TypeProf-compatible format)"
    )]
    version: bool,

    #[arg(
        long,
        global = true,
        help = "Start as LSP server (TypeProf-compatible TCP mode)"
    )]
    lsp: bool,

    #[arg(
        long,
        global = true,
        help = "Debug mode: print per-file analysis timings"
    )]
    debug: bool,

    #[arg(
        long,
        global = true,
        help = "Print type diagnostics as JSON Lines instead of rendered RBS"
    )]
    diagnostics: bool,

    #[arg(
        long,
        global = true,
        help = "Verbose mode: print each file name before processing (useful for finding hangs)"
    )]
    verbose: bool,

    #[arg(
        long,
        global = true,
        help = "Enable/disable DSL detectors and collectors (e.g. auto,+aasm,-protobuf)"
    )]
    dsl: Option<String>,

    #[arg(
        long,
        global = true,
        help = "Print the maintained capability matrix and exit"
    )]
    capability_matrix: bool,

    #[arg(
        long,
        global = true,
        help = "Include synthetic DSL/framework methods in rendered RBS"
    )]
    include_synthetic_dsl_methods: bool,

    #[arg(help = "Files or directories to analyze (CLI mode)")]
    paths: Vec<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    Lsp {
        #[arg(long, help = "Print version in TypeProf format and exit")]
        version: bool,

        #[arg(long, help = "Start LSP server")]
        lsp: bool,
    },
}

#[derive(Clone, Copy)]
struct CliRunOptions {
    debug: bool,
    diagnostics: bool,
    verbose: bool,
    include_synthetic_dsl_methods: bool,
}

#[derive(Clone)]
struct FileTiming {
    path: String,
    elapsed: Duration,
    analysis: AnalysisTimings,
    render: Duration,
    holes: TypeHoleSummary,
    scenario_seed: Option<String>,
}

struct CliRunSummary {
    total_files: usize,
    elapsed: Duration,
    file_timings: Vec<FileTiming>,
    dsl_activation: DslActivation,
    project_versions: ProjectVersions,
}

struct FileAnalysisResult {
    index: usize,
    output: String,
    timing: FileTiming,
}

struct FileDiagnosticResult {
    index: usize,
    diagnostics: Vec<TypeDiagnostic>,
}

struct CliAnalysisContext<'a> {
    user_rbs: &'a tyda::registry::TypeRegistry,
    stdlib_loader: &'a LazyRbsLoader,
    lazy_rbi_loader: Option<&'a LazyRbiLoader>,
    rails_mode: bool,
    dsl_activation: DslActivation,
    project_versions: ProjectVersions,
    workspace_root: std::path::PathBuf,
    debug: bool,
    include_synthetic_dsl_methods: bool,
}

const CLI_COMPACT_ANALYSIS_CHUNK_SIZE: usize = 32;

fn skeleton_scan_chunk_size(analysis_threads: usize) -> usize {
    analysis_threads.saturating_mul(16).clamp(64, 256)
}

const TYDA_VERSION: &str = env!("TYDA_VERSION");

// vscode-typeprof validates the whole `--version` output against `^typeprof X.Y.Z$` (>=0.30.1). Revisit if tyda gets its own editor integration.
const TYPEPROF_COMPAT_VERSION: &str = "0.30.1";

fn print_version() {
    println!("typeprof {TYPEPROF_COMPAT_VERSION}");
}

fn main() {
    let cli = Cli::parse();

    if cli.version {
        print_version();
        return;
    }

    if cli.lsp {
        run_lsp_server();
        return;
    }

    if cli.capability_matrix {
        println!("{}", include_str!("../docs/capability-matrix.md"));
        return;
    }

    match cli.command {
        Some(Commands::Lsp { version, lsp }) => {
            if version {
                print_version();
                return;
            }
            if lsp {
                run_lsp_server();
                return;
            }
            eprintln!("Usage: tyda lsp --version | tyda lsp --lsp");
            std::process::exit(1);
        }
        None => {
            if cli.paths.is_empty() {
                eprintln!(
                    "Usage: tyda <paths...> | tyda --verbose <path> | tyda --debug <path> | tyda --diagnostics <path> | tyda --include-synthetic-dsl-methods <path> | tyda --capability-matrix | tyda --lsp | tyda --version"
                );
                std::process::exit(1);
            }
            run_cli(
                &cli.paths,
                CliRunOptions {
                    debug: cli.debug,
                    diagnostics: cli.diagnostics,
                    verbose: cli.verbose,
                    include_synthetic_dsl_methods: cli.include_synthetic_dsl_methods,
                },
                cli.dsl.as_deref(),
            );
        }
    }
}

fn run_lsp_server() {
    use std::io::Write;
    use std::net::TcpListener;
    use std::process;
    use tokio::runtime::Runtime;
    use tower_lsp::{LspService, Server};
    use tyda::lsp::TydaLsp;

    // Align the LSP global rayon pool with the analysis pool at cores-2, so UI/tokio isn't starved.
    let cores = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .max(1);
    rayon::ThreadPoolBuilder::new()
        .num_threads(cores.saturating_sub(2).max(2))
        .stack_size(tyda::analysis::ANALYSIS_WORKER_STACK_SIZE)
        .build_global()
        .ok();

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind TCP socket");
    let addr = listener.local_addr().expect("Failed to get local address");
    let parent_pid = current_parent_pid();

    let startup_json = serde_json::json!({
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "pid": std::process::id(),
    });
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{startup_json}").expect("Failed to write startup JSON");
    stdout.flush().expect("Failed to flush stdout");

    drop(stdout);

    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(async {
        let (stream, _addr) = listener.accept().expect("Failed to accept connection");
        let monitor_stream = stream.try_clone().expect("Failed to clone TCP stream");
        let stream =
            tokio::net::TcpStream::from_std(stream).expect("Failed to convert to tokio TcpStream");
        let (read, write) = tokio::io::split(stream);

        let (service, socket) = LspService::new(TydaLsp::new);
        let server = Server::new(read, write, socket).serve(service);
        let socket_closed =
            tokio::task::spawn_blocking(move || wait_for_socket_close(monitor_stream));

        tokio::select! {
            _ = server => {}
            _ = socket_closed => {
                process::exit(0);
            }
            _ = wait_for_parent_exit(parent_pid) => {
                process::exit(0);
            }
            _ = wait_for_termination_signal() => {
                process::exit(0);
            }
        }
    });
}

#[cfg(unix)]
fn current_parent_pid() -> u32 {
    unsafe { libc::getppid() as u32 }
}

#[cfg(not(unix))]
fn current_parent_pid() -> u32 {
    0
}

#[cfg(unix)]
async fn wait_for_parent_exit(parent_pid: u32) {
    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let current = current_parent_pid();
        if current == 1 || current != parent_pid {
            return;
        }
    }
}

#[cfg(not(unix))]
async fn wait_for_parent_exit(_parent_pid: u32) {
    std::future::pending::<()>().await;
}

fn wait_for_socket_close(stream: std::net::TcpStream) {
    let _ = stream.set_nonblocking(true);
    let mut buf = [0_u8; 1];

    loop {
        match stream.peek(&mut buf) {
            Ok(0) => return,
            Ok(_) => std::thread::sleep(Duration::from_millis(250)),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        }
    }
}

#[cfg(unix)]
async fn wait_for_termination_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    let mut sighup = signal(SignalKind::hangup()).expect("Failed to register SIGHUP handler");

    tokio::select! {
        _ = sigint.recv() => {}
        _ = sigterm.recv() => {}
        _ = sighup.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_termination_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn run_cli(paths: &[PathBuf], options: CliRunOptions, dsl_spec: Option<&str>) {
    use std::io::{BufWriter, Write};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let start = Instant::now();
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let show_preload_progress = is_tty && !options.debug && !options.verbose;
    let preload_started = Instant::now();
    let preload_stop = Arc::new(AtomicBool::new(false));
    let preload_handle = if show_preload_progress {
        eprintln!("Loading external types...");
        let stop_clone = preload_stop.clone();
        let started_at = preload_started;
        let handle = std::thread::spawn(move || {
            const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0;
            while !stop_clone.load(Ordering::Relaxed) {
                let elapsed = started_at.elapsed().as_secs_f64();
                let frame = FRAMES[i % FRAMES.len()];
                eprint!("\r{frame} Loading external types... {:.1}s", elapsed);
                std::io::stderr().flush().ok();
                std::thread::sleep(Duration::from_millis(80));
                i += 1;
            }
        });
        Some(handle)
    } else {
        None
    };

    let paths_owned: Vec<PathBuf> = paths.to_vec();
    // Diagnostics needs every target source again after analysis (message
    // rendering reads it); the RBS path re-reads per chunk instead of holding
    // the whole corpus resident.
    let retain_sources = options.diagnostics;
    let file_collect_handle = std::thread::spawn(move || {
        let files = collect_analysis_files(&paths_owned);
        // par_iter over an indexed source preserves input order in the collected Vec.
        let scanned: Vec<(PathBuf, Option<String>, BTreeSet<DslLibrary>)> = files
            .into_par_iter()
            .filter_map(|path| {
                if !is_ruby_file(&path) {
                    return None;
                }
                let source = fs::read_to_string(&path).ok()?;
                let mut detected = BTreeSet::new();
                tyda::project::detect_dsl_from_source_text(&source, &mut detected);
                Some((path, retain_sources.then_some(source), detected))
            })
            .collect();
        let mut file_sources: Vec<(PathBuf, Option<String>)> = Vec::with_capacity(scanned.len());
        let mut detected_dsl = BTreeSet::new();
        for (path, source, detected) in scanned {
            file_sources.push((path, source));
            detected_dsl.extend(detected);
        }
        (file_sources, detected_dsl)
    });

    let preload_timing = std::env::var_os("TYDA_PRELOAD_TIMING").is_some();
    let (
        workspace_root,
        stdlib_loader,
        user_rbs,
        lazy_rbi_loader,
        rails_mode,
        mut dsl_activation,
        project_versions,
    ) = build_cli_context(paths, options.debug, dsl_spec);
    let build_cli_context_elapsed = preload_started.elapsed();
    let (file_sources, detected_dsl) = file_collect_handle
        .join()
        .expect("file collection thread panicked");
    let file_collect_joined_elapsed = preload_started.elapsed();

    let dsl_detect_started = Instant::now();
    dsl_activation.auto_detected.extend(detected_dsl);
    let dsl_detect_elapsed = dsl_detect_started.elapsed();
    let preload_elapsed = preload_started.elapsed();
    if preload_timing {
        eprintln!(
            "TIMING preload build_cli_context_ms={:.3} file_collect_joined_at_ms={:.3} dsl_detect_ms={:.3} total_preload_ms={:.3}",
            build_cli_context_elapsed.as_secs_f64() * 1000.0,
            file_collect_joined_elapsed.as_secs_f64() * 1000.0,
            dsl_detect_elapsed.as_secs_f64() * 1000.0,
            preload_elapsed.as_secs_f64() * 1000.0,
        );
    }
    let total_files = file_sources.len();
    let (file_paths, diagnostic_sources): (Vec<PathBuf>, Vec<(PathBuf, String)>) =
        if options.diagnostics {
            (
                Vec::new(),
                file_sources
                    .into_iter()
                    .filter_map(|(path, source)| Some((path, source?)))
                    .collect(),
            )
        } else {
            (
                file_sources.into_iter().map(|(path, _)| path).collect(),
                Vec::new(),
            )
        };
    if let Some(handle) = preload_handle {
        preload_stop.store(true, Ordering::Relaxed);
        let _ = handle.join();
        eprint!("\r\x1b[2K");
        eprintln!(
            "Loaded external types in {:.1}s. Analyzing {} files...",
            preload_elapsed.as_secs_f64(),
            total_files
        );
    }

    let stop = Arc::new(AtomicBool::new(false));
    let processed_count = Arc::new(AtomicUsize::new(0));
    let progress_handle = if should_show_progress(is_tty, total_files, options) {
        let stop_clone = stop.clone();
        let count_clone = processed_count.clone();
        let started_at = Instant::now();
        let handle = std::thread::spawn(move || {
            const BAR_WIDTH: usize = 50;
            const DELAY: Duration = Duration::from_millis(300);
            const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0;
            let mut shown = false;
            while !stop_clone.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(80));
                if started_at.elapsed() < DELAY {
                    continue;
                }
                shown = true;
                let done = count_clone.load(Ordering::Relaxed);
                let pct = done * 100 / total_files.max(1);
                let filled = done * BAR_WIDTH / total_files.max(1);
                let empty = BAR_WIDTH - filled;
                let frame = FRAMES[i % FRAMES.len()];
                eprint!(
                    "\r{frame} [{}{}>] {done}/{total_files} ({pct}%)",
                    "█".repeat(filled),
                    "░".repeat(empty),
                );
                std::io::stderr().flush().ok();
                i += 1;
            }
            shown
        });
        Some(handle)
    } else {
        None
    };

    let mut file_timings = Vec::new();
    let stdout = std::io::stdout();
    let mut stdout = BufWriter::new(stdout.lock());
    let opts = AnalysisOptions {
        rails_mode,
        dsl_activation: dsl_activation.clone(),
        project_versions,
        project_root: Some(workspace_root.clone()),
    };

    let mut analysis_threads = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .max(1);
    // Cap for benchmark / verification runs that must not monopolize the
    // machine (CPU heat and per-worker transient memory scale with workers).
    if let Some(value) = std::env::var_os("TYDA_CLI_ANALYSIS_THREADS")
        && let Some(parsed) = value.to_str().and_then(|v| v.parse::<usize>().ok())
        && parsed > 0
    {
        analysis_threads = parsed;
    }
    let pool_build_started = Instant::now();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(analysis_threads)
        .stack_size(tyda::analysis::ANALYSIS_WORKER_STACK_SIZE)
        .build()
        .expect("failed to build CLI analysis thread pool");
    if preload_timing {
        eprintln!(
            "TIMING pool_build_ms={:.3} analysis_threads={analysis_threads}",
            pool_build_started.elapsed().as_secs_f64() * 1000.0,
        );
    }

    let mut compact_collection_elapsed = Duration::ZERO;
    let mut context_scan_elapsed = Duration::ZERO;
    let mut merge_elapsed = Duration::ZERO;
    let compact_scan_dsl_activation = dsl_activation.clone();
    let compact_scan_project_versions = project_versions;
    let compact_scan_workspace_root = workspace_root.clone();
    let base_user_rbs = user_rbs;
    let scan_job_timing = std::env::var_os("TYDA_SCAN_JOB_TIMING").is_some();
    let memory_breakdown = std::env::var_os("TYDA_MEMORY_BREAKDOWN").is_some();
    if memory_breakdown {
        let b = base_user_rbs.memory_breakdown();
        eprintln!(
            "[mem] preload: rss={:.0}MB live={:.0}MB classes={} methods={} (shared={}) call_sites={} param_cache={}",
            max_rss_mb(),
            current_rss_mb(),
            b.classes,
            b.methods_total,
            b.methods_shared,
            b.call_sites_total,
            b.param_cache_entries,
        );
    }

    let mut context_file_count = 0usize;
    let mut diagnostic_replays: Vec<FileAnalysisSnapshot> = Vec::new();
    let workspace_rbs = if options.diagnostics {
        // Skeleton first (all production files, definitions-only) so target
        // compact+hover sees cross-file ancestors / DSL bases. Judgment then
        // uses those sites — no per-file Full re-analysis.
        let target_paths: std::collections::HashSet<PathBuf> = diagnostic_sources
            .iter()
            .map(|(path, _)| path.canonicalize().unwrap_or_else(|_| path.clone()))
            .collect();
        let context_paths: Vec<PathBuf> = collect_rb_files_from_roots_with_scope(
            std::slice::from_ref(&compact_scan_workspace_root),
            RubyScanScope::Production,
        )
        .into_iter()
        .filter(|path| {
            let key = path.canonicalize().unwrap_or_else(|_| path.clone());
            !target_paths.contains(&key)
        })
        .collect();
        eprintln!(
            "Scanning {} production files as definitions-only skeleton ({} targets, {} context)...",
            diagnostic_sources.len() + context_paths.len(),
            diagnostic_sources.len(),
            context_paths.len(),
        );

        let mut skeleton_jobs: Vec<(&Path, Option<&str>)> =
            Vec::with_capacity(diagnostic_sources.len() + context_paths.len());
        skeleton_jobs.extend(
            diagnostic_sources
                .iter()
                .map(|(path, source)| (path.as_path(), Some(source.as_str()))),
        );
        skeleton_jobs.extend(context_paths.iter().map(|path| (path.as_path(), None)));

        // See the non-diagnostics branch below for why the accumulator needs its
        // own copy of `base_user_rbs` distinct from the analysis-context reference.
        let mut skeleton_builder =
            tyda::workspace_state::BatchProjectionBuilder::new(base_user_rbs.clone());
        for chunk in skeleton_jobs.chunks(skeleton_scan_chunk_size(analysis_threads)) {
            let compact_chunk_start = Instant::now();
            let skeleton_user_rbs = &base_user_rbs;
            let skeleton_lazy_rbi_loader = lazy_rbi_loader.as_ref();
            let analyses = pool.install(|| {
                chunk
                    .par_iter()
                    .filter_map(|(path, source)| {
                        let is_context = source.is_none();
                        let source = match source {
                            Some(source) => Cow::Borrowed(*source),
                            None => Cow::Owned(fs::read_to_string(path).ok()?),
                        };
                        let file_path_str = path.to_string_lossy();
                        let job_start = scan_job_timing.then(Instant::now);
                        let snapshot = analyze_definitions_only_snapshot_timed(
                            source.as_ref(),
                            Some(skeleton_user_rbs),
                            &stdlib_loader,
                            skeleton_lazy_rbi_loader,
                            &file_path_str,
                            opts.clone(),
                        )
                        .0;
                        if let Some(job_start) = job_start {
                            let elapsed = job_start.elapsed();
                            if elapsed.as_millis() >= 500 {
                                eprintln!(
                                    "skeleton-scan-job {file_path_str}: {:.3}s",
                                    elapsed.as_secs_f64()
                                );
                            }
                        }
                        Some((file_path_str.into_owned(), snapshot, is_context))
                    })
                    .collect::<Vec<_>>()
            });
            context_scan_elapsed += compact_chunk_start.elapsed();
            context_file_count += analyses
                .iter()
                .filter(|(_, _, is_context)| *is_context)
                .count();
            let merge_chunk_start = Instant::now();
            skeleton_builder.apply_chunk(
                analyses
                    .into_iter()
                    .map(|(file_path, snapshot, _)| (file_path, snapshot)),
            );
            merge_elapsed += merge_chunk_start.elapsed();
        }
        // Context copy no longer needed; drop it before resolution grows the registry.
        drop(base_user_rbs);

        if memory_breakdown {
            eprintln!(
                "[mem] after-skeleton-scan: rss={:.0}MB live={:.0}MB files_merged={} (+{} context)",
                max_rss_mb(),
                current_rss_mb(),
                skeleton_builder.applied_file_count(),
                context_file_count,
            );
        }

        let resolve_start = Instant::now();
        let skeleton = skeleton_builder.finish(Some(&pool));
        tyda::reclaim_freed_memory(Some(&pool));
        merge_elapsed += resolve_start.elapsed();

        eprintln!(
            "Recording diagnostic sites for {} target files...",
            diagnostic_sources.len()
        );
        diagnostic_replays.reserve(diagnostic_sources.len());
        for chunk in diagnostic_sources.chunks(CLI_COMPACT_ANALYSIS_CHUNK_SIZE) {
            let compact_chunk_start = Instant::now();
            let skeleton_ref = skeleton.as_ref();
            let compact_lazy_rbi_loader = lazy_rbi_loader.as_ref();
            let compact_analyses = pool.install(|| {
                chunk
                    .par_iter()
                    .map(|(path, source)| {
                        let file_path_str = path.to_string_lossy();
                        let job_start = scan_job_timing.then(Instant::now);
                        let snapshot = analyze_cli_diagnostic_target_snapshot_timed(
                            source,
                            Some(skeleton_ref),
                            &stdlib_loader,
                            compact_lazy_rbi_loader,
                            &file_path_str,
                            opts.clone(),
                            true,
                        )
                        .0;
                        if let Some(job_start) = job_start {
                            let elapsed = job_start.elapsed();
                            if elapsed.as_millis() >= 500 {
                                eprintln!(
                                    "scan-job {file_path_str}: {:.3}s",
                                    elapsed.as_secs_f64()
                                );
                            }
                        }
                        (file_path_str.into_owned(), snapshot)
                    })
                    .collect::<Vec<_>>()
            });
            compact_collection_elapsed += compact_chunk_start.elapsed();
            for (_file_path, snapshot) in compact_analyses {
                diagnostic_replays.push(snapshot);
            }
        }

        let resolve_start = Instant::now();
        let skeleton = std::sync::Arc::try_unwrap(skeleton).unwrap_or_else(|arc| (*arc).clone());
        let workspace_rbs = tyda::workspace_state::WorkspaceState::project_borrowed_snapshots(
            skeleton,
            diagnostic_replays.iter(),
            Some(&pool),
        );
        tyda::reclaim_freed_memory(Some(&pool));
        merge_elapsed += resolve_start.elapsed();
        workspace_rbs
    } else {
        // Seed the accumulator with a copy of `base_user_rbs` so the original stays
        // available, unmutated, as analysis context for every chunk (cross-file
        // visibility must come only from the single post-merge Batch resolution
        // below, never from a partially-merged accumulator).
        let mut batch_builder =
            tyda::workspace_state::BatchProjectionBuilder::new(base_user_rbs.clone());
        // Sources are read per chunk and dropped with it: nothing past this loop
        // reads them, so the corpus is never resident all at once.
        for chunk in file_paths.chunks(CLI_COMPACT_ANALYSIS_CHUNK_SIZE) {
            let compact_chunk_start = Instant::now();
            let compact_user_rbs = &base_user_rbs;
            let compact_lazy_rbi_loader = lazy_rbi_loader.as_ref();
            let compact_analyses = pool.install(|| {
                chunk
                    .par_iter()
                    .filter_map(|path| {
                        let source = fs::read_to_string(path).ok()?;
                        let file_path_str = path.to_string_lossy();
                        let job_start = scan_job_timing.then(Instant::now);
                        let snapshot = analyze_compact_file_snapshot_timed(
                            &source,
                            Some(compact_user_rbs),
                            &stdlib_loader,
                            compact_lazy_rbi_loader,
                            &file_path_str,
                            opts.clone(),
                            true,
                        )
                        .0;
                        if let Some(job_start) = job_start {
                            let elapsed = job_start.elapsed();
                            if elapsed.as_millis() >= 500 {
                                eprintln!(
                                    "scan-job {file_path_str}: {:.3}s",
                                    elapsed.as_secs_f64()
                                );
                            }
                        }
                        Some((file_path_str.into_owned(), snapshot))
                    })
                    .collect::<Vec<_>>()
            });
            compact_collection_elapsed += compact_chunk_start.elapsed();

            let merge_chunk_start = Instant::now();
            batch_builder.apply_chunk(compact_analyses);
            merge_elapsed += merge_chunk_start.elapsed();
        }
        // Context copy no longer needed; drop it before resolution grows the registry.
        drop(base_user_rbs);

        if memory_breakdown {
            eprintln!(
                "[mem] after-scan: rss={:.0}MB live={:.0}MB files_merged={} (+{} context)",
                max_rss_mb(),
                current_rss_mb(),
                batch_builder.applied_file_count(),
                context_file_count,
            );
        }

        let resolve_start = Instant::now();
        let workspace_rbs = batch_builder.finish(Some(&pool));
        tyda::reclaim_freed_memory(Some(&pool));
        merge_elapsed += resolve_start.elapsed();
        workspace_rbs
    };

    if memory_breakdown {
        let b = workspace_rbs.memory_breakdown();
        eprintln!(
            "[mem] after-projection: rss={:.0}MB live={:.0}MB classes={} methods={} (shared={}) call_sites={} constants={} ivars={} param_cache={} name_pool={}",
            max_rss_mb(),
            current_rss_mb(),
            b.classes,
            b.methods_total,
            b.methods_shared,
            b.call_sites_total,
            b.constants_total,
            b.ivars_total,
            b.param_cache_entries,
            b.name_pool_entries,
        );
        // Deep byte attribution (container/constant+ivar/call-site/method-body), same
        // fields the LSP's TYDA_RESOLUTION_TIMING debug report already prints.
        let mut seen = rustc_hash::FxHashSet::default();
        let d = workspace_rbs.deep_breakdown(&mut seen);
        let mb = |bytes: usize| bytes as f64 / 1_048_576.0;
        eprintln!(
            "[mem] deep after-projection: total={:.1}MB bodies={:.1}MB (new={} shared_prior={}) call_sites={:.1}MB (n={}) containers={:.1}MB const_ivar={:.1}MB",
            mb(d.total_bytes),
            mb(d.method_body_bytes),
            d.methods_new,
            d.methods_shared_prior,
            mb(d.call_site_bytes),
            d.call_site_count,
            mb(d.container_bytes),
            mb(d.constant_ivar_bytes),
        );
        let param_cache_bytes = workspace_rbs.resolve_params_cache_deep_bytes();
        eprintln!(
            "[mem] deep resolve_params_cache: entries={} bytes={:.1}MB",
            b.param_cache_entries,
            mb(param_cache_bytes),
        );
        // Lazy stdlib caches outlive projection: the per-class shape memo is
        // unbounded, so it is attributed separately from the parsed-file cache.
        let s = stdlib_loader.deep_breakdown(&mut seen);
        eprintln!(
            "[mem] deep stdlib_loader: shapes={:.1}MB (built={} slots={}) file_cache={:.1}MB (parsed={}) class_index={:.1}MB",
            mb(s.shapes.total_bytes),
            s.shape_count,
            s.shape_slot_count,
            mb(s.file_cache.total_bytes),
            s.file_cache_count,
            mb(s.index_bytes),
        );
        let source_bytes: usize = diagnostic_sources
            .iter()
            .map(|(path, source)| source.capacity() + path.as_os_str().len())
            .sum();
        eprintln!(
            "[mem] deep file_sources: retained={} bytes={:.1}MB",
            diagnostic_sources.len(),
            mb(source_bytes),
        );
        // Interned names are leaked for the process lifetime and are excluded from
        // every registry walk above (registries store `Sym` handles, not bytes).
        let (sym_count, sym_bytes) = tyda::sym::interner_stats();
        eprintln!(
            "[mem] deep sym_interner: entries={sym_count} bytes={:.1}MB (+{:.1}MB shell)",
            mb(sym_bytes),
            mb(sym_count * (std::mem::size_of::<usize>() * 3 + 1)),
        );
    }
    let summary_dsl_activation = compact_scan_dsl_activation.clone();
    let summary_project_versions = compact_scan_project_versions;

    let mut diagnostic_count = None;
    if options.diagnostics {
        eprintln!(
            "Emitting diagnostics for {} files...",
            diagnostic_sources.len()
        );
        let diagnostic_analysis = CliAnalysisContext {
            user_rbs: &workspace_rbs,
            stdlib_loader: &stdlib_loader,
            lazy_rbi_loader: lazy_rbi_loader.as_ref(),
            rails_mode,
            dsl_activation: compact_scan_dsl_activation,
            project_versions: compact_scan_project_versions,
            workspace_root: compact_scan_workspace_root,
            debug: options.debug,
            include_synthetic_dsl_methods: options.include_synthetic_dsl_methods,
        };
        diagnostic_count = Some(write_cli_diagnostics(
            &diagnostic_sources,
            diagnostic_replays,
            &diagnostic_analysis,
            &pool,
            &processed_count,
            &mut stdout,
        ));
    } else if options.verbose || options.debug {
        let final_analysis = CliAnalysisContext {
            user_rbs: &workspace_rbs,
            stdlib_loader: &stdlib_loader,
            lazy_rbi_loader: lazy_rbi_loader.as_ref(),
            rails_mode,
            dsl_activation: compact_scan_dsl_activation,
            project_versions: compact_scan_project_versions,
            workspace_root: compact_scan_workspace_root,
            debug: options.debug,
            include_synthetic_dsl_methods: options.include_synthetic_dsl_methods,
        };

        let files: &[PathBuf] = &file_paths;
        if options.verbose {
            for (index, path) in files.iter().enumerate() {
                eprint!(
                    "\x1b[2m[{}/{}]\x1b[0m {} ... ",
                    index + 1,
                    total_files,
                    path.display()
                );
                std::io::stderr().flush().ok();
                let file_start = Instant::now();
                match analyze_file(path, &final_analysis) {
                    Some((output, timing)) => {
                        let ms = file_start.elapsed().as_secs_f64() * 1000.0;
                        if ms > 1000.0 {
                            eprintln!("\x1b[33m{:.0}ms\x1b[0m", ms);
                        } else {
                            eprintln!("\x1b[32m{:.1}ms\x1b[0m", ms);
                        }
                        stdout.write_all(output.as_bytes()).ok();
                        file_timings.push(timing);
                    }
                    None => {
                        eprintln!("\x1b[2mskipped\x1b[0m");
                    }
                }
                processed_count.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            const OUTPUT_CHUNK_SIZE: usize = 16;
            for (chunk_start, chunk) in files.chunks(OUTPUT_CHUNK_SIZE).enumerate() {
                let mut results: Vec<FileAnalysisResult> = pool.install(|| {
                    chunk
                        .par_iter()
                        .enumerate()
                        .filter_map(|(offset, path)| {
                            let result =
                                analyze_file(path, &final_analysis).map(|(output, timing)| {
                                    FileAnalysisResult {
                                        index: chunk_start * OUTPUT_CHUNK_SIZE + offset,
                                        output,
                                        timing,
                                    }
                                });
                            processed_count.fetch_add(1, Ordering::Relaxed);
                            result
                        })
                        .collect()
                });
                results.sort_by_key(|result| result.index);
                for result in results {
                    stdout.write_all(result.output.as_bytes()).ok();
                    file_timings.push(result.timing);
                }
                stdout.flush().ok();
            }
        }
    } else {
        let render_start = Instant::now();
        render_rbs_to_writer_in_pool(
            &workspace_rbs,
            RenderOptions {
                include_synthetic_dsl_methods: options.include_synthetic_dsl_methods,
            },
            &mut stdout,
            Some(&pool),
        )
        .ok();
        stdout.flush().ok();
        let _render_elapsed = render_start.elapsed();
        processed_count.store(total_files, Ordering::Relaxed);
    }

    if memory_breakdown {
        eprintln!(
            "[mem] after-output: rss={:.0}MB live={:.0}MB",
            max_rss_mb(),
            current_rss_mb()
        );
    }

    if let Some(handle) = progress_handle {
        stop.store(true, Ordering::Relaxed);
        let shown = handle.join().unwrap_or(false);
        if shown {
            eprint!("\r\x1b[2K");
            std::io::stderr().flush().ok();
        }
    }

    let summary = CliRunSummary {
        total_files,
        elapsed: start.elapsed(),
        file_timings,
        dsl_activation: summary_dsl_activation,
        project_versions: summary_project_versions,
    };

    stdout.flush().ok();

    if options.debug && !options.diagnostics {
        print_debug_report(&summary);
    }

    if let Some(diagnostic_count) = diagnostic_count {
        eprintln!(
            "\nTyda v{} — analyzed {} files, emitted {} diagnostics in {:.3}s (preload {:.3}s, compact-scan {:.3}s, context-scan {:.3}s [+{} files], merge {:.3}s, final-diagnostics {:.3}s)",
            TYDA_VERSION,
            summary.total_files,
            diagnostic_count,
            summary.elapsed.as_secs_f64(),
            preload_elapsed.as_secs_f64(),
            compact_collection_elapsed.as_secs_f64(),
            context_scan_elapsed.as_secs_f64(),
            context_file_count,
            merge_elapsed.as_secs_f64(),
            summary.elapsed.as_secs_f64()
                - preload_elapsed.as_secs_f64()
                - compact_collection_elapsed.as_secs_f64()
                - context_scan_elapsed.as_secs_f64()
                - merge_elapsed.as_secs_f64(),
        );
    } else {
        eprintln!(
            "\nTyda v{} — analyzed {} files in {:.3}s (preload {:.3}s, compact-scan {:.3}s, merge {:.3}s, final-resolution+render {:.3}s)",
            TYDA_VERSION,
            summary.total_files,
            summary.elapsed.as_secs_f64(),
            preload_elapsed.as_secs_f64(),
            compact_collection_elapsed.as_secs_f64(),
            merge_elapsed.as_secs_f64(),
            summary.elapsed.as_secs_f64()
                - preload_elapsed.as_secs_f64()
                - compact_collection_elapsed.as_secs_f64()
                - merge_elapsed.as_secs_f64(),
        );
    }

    // Batch CLI: skip Drop of GB-scale registries / 64MiB rayon stacks. Walking
    // every allocation after stdout was already complete looks like a hang.
    std::process::exit(0);
}

fn build_cli_context(
    paths: &[PathBuf],
    debug: bool,
    dsl_spec: Option<&str>,
) -> (
    PathBuf,
    LazyRbsLoader,
    tyda::registry::TypeRegistry,
    Option<LazyRbiLoader>,
    bool,
    DslActivation,
    ProjectVersions,
) {
    let preload_timing = std::env::var_os("TYDA_PRELOAD_TIMING").is_some();
    let started_at = Instant::now();
    let root_started = Instant::now();
    let workspace_root = infer_workspace_root(paths);
    let project_versions = ProjectVersions::detect(&workspace_root);
    let root_elapsed = root_started.elapsed();
    let vendor_rbs_root = tyda::rbs::workspace::default_vendor_rbs_root();
    let stdlib_started = Instant::now();
    let stdlib_loader =
        LazyRbsLoader::for_ruby_version(vendor_rbs_root, project_versions.effective_ruby());
    let stdlib_elapsed = stdlib_started.elapsed();

    let external_started = Instant::now();
    let mut loaded = load_cli_type_environment(paths, &stdlib_loader);
    let external_elapsed = external_started.elapsed();
    let dsl_started = Instant::now();
    let mut dsl_activation = DslActivation::with_auto_detected(
        tyda::project::detect_dsl_libraries_from_gems(&workspace_root),
    );
    if let Some(spec) = dsl_spec {
        dsl_activation.apply_cli_spec(spec);
    }
    let dsl_elapsed = dsl_started.elapsed();
    let rails_started = Instant::now();
    let rails_mode = if dsl_activation.rails_mode_compat() {
        tyda::rails::load_project_types_with_activation(
            &workspace_root,
            &mut loaded.user_rbs,
            &dsl_activation,
        )
    } else {
        false
    };
    let rails_elapsed = rails_started.elapsed();
    loaded.user_rbs.shrink_to_fit_after_compact();
    let enabled_dsl = dsl_activation
        .enabled_libraries()
        .into_iter()
        .map(|dsl| dsl.cli_name())
        .collect::<Vec<_>>()
        .join(",");

    if debug || preload_timing {
        eprintln!(
            "DEBUG cli_context ruby={:?} rails={:?} dsl=[{}] root_ms={:.3} stdlib_ms={:.3} external_ms={:.3} dsl_ms={:.3} rails_ms={:.3} total_ms={:.3}",
            project_versions.ruby,
            project_versions.rails,
            enabled_dsl,
            root_elapsed.as_secs_f64() * 1000.0,
            stdlib_elapsed.as_secs_f64() * 1000.0,
            external_elapsed.as_secs_f64() * 1000.0,
            dsl_elapsed.as_secs_f64() * 1000.0,
            rails_elapsed.as_secs_f64() * 1000.0,
            started_at.elapsed().as_secs_f64() * 1000.0,
        );
    }

    (
        workspace_root,
        stdlib_loader,
        loaded.user_rbs,
        loaded.lazy_rbi_loader,
        rails_mode,
        dsl_activation,
        project_versions,
    )
}

fn should_show_progress(is_tty: bool, total_files: usize, options: CliRunOptions) -> bool {
    is_tty && total_files > 0 && !options.debug && !options.verbose
}

fn files_per_sec(summary: &CliRunSummary) -> f64 {
    let elapsed = summary.elapsed.as_secs_f64();
    if elapsed > 0.0 {
        summary.total_files as f64 / elapsed
    } else {
        summary.total_files as f64
    }
}

fn print_debug_report(summary: &CliRunSummary) {
    let mut phase_totals = AnalysisTimings::default();
    let mut render_total = Duration::ZERO;
    let mut total_holes = 0usize;
    let mut total_untyped = 0usize;
    let mut by_kind = BTreeMap::new();
    let mut by_reason = BTreeMap::new();
    let mut holes_by_file: BTreeMap<String, usize> = BTreeMap::new();
    let mut unresolved_ranking: BTreeMap<String, usize> = BTreeMap::new();
    for timing in &summary.file_timings {
        phase_totals.parse += timing.analysis.parse;
        phase_totals.comments += timing.analysis.comments;
        phase_totals.definition_collection += timing.analysis.definition_collection;
        phase_totals.parameter_reference_resolution +=
            timing.analysis.parameter_reference_resolution;
        phase_totals.receiver_reference_preload += timing.analysis.receiver_reference_preload;
        phase_totals.deps += timing.analysis.deps;
        render_total += timing.render;
        total_holes += timing.holes.total_count();
        total_untyped += timing.holes.untyped_count();
        for (kind, count) in timing.holes.counts_by_kind() {
            *by_kind.entry(kind).or_insert(0usize) += count;
        }
        for (reason, count) in timing.holes.counts_by_reason() {
            *by_reason.entry(reason).or_insert(0usize) += count;
        }
        for hole in &timing.holes.holes {
            let entry = unresolved_ranking
                .entry(format!(
                    "{}#{}:{}",
                    hole.class_name, hole.member_name, hole.slot_name
                ))
                .or_insert(0);
            *entry += 1;
        }
        holes_by_file.insert(timing.path.clone(), timing.holes.total_count());
    }

    eprintln!(
        "DEBUG summary total_files={} elapsed_sec={:.3} files_per_sec={:.2}",
        summary.total_files,
        summary.elapsed.as_secs_f64(),
        files_per_sec(summary)
    );
    eprintln!(
        "DEBUG phases parse_ms={:.3} comments_ms={:.3} definition_collection_ms={:.3} parameter_resolution_ms={:.3} receiver_preload_ms={:.3} deps_ms={:.3} render_ms={:.3}",
        phase_totals.parse.as_secs_f64() * 1000.0,
        phase_totals.comments.as_secs_f64() * 1000.0,
        phase_totals.definition_collection.as_secs_f64() * 1000.0,
        phase_totals.parameter_reference_resolution.as_secs_f64() * 1000.0,
        phase_totals.receiver_reference_preload.as_secs_f64() * 1000.0,
        phase_totals.deps.as_secs_f64() * 1000.0,
        render_total.as_secs_f64() * 1000.0,
    );
    eprintln!(
        "DEBUG holes total={} untyped={}",
        total_holes, total_untyped
    );
    eprintln!(
        "DEBUG versions ruby={:?} rails={:?}",
        summary.project_versions.ruby, summary.project_versions.rails
    );
    let mut enabled_collectors = 0usize;
    for library in tyda::project::DslLibrary::official_builtins() {
        let source = summary.dsl_activation.activation_source(*library);
        if matches!(source, DslActivationSource::Disabled) {
            continue;
        }
        enabled_collectors += 1;
        eprintln!(
            "DEBUG collector library={} family={} activation={}",
            library.cli_name(),
            debug_dsl_family_name(library.family()),
            debug_activation_source_name(source)
        );
    }
    eprintln!("DEBUG collectors enabled={enabled_collectors}");
    for (kind, count) in by_kind {
        eprintln!("DEBUG holes_by_kind kind={} count={}", kind, count);
    }
    for (reason, count) in by_reason {
        eprintln!("DEBUG holes_by_reason reason={} count={}", reason, count);
    }
    eprintln!("DEBUG hole_heatmap:");
    let mut heatmap: Vec<_> = holes_by_file.into_iter().collect();
    heatmap.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (rank, (path, holes)) in heatmap.into_iter().take(10).enumerate() {
        eprintln!(
            "DEBUG hole_heat rank={} holes={} path={}",
            rank + 1,
            holes,
            path
        );
    }
    eprintln!("DEBUG unresolved_ranking:");
    let mut ranking: Vec<_> = unresolved_ranking.into_iter().collect();
    ranking.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (rank, (target, count)) in ranking.into_iter().take(20).enumerate() {
        eprintln!(
            "DEBUG unresolved rank={} count={} target={}",
            rank + 1,
            count,
            target
        );
    }

    for timing in &summary.file_timings {
        eprintln!(
            "DEBUG file elapsed_ms={:.3} parse_ms={:.3} comments_ms={:.3} definition_collection_ms={:.3} parameter_resolution_ms={:.3} receiver_preload_ms={:.3} deps_ms={:.3} render_ms={:.3} path={}",
            timing.elapsed.as_secs_f64() * 1000.0,
            timing.analysis.parse.as_secs_f64() * 1000.0,
            timing.analysis.comments.as_secs_f64() * 1000.0,
            timing.analysis.definition_collection.as_secs_f64() * 1000.0,
            timing.analysis.parameter_reference_resolution.as_secs_f64() * 1000.0,
            timing.analysis.receiver_reference_preload.as_secs_f64() * 1000.0,
            timing.analysis.deps.as_secs_f64() * 1000.0,
            timing.render.as_secs_f64() * 1000.0,
            timing.path
        );
        for hole in timing.holes.holes.iter().take(10) {
            eprintln!(
                "DEBUG hole kind={} reason={} class={} member={} slot={} type={} line={}",
                hole.kind.as_str(),
                hole.reason.as_str(),
                hole.class_name,
                hole.member_name,
                hole.slot_name,
                hole.rendered_type,
                hole.line.unwrap_or(0)
            );
        }
        if let Some(seed) = &timing.scenario_seed {
            eprintln!("DEBUG scenario_seed_begin path={}", timing.path);
            eprintln!("{seed}");
            eprintln!("DEBUG scenario_seed_end path={}", timing.path);
        }
    }

    let mut slowest = summary.file_timings.clone();
    slowest.sort_by(|a, b| b.elapsed.cmp(&a.elapsed));
    eprintln!("DEBUG slowest files:");
    for timing in slowest.into_iter().take(10) {
        eprintln!(
            "DEBUG slowest elapsed_ms={:.3} parameter_resolution_ms={:.3} receiver_preload_ms={:.3} render_ms={:.3} path={}",
            timing.elapsed.as_secs_f64() * 1000.0,
            timing.analysis.parameter_reference_resolution.as_secs_f64() * 1000.0,
            timing.analysis.receiver_reference_preload.as_secs_f64() * 1000.0,
            timing.render.as_secs_f64() * 1000.0,
            timing.path
        );
    }
}

fn debug_activation_source_name(source: DslActivationSource) -> &'static str {
    match source {
        DslActivationSource::AutoDetected => "auto",
        DslActivationSource::ForcedOn => "forced_on",
        DslActivationSource::ForcedOff => "forced_off",
        DslActivationSource::Disabled => "disabled",
    }
}

fn debug_dsl_family_name(family: DslFamily) -> &'static str {
    match family {
        DslFamily::Rails => "rails",
        DslFamily::Ruby => "ruby",
        DslFamily::Gem => "gem",
    }
}

fn write_cli_diagnostics<W: std::io::Write>(
    file_sources: &[(PathBuf, String)],
    mut diagnostic_replays: Vec<FileAnalysisSnapshot>,
    analysis: &CliAnalysisContext<'_>,
    pool: &rayon::ThreadPool,
    processed_count: &std::sync::atomic::AtomicUsize,
    stdout: &mut W,
) -> usize {
    const DIAGNOSTIC_CHUNK_SIZE: usize = 16;
    let total = file_sources.len();
    let log_heartbeat = !std::io::IsTerminal::is_terminal(&std::io::stderr());
    let heartbeat_every = 256.max(DIAGNOSTIC_CHUNK_SIZE);
    let mut diagnostic_count = 0usize;
    for (chunk_start, chunk) in file_sources.chunks(DIAGNOSTIC_CHUNK_SIZE).enumerate() {
        let replay_chunk: Vec<FileAnalysisSnapshot> = diagnostic_replays
            .drain(..chunk.len().min(diagnostic_replays.len()))
            .collect();
        let mut results: Vec<FileDiagnosticResult> = pool.install(|| {
            chunk
                .par_iter()
                .zip(replay_chunk)
                .enumerate()
                .map(|(offset, ((path, source), snapshot))| {
                    let index = chunk_start * DIAGNOSTIC_CHUNK_SIZE + offset;
                    let started = Instant::now();
                    let diagnostics =
                        diagnostics_from_snapshot_owned(path, source, snapshot, analysis);
                    let elapsed = started.elapsed();
                    if elapsed.as_millis() >= 2000 {
                        eprintln!(
                            "diagnostics-job {}: {:.3}s",
                            path.display(),
                            elapsed.as_secs_f64()
                        );
                    }
                    processed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    FileDiagnosticResult { index, diagnostics }
                })
                .collect()
        });
        results.sort_by_key(|result| result.index);
        for mut result in results {
            // Position order (not category-grouping order) keeps JSONL diffable across runs.
            result.diagnostics.sort_by(|a, b| {
                (
                    a.line,
                    a.column,
                    a.end_line,
                    a.end_column,
                    a.code,
                    &a.message,
                )
                    .cmp(&(
                        b.line,
                        b.column,
                        b.end_line,
                        b.end_column,
                        b.code,
                        &b.message,
                    ))
            });
            for diagnostic in result.diagnostics {
                serde_json::to_writer(&mut *stdout, &diagnostic).ok();
                stdout.write_all(b"\n").ok();
                diagnostic_count += 1;
            }
        }
        stdout.flush().ok();
        let done = processed_count.load(std::sync::atomic::Ordering::Relaxed);
        if log_heartbeat && (done == total || done % heartbeat_every < DIAGNOSTIC_CHUNK_SIZE) {
            eprintln!("diagnostics {done}/{total} files, {diagnostic_count} emitted");
        }
    }
    if analysis.debug {
        for (reason, count) in tyda::diagnostics::gating_suppression_counts() {
            eprintln!("DEBUG gating_suppressed reason={reason} count={count}");
        }
    }
    diagnostic_count
}

fn diagnostics_from_snapshot_owned(
    path: &Path,
    source: &str,
    snapshot: FileAnalysisSnapshot,
    analysis: &CliAnalysisContext<'_>,
) -> Vec<TypeDiagnostic> {
    let file_path_str = path.to_string_lossy();
    tyda::analysis::cli_diagnostics_from_snapshot_owned(
        snapshot,
        source,
        &file_path_str,
        analysis.stdlib_loader,
        analysis.lazy_rbi_loader,
        Some(analysis.user_rbs),
    )
}

fn analyze_file(path: &Path, analysis: &CliAnalysisContext<'_>) -> Option<(String, FileTiming)> {
    if !is_ruby_file(path) {
        return None;
    }

    match fs::read_to_string(path) {
        Ok(source) => {
            let file_path_str = path.to_string_lossy();
            let started_at = Instant::now();
            let (registry, analysis_timings) = analyze_file_registry_timed(
                &source,
                Some(analysis.user_rbs),
                analysis.stdlib_loader,
                analysis.lazy_rbi_loader,
                &file_path_str,
                AnalysisOptions {
                    rails_mode: analysis.rails_mode,
                    dsl_activation: analysis.dsl_activation.clone(),
                    project_versions: analysis.project_versions,
                    project_root: Some(analysis.workspace_root.clone()),
                },
                true,
            );
            let render_started = Instant::now();
            let rbs = render_rbs_with_options(
                &registry,
                RenderOptions {
                    include_synthetic_dsl_methods: analysis.include_synthetic_dsl_methods,
                },
            );
            let render_elapsed = render_started.elapsed();
            let elapsed = started_at.elapsed();
            let holes = summarize_type_holes(&registry);
            let scenario_seed = if analysis.debug {
                build_scenario_seed(&file_path_str, &source, &registry, &holes)
            } else {
                None
            };
            Some((
                rbs,
                FileTiming {
                    path: file_path_str.to_string(),
                    elapsed,
                    analysis: analysis_timings,
                    render: render_elapsed,
                    holes,
                    scenario_seed,
                },
            ))
        }
        Err(e) => {
            eprintln!("Error reading {}: {e}", path.display());
            None
        }
    }
}

fn is_ruby_file(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|ext| ext == "rb")
}

fn collect_analysis_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        collect_analysis_files_from_path(path, &mut files, &mut seen);
    }
    files
}

fn collect_analysis_files_from_path(
    path: &Path,
    files: &mut Vec<PathBuf>,
    seen: &mut std::collections::HashSet<PathBuf>,
) {
    if path.is_file() {
        if is_ruby_file(path) {
            let path_buf = path.to_path_buf();
            if seen.insert(path_buf.clone()) {
                files.push(path_buf);
            }
        }
        return;
    }

    if !path.is_dir() {
        eprintln!("Warning: {} is not a file or directory", path.display());
        return;
    }

    let mut entries: Vec<_> = match fs::read_dir(path) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(e) => {
            eprintln!("Error reading directory {}: {e}", path.display());
            return;
        }
    };
    // fs::read_dir order is filesystem-dependent; sort for deterministic output.
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let child = entry.path();
        if child.is_dir() && should_skip_dir(&child) {
            continue;
        }
        collect_analysis_files_from_path(&child, files, seen);
    }
}

fn should_skip_dir(path: &Path) -> bool {
    tyda::workspace_discovery::should_skip_dir(path, RubyScanScope::Production)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_for_socket_close_returns_after_peer_disconnect() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept stream");
            let start = Instant::now();
            wait_for_socket_close(stream);
            start.elapsed()
        });

        let client = std::net::TcpStream::connect(addr).expect("connect client");
        std::thread::sleep(Duration::from_millis(100));
        drop(client);

        let elapsed = server.join().expect("server thread");
        assert!(elapsed < Duration::from_secs(2));
    }

    #[test]
    fn wait_for_socket_close_waits_until_peer_disconnects() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept stream");
            let start = Instant::now();
            wait_for_socket_close(stream);
            start.elapsed()
        });

        let client = std::net::TcpStream::connect(addr).expect("connect client");
        std::thread::sleep(Duration::from_millis(350));
        drop(client);

        let elapsed = server.join().expect("server thread");
        assert!(elapsed >= Duration::from_millis(250));
    }

    #[test]
    fn skip_dir_name_matches_known_large_directories() {
        use tyda::workspace_discovery::should_skip_dir_name;
        assert!(should_skip_dir_name("vendor", RubyScanScope::Production));
        assert!(should_skip_dir_name("target", RubyScanScope::Production));
        assert!(should_skip_dir_name("spec", RubyScanScope::Production));
        assert!(should_skip_dir_name("migrate", RubyScanScope::Production));
        assert!(should_skip_dir_name(
            "post_migrate",
            RubyScanScope::Production
        ));
        assert!(!should_skip_dir_name("app", RubyScanScope::Production));
    }
}
