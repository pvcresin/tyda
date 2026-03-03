// Measure hover coverage using the LSP-style workspace registry path.
// Unlike a direct hover_at probe, this mirrors what an editor actually
// experiences: workspace_state accumulates FileAnalysisSnapshots, then
// hover queries go through workspace_registry_excluding (display scope)
// + hover_at_with_analysis_options.

use ruby_prism::{Location, Node};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tyda::analysis::{
    AnalysisOptions, analyze_file_facts_with_deps, hover_at_with_analysis_options,
};
use tyda::project::{DslActivation, ProjectVersions};
use tyda::rbs::stdlib_loader::LazyRbsLoader;
use tyda::registry::TypeRegistry;
use tyda::workspace_state::{WorkspaceState, hash_content};

#[derive(Default)]
struct Stats {
    per_kind: BTreeMap<&'static str, Bucket>,
}

#[derive(Default, Clone, Copy)]
struct Bucket {
    resolved: usize,
    // Method/identifier was found but return/value is genuinely untyped.
    resolved_untyped: usize,
    // Method/identifier could not be resolved at all (real gap).
    unresolved: usize,
    // hover returned None (no token / unknown).
    missing: usize,
}

impl Bucket {
    fn total(&self) -> usize {
        self.resolved + self.resolved_untyped + self.unresolved + self.missing
    }
    fn pct(&self) -> f64 {
        if self.total() == 0 {
            0.0
        } else {
            100.0 * self.resolved as f64 / self.total() as f64
        }
    }
    // "any resolution" = found something (even if untyped return)
    fn pct_any(&self) -> f64 {
        if self.total() == 0 {
            0.0
        } else {
            100.0 * (self.resolved + self.resolved_untyped) as f64 / self.total() as f64
        }
    }
}

fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 0usize;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

struct Ctx<'a> {
    source: &'a str,
    loader: &'a LazyRbsLoader,
    workspace: &'a TypeRegistry,
    file_path: &'a str,
    opts: &'a AnalysisOptions,
}

fn probe(ctx: &Ctx<'_>, loc: &Location<'_>, kind: &'static str, stats: &mut Stats) {
    let (line, col) = offset_to_line_col(ctx.source, loc.start_offset());
    let b = stats.per_kind.entry(kind).or_default();
    let r = hover_at_with_analysis_options(
        ctx.source,
        Some(ctx.workspace),
        ctx.loader,
        ctx.file_path,
        line,
        col,
        ctx.opts.clone(),
    );
    match r {
        Some(h) => {
            let t = h.ty.to_string();
            let is_untyped = t == "__todo__" || t == "untyped";
            if is_untyped {
                if h.unresolved_method.is_some() {
                    b.unresolved += 1;
                    if std::env::var("DUMP_UNRESOLVED").is_ok() {
                        let snippet = ctx
                            .source
                            .get(loc.start_offset()..loc.end_offset().min(ctx.source.len()))
                            .unwrap_or("")
                            .chars()
                            .take(60)
                            .collect::<String>();
                        eprintln!(
                            "UNRESOLVED {kind} {}:{line}:{col} {}={} (unresolved={:?})",
                            ctx.file_path, h.name, snippet, h.unresolved_method
                        );
                    }
                } else {
                    b.resolved_untyped += 1;
                    if std::env::var("DUMP_UNTYPED").is_ok() {
                        let snippet = ctx
                            .source
                            .get(loc.start_offset()..loc.end_offset().min(ctx.source.len()))
                            .unwrap_or("")
                            .chars()
                            .take(60)
                            .collect::<String>();
                        eprintln!(
                            "UNTYPED {kind} {}:{line}:{col} {}={}",
                            ctx.file_path, h.name, snippet
                        );
                    }
                }
            } else {
                b.resolved += 1;
            }
        }
        None => {
            b.missing += 1;
        }
    }
}

fn walk(node: &Node<'_>, ctx: &Ctx<'_>, stats: &mut Stats) {
    match node {
        Node::LocalVariableReadNode { .. } => probe(ctx, &node.location(), "local_var", stats),
        Node::InstanceVariableReadNode { .. } => probe(ctx, &node.location(), "ivar", stats),
        Node::ClassVariableReadNode { .. } => probe(ctx, &node.location(), "cvar", stats),
        Node::GlobalVariableReadNode { .. } => probe(ctx, &node.location(), "gvar", stats),
        Node::ConstantReadNode { .. } => probe(ctx, &node.location(), "const_read", stats),
        Node::ConstantPathNode { .. } => probe(ctx, &node.location(), "const_path", stats),
        Node::CallNode { .. } => {
            if let Some(call) = node.as_call_node()
                && let Some(msg_loc) = call.message_loc()
            {
                probe(ctx, &msg_loc, "call", stats);
            }
        }
        _ => {}
    }
    if let Some(p) = node.as_program_node() {
        for n in p.statements().body().iter() {
            walk(&n, ctx, stats);
        }
    } else if let Some(s) = node.as_statements_node() {
        for n in s.body().iter() {
            walk(&n, ctx, stats);
        }
    } else if let Some(c) = node.as_class_node() {
        if let Some(b) = c.body() {
            walk(&b, ctx, stats);
        }
    } else if let Some(m) = node.as_module_node() {
        if let Some(b) = m.body() {
            walk(&b, ctx, stats);
        }
    } else if let Some(d) = node.as_def_node() {
        if let Some(b) = d.body() {
            walk(&b, ctx, stats);
        }
    } else if let Some(call) = node.as_call_node() {
        if let Some(r) = call.receiver() {
            walk(&r, ctx, stats);
        }
        if let Some(args) = call.arguments() {
            for a in args.arguments().iter() {
                walk(&a, ctx, stats);
            }
        }
        if let Some(bl) = call.block()
            && let Some(b) = bl.as_block_node()
            && let Some(body) = b.body()
        {
            walk(&body, ctx, stats);
        }
    } else if let Some(i) = node.as_if_node() {
        walk(&i.predicate(), ctx, stats);
        if let Some(s) = i.statements() {
            for n in s.body().iter() {
                walk(&n, ctx, stats);
            }
        }
        if let Some(e) = i.subsequent() {
            walk(&e, ctx, stats);
        }
    } else if let Some(w) = node.as_local_variable_write_node() {
        walk(&w.value(), ctx, stats);
    } else if let Some(w) = node.as_instance_variable_write_node() {
        walk(&w.value(), ctx, stats);
    } else if let Some(w) = node.as_constant_write_node() {
        walk(&w.value(), ctx, stats);
    } else if let Some(a) = node.as_array_node() {
        for e in a.elements().iter() {
            walk(&e, ctx, stats);
        }
    } else if let Some(h) = node.as_hash_node() {
        for e in h.elements().iter() {
            walk(&e, ctx, stats);
        }
    } else if let Some(b) = node.as_begin_node() {
        if let Some(s) = b.statements() {
            for n in s.body().iter() {
                walk(&n, ctx, stats);
            }
        }
    } else if let Some(w) = node.as_while_node() {
        walk(&w.predicate(), ctx, stats);
        if let Some(s) = w.statements() {
            for n in s.body().iter() {
                walk(&n, ctx, stats);
            }
        }
    }
}

fn collect(root: &Path, out: &mut Vec<PathBuf>) {
    if root.is_file() {
        if root.extension().is_some_and(|e| e == "rb") {
            out.push(root.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.file_name()
            .is_some_and(|n| n == "vendor" || n == "test" || n == "spec")
        {
            continue;
        }
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|e| e == "rb") {
            out.push(p);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: ROOT...");
        std::process::exit(1);
    }
    // Load full stdlib (core + stdlib) so the coverage measurement
    // matches what a real LSP sees. `LazyRbsLoader::new` only loads core,
    // which under-reports resolution for projects that use FileUtils,
    // URI, Zlib, OpenSSL, etc.
    let vendor_rbs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs");
    let loader =
        LazyRbsLoader::for_ruby_version(vendor_rbs, tyda::project::RubyVersion::new(3, 3, 0));
    let mut files = Vec::new();
    for a in &args {
        collect(&PathBuf::from(a), &mut files);
    }
    eprintln!("collected {} files", files.len());

    let opts = AnalysisOptions {
        rails_mode: false,
        dsl_activation: DslActivation::with_rails_mode(false),
        project_versions: ProjectVersions::default(),
        project_root: None,
    };

    // Phase 1: build workspace_state from compact analyses.
    let mut ws = WorkspaceState::new();
    for p in &files {
        let Ok(src) = std::fs::read_to_string(p) else {
            continue;
        };
        let (analysis, deps) = analyze_file_facts_with_deps(
            &src,
            None,
            Some(&loader),
            Some(&p.to_string_lossy()),
            opts.clone(),
        );
        ws.upsert_file(
            p.to_string_lossy().into_owned(),
            hash_content(&src),
            analysis,
            deps,
        );
    }
    eprintln!("workspace_state populated");
    let user_rbs = TypeRegistry::new();

    // Phase 2: per-file hover coverage using excluding-registry.
    let mut total = Stats::default();
    for p in &files {
        let path_str = p.to_string_lossy().to_string();
        let Ok(src) = std::fs::read_to_string(p) else {
            continue;
        };
        let fp = ws.excluding_fingerprint(&path_str);
        let workspace = ws.workspace_registry_excluding(&user_rbs, &path_str, fp);

        let parsed = ruby_prism::parse(src.as_bytes());
        let root = parsed.node();
        let ctx = Ctx {
            source: &src,
            loader: &loader,
            workspace: &workspace,
            file_path: &path_str,
            opts: &opts,
        };
        let mut stats = Stats::default();
        walk(&root, &ctx, &mut stats);
        for (k, b) in &stats.per_kind {
            let t = total.per_kind.entry(k).or_default();
            t.resolved += b.resolved;
            t.resolved_untyped += b.resolved_untyped;
            t.unresolved += b.unresolved;
            t.missing += b.missing;
        }
        let _ = parsed;
    }
    println!("files: {}", files.len());
    let mut overall = Bucket::default();
    for (k, b) in &total.per_kind {
        overall.resolved += b.resolved;
        overall.resolved_untyped += b.resolved_untyped;
        overall.unresolved += b.unresolved;
        overall.missing += b.missing;
        println!(
            "{:>14} resolved={:>6} untyped={:>5} unresolved={:>5} miss={:>4} total={:>6} typed={:>5.1}% any={:>5.1}%",
            k,
            b.resolved,
            b.resolved_untyped,
            b.unresolved,
            b.missing,
            b.total(),
            b.pct(),
            b.pct_any()
        );
    }
    println!(
        "{:>14} resolved={:>6} untyped={:>5} unresolved={:>5} miss={:>4} total={:>6} typed={:>5.1}% any={:>5.1}%",
        "ALL",
        overall.resolved,
        overall.resolved_untyped,
        overall.unresolved,
        overall.missing,
        overall.total(),
        overall.pct(),
        overall.pct_any()
    );
}
