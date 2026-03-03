//! Robustness tests against pathological input: throws a large batch of Ruby / RBS / RBI
//! patterns designed to drive static type inference into an infinite loop, stack overflow,
//! exponential blowup, or excessive memory use, and verifies that analysis **always finishes
//! in bounded time without panicking**. Tyda is an inferencer, not a compiler, so for any
//! input it must degrade gracefully rather than hang or overflow (`docs/incomplete-code-policy.md`).
//!
//! Each probe runs on a dedicated thread with the "smallest realistic worker stack"
//! (~2MiB, comparable to std's default worker / cargo test thread / wasm's linear stack),
//! and a watchdog detects non-termination. This pins down both that the depth cap prevents
//! stack overflow even on a small stack, and that the fuel cap / type-size cap bound total work.
//!
//! When you find an unsupported pathological scenario, add a case here to prevent regressions.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use tyda::rbs::import::load_rbs_string;
use tyda::rbs::render::{RenderOptions, render_rbs_with_options};
use tyda::rbs::stdlib_loader::LazyRbsLoader;
use tyda::registry::TypeRegistry;
use tyda::sorbet::rbi::merge_rbi_source_into_registry;

/// Exceeding this is treated as "fully stuck" (an infinite loop / deadlock).
/// If the internal depth / fuel / type-size caps are working, every input finishes
/// well before this. The value has enough margin to absorb CI variance.
const HANG_TIMEOUT: Duration = Duration::from_secs(30);
/// Soft ceiling that detects "it finished, but too slowly" (e.g. a regression where a cap was loosened).
/// Debug builds run without optimizations and are orders of magnitude slower, so the same
/// corpus exceeds the release threshold (8s). This absorbs debug's constant-factor slowdown
/// while still preserving the meaning of the regression check.
#[cfg(debug_assertions)]
const SLOW_BOUND: Duration = Duration::from_secs(25);
#[cfg(not(debug_assertions))]
const SLOW_BOUND: Duration = Duration::from_secs(8);
/// Probe stack for the default tests. Matches the smallest realistic deployment stack
/// (std's default worker / wasm's linear stack) to verify that the depth cap prevents
/// inference stack overflow even on a small stack. Debug builds have stack frames several
/// times larger than optimized builds, and deeply nested **prism parse recursion** (on the
/// parser side, not inference) can exhaust 2MiB, so debug widens the budget to measure the
/// same logical guarantee (inference-cap verification at the 2MiB minimum stack is covered
/// by the release build instead).
#[cfg(debug_assertions)]
const SMALL_PROBE_STACK: usize = 16 * 1024 * 1024;
#[cfg(not(debug_assertions))]
const SMALL_PROBE_STACK: usize = 2 * 1024 * 1024;
/// Probe stack for the aggressive tests. A real-deployment-sized stack matching the
/// CLI / LSP analysis pool (`ANALYSIS_WORKER_STACK_SIZE`). Inputs with extreme nesting depth
/// (thousands of levels) exhaust a small stack via **prism's parse / AST-drop recursion**
/// rather than inference, so this measures whether the caps bound total work at a
/// production-sized stack (kept separate from the parser's own stack limit).
const PROD_PROBE_STACK: usize = 64 * 1024 * 1024;

fn loader() -> Arc<LazyRbsLoader> {
    let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
    Arc::new(LazyRbsLoader::new(core_dir))
}

enum Outcome {
    /// Finished normally, with the elapsed time.
    Completed(Duration),
    /// Panicked during analysis.
    Panicked,
    /// Did not finish within the watchdog window (treated as stuck).
    Hang,
}

/// Runs `work` on a dedicated small-stack thread, catching panics while a watchdog
/// detects non-termination. On a hang, the stuck thread is leaked (the test fails below).
fn run_probe<F>(loader: &Arc<LazyRbsLoader>, stack_size: usize, work: F) -> Outcome
where
    F: FnOnce(&LazyRbsLoader) + Send + 'static,
{
    let loader = Arc::clone(loader);
    let (tx, rx) = mpsc::channel();
    let handle = thread::Builder::new()
        .stack_size(stack_size)
        .spawn(move || {
            let start = Instant::now();
            let result = catch_unwind(AssertUnwindSafe(|| work(&loader)));
            let outcome: Result<Duration, ()> = match result {
                Ok(()) => Ok(start.elapsed()),
                Err(_) => Err(()),
            };
            let _ = tx.send(outcome);
        })
        .expect("spawn probe thread");

    match rx.recv_timeout(HANG_TIMEOUT) {
        Ok(Ok(elapsed)) => {
            let _ = handle.join();
            Outcome::Completed(elapsed)
        }
        Ok(Err(())) => {
            let _ = handle.join();
            Outcome::Panicked
        }
        // Don't join the stuck thread; leak it instead. We want the test to fail rather than abort.
        Err(_) => Outcome::Hang,
    }
}

/// A single probe = a label plus an execution closure.
struct Probe {
    label: String,
    work: Box<dyn FnOnce(&LazyRbsLoader) + Send + 'static>,
}

fn ruby(label: impl Into<String>, source: String) -> Probe {
    Probe {
        label: label.into(),
        work: Box::new(move |l| {
            let _ = tyda::analysis::playground_analyze(&source, "", l, "m.rb");
        }),
    }
}

/// Path that loads `.rbs` as user RBS and analyzes Ruby against it as context.
fn ruby_with_rbs(label: impl Into<String>, ruby_src: String, rbs_src: String) -> Probe {
    Probe {
        label: label.into(),
        work: Box::new(move |l| {
            let _ = tyda::analysis::playground_analyze(&ruby_src, &rbs_src, l, "m.rb");
        }),
    }
}

/// Path that loads `.rbs` directly into the registry and runs it through render (i.e. resolution).
fn rbs_load(label: impl Into<String>, rbs_src: String) -> Probe {
    Probe {
        label: label.into(),
        work: Box::new(move |_l| {
            let mut reg = TypeRegistry::new();
            load_rbs_string(&rbs_src, &mut reg);
            let _ = render_rbs_with_options(
                &reg,
                RenderOptions {
                    include_synthetic_dsl_methods: false,
                },
            );
        }),
    }
}

/// Path that merges `.rbi` into the registry and runs it through render.
fn rbi_load(label: impl Into<String>, rbi_src: String) -> Probe {
    Probe {
        label: label.into(),
        work: Box::new(move |l| {
            let mut reg = TypeRegistry::new();
            merge_rbi_source_into_registry(&rbi_src, &mut reg, l);
            let _ = render_rbs_with_options(
                &reg,
                RenderOptions {
                    include_synthetic_dsl_methods: false,
                },
            );
        }),
    }
}

fn repeat(line: &str, n: usize) -> String {
    line.repeat(n)
}

/// Corpus of pathological Ruby / RBS / RBI. `scale` switches the generated size
/// (moderate for the default tests, extreme for the aggressive tests).
fn corpus(scale: usize) -> Vec<Probe> {
    let mut probes = Vec::new();

    // --- Mutual param-receiver reference chain on the deferred (workspace) path ---
    // Links a ring of `def m(x) = x.m` methods through cross-class call sites. Pins down that
    // the pre-worklist per-method call-site substitution (marker kept, once per method) and the
    // worklist's fixed point converge to untyped in bounded time via the visiting set /
    // ROUND_BACKSTOP (no divergence or incorrect concretization).
    probes.push(Probe {
        label: "deferred_mutual_param_receiver_chain".into(),
        work: Box::new(move |l| {
            let n = scale.clamp(2, 64);
            let mut src = String::new();
            for i in 0..n {
                src.push_str(&format!(
                    "class C{i}
  def m(x) = x.m
end
"
                ));
            }
            src.push_str(
                "class Caller
  def use
",
            );
            for i in 0..n {
                let j = (i + 1) % n;
                src.push_str(&format!(
                    "    C{i}.new.m(C{j}.new)
"
                ));
            }
            src.push_str(
                "  end
end
",
            );
            let (snapshot, _) = tyda::analysis::analyze_compact_file_snapshot_timed(
                &src,
                None,
                l,
                None,
                "m.rb",
                tyda::analysis::AnalysisOptions::default(),
                true,
            );
            let mut reg = TypeRegistry::new();
            snapshot.apply_to_registry(&mut reg);
            reg.apply_cli_resolution();
        }),
    });

    // --- Self-referential shapes (array push / literal / hash write) ---
    probes.push(ruby(
        "self_push_array",
        format!("def f\n  x = []\n{}  x\nend\n", repeat("  x << x\n", scale)),
    ));
    probes.push(ruby(
        "self_push_method",
        format!(
            "def f\n  x = []\n{}  x\nend\n",
            repeat("  x.push(x)\n", scale)
        ),
    ));
    probes.push(ruby(
        "self_unshift",
        format!(
            "def f\n  x = []\n{}  x\nend\n",
            repeat("  x.unshift(x)\n", scale / 2)
        ),
    ));
    probes.push(ruby(
        "self_literal_assign",
        format!(
            "def f\n  x = []\n{}  x\nend\n",
            repeat("  x = [x, x, x, x]\n", scale / 4)
        ),
    ));
    probes.push(ruby(
        "self_hash_write_symbol_keys",
        format!(
            "def f\n  h = {{}}\n{}  h\nend\n",
            (0..scale)
                .map(|i| format!("  h[:k{i}] = h\n"))
                .collect::<String>()
        ),
    ));
    probes.push(ruby(
        "self_hash_key_and_value",
        format!(
            "def f\n  h = {{}}\n{}  h\nend\n",
            repeat("  h[h] = h\n", scale / 2)
        ),
    ));
    probes.push(ruby(
        "self_merge_bang",
        format!(
            "def f\n  h = {{a: 1}}\n{}  h\nend\n",
            repeat("  h.merge!(h)\n", scale / 4)
        ),
    ));
    probes.push(ruby(
        "mutual_array_refs",
        format!(
            "def f\n  a = []\n  b = []\n{}  a\nend\n",
            repeat("  a << b\n  b << a\n", scale / 2)
        ),
    ));

    // --- Exponential shape growth (doubling) ---
    probes.push(ruby(
        "exp_double_literal",
        format!(
            "def f\n  a = [1]\n{}  a\nend\n",
            repeat("  a = [a, a]\n", 24)
        ),
    ));
    probes.push(ruby(
        "exp_splat_double",
        format!(
            "def f\n  a = [1]\n{}  a\nend\n",
            repeat("  a = [*a, *a]\n", 24)
        ),
    ));

    // --- Deeply nested literals / chains (recursion depth, stack) ---
    let deep = scale.max(200);
    probes.push(ruby(
        "nested_array_literal",
        format!("def f = {}1{}\n", repeat("[", deep), repeat("]", deep)),
    ));
    probes.push(ruby(
        "nested_hash_literal",
        format!("def f = {}1{}\n", repeat("{a: ", deep), repeat("}", deep)),
    ));
    probes.push(ruby(
        "deep_method_chain",
        format!("def f = 1{}\n", repeat(".succ", deep * 2)),
    ));
    probes.push(ruby(
        "deep_string_chain",
        format!("def f = \"x\"{}\n", repeat(".succ", deep)),
    ));
    probes.push(ruby(
        "deep_operator_chain",
        format!("def f = 1{}\n", repeat(" + 1", deep * 2)),
    ));
    probes.push(ruby(
        "deep_nested_block",
        format!(
            "def f\n  x = [1]\n  {}a{}\nend\n",
            repeat("x.map { |a| ", deep),
            repeat(" }", deep)
        ),
    ));
    probes.push(ruby(
        "deep_ternary",
        format!(
            "def f(n) = {}2{}\n",
            repeat("n > 0 ? 1 : (", deep),
            repeat(")", deep)
        ),
    ));
    probes.push(ruby(
        "deep_nested_pattern_match",
        format!(
            "def f(x)\n  case x\n  in {}b{}\n    b\n  else\n    0\n  end\nend\n",
            repeat("[a, ", deep / 2),
            repeat("]", deep / 2)
        ),
    ));
    probes.push(ruby(
        "deep_string_interp",
        format!(
            "def f = {}1{}\n",
            repeat("\"#{", deep / 2),
            repeat("}\"", deep / 2)
        ),
    ));
    probes.push(ruby(
        "deep_begin_rescue",
        format!(
            "def f\n{}    1\n{}end\n",
            repeat("  begin\n", deep),
            repeat("  rescue\n    2\n  end\n", deep)
        ),
    ));
    probes.push(ruby(
        "deep_nested_if",
        format!(
            "def f(v)\n{}v\n{}end\n",
            repeat("if true\n", deep * 2),
            repeat("end\n", deep * 2)
        ),
    ));
    probes.push(ruby(
        "deep_nested_if_narrowing",
        format!(
            "def f(v)\n{}v\n{}end\n",
            repeat("if v.is_a?(Integer)\n", deep * 2),
            repeat("end\n", deep * 2)
        ),
    ));

    // --- Deeply nested branches x multi-variable narrowing (branch-scope depth/fuel bound) ---
    // Nests, at each level, a narrowing of a different local variable via is_a? / nil? /
    // truthiness, to check that stacking branch-scope clones and narrowing layers stays
    // bounded within the depth cap / fuel. Depth is fixed well above `MAX_INFER_NODE_DEPTH`
    // (100) (nested-if body-summary traversal is super-linear in nesting depth regardless of
    // narrowing; that's an existing property. Here we pin down that the depth cap still
    // applies on the narrowing path too, guaranteeing bounded-time completion).
    let vars = 8usize;
    let nest = 150usize;
    probes.push(ruby(
        "deep_nested_narrowing_multivar",
        format!(
            "def f({params})\n{opens}  [{reads}]\n{closes}end\n",
            params = (0..vars)
                .map(|i| format!("v{i}"))
                .collect::<Vec<_>>()
                .join(", "),
            opens = (0..nest)
                .map(|i| {
                    let v = i % vars;
                    match i % 3 {
                        0 => format!("{}if v{v}.is_a?(Integer)\n", repeat("  ", i + 1)),
                        1 => format!("{}unless v{v}.nil?\n", repeat("  ", i + 1)),
                        _ => format!("{}if v{v}\n", repeat("  ", i + 1)),
                    }
                })
                .collect::<String>(),
            reads = (0..vars)
                .map(|i| format!("v{i}"))
                .collect::<Vec<_>>()
                .join(", "),
            closes = (0..nest)
                .map(|i| format!("{}end\n", repeat("  ", nest - i)))
                .collect::<String>(),
        ),
    ));

    // --- Huge unions / huge literals (cardinality cap) ---
    probes.push(ruby(
        "huge_union_case",
        format!(
            "def f(n)\n  case n\n{}  end\nend\n",
            (0..scale * 4)
                .map(|i| format!("  when {i} then :s{i}\n"))
                .collect::<String>()
        ),
    ));
    probes.push(ruby(
        "huge_array_literal",
        format!(
            "def f = [{}]\n",
            (0..scale * 4)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ));
    probes.push(ruby(
        "huge_symbol_union_reassign",
        format!(
            "def f\n  x = :s0\n{}  x\nend\n",
            (1..scale * 4)
                .map(|i| format!("  x = :s{i}\n"))
                .collect::<String>()
        ),
    ));

    // --- Recursive methods (direct / mutual / return-type ref cycles) ---
    probes.push(ruby(
        "direct_recursion_type_growth",
        "def f(n)\n  return [] if n <= 0\n  [f(n - 1)]\nend\n\ndef g = f(100)\n".to_string(),
    ));
    probes.push(ruby(
        "mutual_recursion_chain",
        format!(
            "{}def g = m0(50)\n",
            (0..10)
                .map(|i| format!("def m{i}(n) = n <= 0 ? 0 : m{}(n - 1)\n", (i + 1) % 10))
                .collect::<String>()
        ),
    ));
    probes.push(ruby(
        "return_ref_cycle",
        format!(
            "{}def g = a0\n",
            (0..20)
                .map(|i| format!("def a{i} = a{}\n", (i + 1) % 20))
                .collect::<String>()
        ),
    ));

    // --- attr_reader / initialize mutual recursion (previously hung on large repos) ---
    probes.push(ruby(
        "attr_init_mutual_recursion",
        format!(
            "{}\ndef g = C0.new.dep\n",
            (0..8)
                .map(|i| format!(
                    "class C{i}\n  attr_reader :dep\n  def initialize(dep: C{}.new.dep)\n    @dep = dep\n  end\nend\n",
                    (i + 1) % 8
                ))
                .collect::<String>()
        ),
    ));
    probes.push(ruby(
        "attr_reader_self_ivar_cycle",
        "class A\n  attr_reader :x\n  def initialize\n    @x = compute\n  end\n  def compute = x\nend\n\ndef g = A.new.x\n".to_string(),
    ));

    // --- Circular class / module hierarchies ---
    probes.push(ruby(
        "self_superclass",
        "class A < A\nend\ndef f = A.new\n".to_string(),
    ));
    probes.push(ruby(
        "mutual_superclass",
        "class A < B\nend\nclass B < A\nend\ndef f = A.new\n".to_string(),
    ));
    probes.push(ruby(
        "long_inheritance_chain",
        format!(
            "class C0\nend\n{}def f = C{}.new\n",
            (1..scale)
                .map(|i| format!("class C{i} < C{}\nend\n", i - 1))
                .collect::<String>(),
            scale - 1
        ),
    ));
    probes.push(ruby(
        "self_include",
        "module M\n  include M\nend\nclass C\n  include M\nend\ndef f = C.new\n".to_string(),
    ));
    probes.push(ruby(
        "mutual_include",
        "module A\n  include B\nend\nmodule B\n  include A\nend\nclass C\n  include A\nend\ndef f = C.new\n".to_string(),
    ));
    probes.push(ruby(
        "long_include_chain",
        format!(
            "module M0\nend\n{}class C\n  include M{}\nend\ndef f = C.new\n",
            (1..scale / 2)
                .map(|i| format!("module M{i}\n  include M{}\nend\n", i - 1))
                .collect::<String>(),
            scale / 2 - 1
        ),
    ));
    probes.push(ruby(
        "prepend_cycle",
        "module M\n  prepend M\nend\nclass C\n  prepend M\nend\ndef f = C.new\n".to_string(),
    ));
    probes.push(ruby(
        "extend_self_cycle",
        "module M\n  extend M\n  def x = 1\nend\ndef f = M.x\n".to_string(),
    ));

    // --- Constant alias cycles ---
    probes.push(ruby("const_alias_self", "A = A\ndef f = A\n".to_string()));
    probes.push(ruby(
        "const_alias_cycle",
        "A = B\nB = A\ndef f = A\n".to_string(),
    ));

    // --- Metaprogramming ---
    probes.push(ruby(
        "define_method_loop",
        "class C\n  (0..1000).each do |i|\n    define_method(\"m#{i}\") { i }\n  end\nend\n"
            .to_string(),
    ));
    probes.push(ruby(
        "method_missing_recursion",
        "class C\n  def method_missing(n, *a) = send(n)\nend\n\ndef f = C.new.foo\n".to_string(),
    ));
    probes.push(ruby(
        "send_chain_self",
        "class C\n  def a = send(:b)\n  def b = send(:a)\nend\ndef f = C.new.a\n".to_string(),
    ));

    // --- Self-assignment operators / large-scale reopening ---
    probes.push(ruby(
        "self_plus_assign_array",
        format!(
            "def f\n  x = [1]\n{}  x\nend\n",
            repeat("  x += x\n", scale / 4)
        ),
    ));
    probes.push(ruby(
        "class_reopen",
        format!(
            "{}def f = C.new\n",
            (0..scale)
                .map(|i| format!("class C\n  def m{i} = {i}\nend\n"))
                .collect::<String>()
        ),
    ));

    // --- RBS input (cyclic aliases / cyclic hierarchies / deep generics / huge unions) ---
    probes.push(rbs_load("rbs_alias_self", "type a = a\n".to_string()));
    probes.push(rbs_load(
        "rbs_alias_cycle",
        "type a = b\ntype b = a\n".to_string(),
    ));
    probes.push(rbs_load(
        "rbs_alias_recursive_tree",
        "type tree = [tree, tree]\n".to_string(),
    ));
    probes.push(rbs_load(
        "rbs_alias_chain",
        format!(
            "type t0 = Integer\n{}",
            (1..scale.min(200))
                .map(|i| format!("type t{i} = t{}\n", i - 1))
                .collect::<String>()
        ),
    ));
    probes.push(rbs_load(
        "rbs_self_superclass",
        "class A < A\nend\n".to_string(),
    ));
    probes.push(rbs_load(
        "rbs_mutual_superclass",
        "class A < B\nend\nclass B < A\nend\n".to_string(),
    ));
    probes.push(rbs_load(
        "rbs_mutual_module_include",
        "module A\n  include B\nend\nmodule B\n  include A\nend\n".to_string(),
    ));
    probes.push(rbs_load(
        "rbs_deep_generic",
        format!(
            "class C\n  def f: () -> {}Integer{}\nend\n",
            repeat("Array[", deep),
            repeat("]", deep)
        ),
    ));
    // Pins down that deep nesting of a structured `Generic` (any class other than
    // Array/Hash) stays bounded via the type-size cap (`Foo[Foo[...Integer...]]`).
    probes.push(rbs_load(
        "rbs_deep_custom_generic",
        format!(
            "class Foo[T]\nend\n\nclass C\n  def f: () -> {}Integer{}\nend\n",
            repeat("Foo[", deep),
            repeat("]", deep)
        ),
    ));
    probes.push(rbs_load(
        "rbs_huge_union",
        format!(
            "type t = {}\n",
            (0..scale * 4)
                .map(|i| format!(":s{i}"))
                .collect::<Vec<_>>()
                .join(" | ")
        ),
    ));
    probes.push(ruby_with_rbs(
        "rbs_cyclic_alias_used_from_ruby",
        "def f(x)\n  x\nend\n".to_string(),
        "type a = b\ntype b = a\n".to_string(),
    ));

    // --- RBI input (cyclic hierarchies / self-referential superclass) ---
    probes.push(rbi_load(
        "rbi_self_superclass",
        "class A < A\nend\n".to_string(),
    ));
    probes.push(rbi_load(
        "rbi_mutual_superclass",
        "class A < B\nend\nclass B < A\nend\n".to_string(),
    ));
    probes.push(rbi_load(
        "rbi_deep_generic",
        format!(
            "class C\n  sig {{ returns({}Integer{}) }}\n  def f; end\nend\n",
            repeat("T::Array[", deep.min(400)),
            repeat("]", deep.min(400))
        ),
    ));

    // --- Sig-annotated Ruby (Sorbet) ---
    probes.push(ruby(
        "sig_deep_nested_block",
        format!(
            "class C\n  sig {{ returns(Integer) }}\n  def f\n    x = [1]\n    {}a{}\n  end\nend\n",
            repeat("x.map { |a| ", deep / 2),
            repeat(" }", deep / 2)
        ),
    ));

    probes
}

fn assert_corpus_bounded(scale: usize, slow_bound: Duration, stack_size: usize) {
    let loader = loader();
    // Warm the stdlib cache so per-probe time reflects only the analysis itself.
    let _ = run_probe(&loader, stack_size, |l| {
        let _ = tyda::analysis::playground_analyze("class A\nend\n", "", l, "m.rb");
    });

    // Silence the per-panic hook during the sweep (offenders are reported below).
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let mut hung: Vec<String> = Vec::new();
    let mut panicked: Vec<String> = Vec::new();
    let mut slow: Vec<(Duration, String)> = Vec::new();
    let mut slowest = Duration::ZERO;
    let mut slowest_label = String::new();

    let probes = corpus(scale);
    let total = probes.len();
    for probe in probes {
        let Probe { label, work } = probe;
        match run_probe(&loader, stack_size, work) {
            Outcome::Completed(elapsed) => {
                if elapsed > slowest {
                    slowest = elapsed;
                    slowest_label = label.clone();
                }
                if elapsed > slow_bound {
                    slow.push((elapsed, label));
                }
            }
            Outcome::Panicked => panicked.push(label),
            Outcome::Hang => hung.push(label),
        }
    }

    std::panic::set_hook(prev_hook);

    eprintln!(
        "pathological corpus (scale={scale}): {total} probes, slowest {:?} ({slowest_label})",
        slowest
    );

    assert!(
        hung.is_empty(),
        "{} of {total} pathological inputs did not terminate within {HANG_TIMEOUT:?} \
         (infinite loop / stall). Offenders: {hung:?}",
        hung.len(),
    );
    assert!(
        panicked.is_empty(),
        "{} of {total} pathological inputs panicked. Offenders: {panicked:?}",
        panicked.len(),
    );
    assert!(
        slow.is_empty(),
        "{} of {total} pathological inputs exceeded {slow_bound:?} (a cap likely regressed). \
         First: {:?}",
        slow.len(),
        slow.first(),
    );
}

/// Moderate-size corpus. Always run in CI to catch regressions such as infinite
/// loops, stack overflow, or exponential blowup.
#[test]
fn pathological_inputs_stay_bounded() {
    assert_corpus_bounded(500, SLOW_BOUND, SMALL_PROBE_STACK);
}

/// Margin check that runs a larger-size corpus with a production-deploy-equivalent
/// stack. Heavy, so ignored by default. The goal is confirming headroom on the depth /
/// fuel / cardinality caps (deep nesting, wide unions, exponential shapes), not scaling
/// the raw statement count. Thousands of self-writes cost linear-to-superlinear time on
/// the hover + diagnostics paths, but that's the inherent cost of a huge input, not an
/// infinite loop; the default test covers realistic scale.
/// Run with `cargo test --release --test pathological_inputs -- --ignored --nocapture`.
#[test]
#[ignore = "heavy; run on demand to stress the depth / fuel / type-size caps"]
fn pathological_inputs_stay_bounded_aggressive() {
    assert_corpus_bounded(1500, Duration::from_secs(25), PROD_PROBE_STACK);
}
