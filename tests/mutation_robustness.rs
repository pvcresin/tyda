//! Mutation-robustness tests: take valid Ruby, "mutate" it in every position
//! (the way a developer's half-typed / broken buffer looks), and assert the
//! full user-facing analysis path (RBS render + diagnostics + hover + CodeLens,
//! via `playground_analyze`) never panics and stays bounded. Tyda is an
//! inference engine, not a compiler — it must degrade gracefully on any input.
//!
//! See `docs/incomplete-code-policy.md` for the principles this pins.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rayon::prelude::*;
use tyda::rbs::stdlib_loader::LazyRbsLoader;

fn loader() -> LazyRbsLoader {
    let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
    LazyRbsLoader::new(core_dir)
}

/// Diverse, valid Ruby exercising classes / modules / mixins / blocks / procs /
/// pattern matching / constants / heredocs / interpolation / keyword args /
/// splats / rescue / mutating methods / method chains — the constructs whose
/// half-written forms most stress the analyzer.
const CORPUS: &[&str] = &[
    // Class with ivars, attr, mutating methods, method chains, self return.
    "class Account\n  attr_reader :balance\n  def initialize(balance)\n    @balance = balance\n    @log = []\n  end\n  def deposit(amount)\n    @balance += amount\n    @log << amount\n    @log.map! { |x| x * 2 }\n    self\n  end\n  def history = @log.sort!\nend\n",
    // Module + mixin + constant + pattern matching + interpolation.
    "module Shape\n  PI = 3.14\n  def area = raise NotImplementedError\nend\nclass Circle\n  include Shape\n  def initialize(r) = @r = r\n  def area = PI * (@r**2)\n  def describe(x)\n    case x\n    in [a, b]\n      \"#{a}-#{b}\"\n    in { name: }\n      name\n    else\n      x.to_s\n    end\n  end\nend\n",
    // Control flow + rescue/else + ternary + block + chained calls.
    "def process(items)\n  result = items.map do |item|\n    item.even? ? item * 2 : item.to_s\n  end\n  result.sum\nrescue => e\n  e.message\nend\n",
    // Hashes / records / keyword args / double splat / merge.
    "class Config\n  def build(name:, **opts)\n    { name: name, **opts }\n  end\n  def merge_all(base, *rest)\n    rest.reduce(base) { |acc, h| acc.merge(h) }\n  end\nend\n",
    // Inline RBS annotation + an inferred chain + an undefined constant + a
    // missing method on a known class (exercises the diagnostic paths).
    "class Widget\n  #: (String) -> Integer\n  def size(s) = s.length\n  def use = size(123)\n  def gone = self.totally_missing\n  def ext = Unknown::Thing.new.run\nend\n",
    // Heredoc + string ops + safe navigation + numeric tower.
    "class Report\n  def render(rows)\n    body = rows&.map { |r| r.to_s.upcase }&.join(\"\\n\")\n    <<~TEXT\n      total: #{rows&.size}\n      #{body}\n    TEXT\n  end\nend\n",
];

/// Run the full analysis path under a panic guard. Returns `Err` on panic.
fn analyze_guarded(loader: &LazyRbsLoader, src: &str) -> Result<Duration, ()> {
    let start = Instant::now();
    catch_unwind(AssertUnwindSafe(|| {
        let _ = tyda::analysis::playground_analyze(src, "", loader, "m.rb");
    }))
    .map(|_| start.elapsed())
    .map_err(|_| ())
}

/// Remove one occurrence of `needle` at byte `at` (length in bytes given).
fn without_range(src: &str, at: usize, len: usize) -> String {
    let mut s = String::with_capacity(src.len());
    s.push_str(&src[..at]);
    s.push_str(&src[at + len..]);
    s
}

/// All single mutations of `src` we sweep — every shape a half-written or
/// corrupted buffer takes:
/// - prefix / suffix truncation at every char boundary,
/// - dropping each line,
/// - deleting each single character,
/// - removing each whole block keyword (`end` / `do` / `class` / `module` /
///   `def`) to unbalance scopes.
fn mutations(src: &str) -> Vec<String> {
    let mut out = Vec::new();

    // Prefix / suffix truncation at every char boundary.
    for i in 0..=src.len() {
        if src.is_char_boundary(i) {
            out.push(src[..i].to_string());
            out.push(src[i..].to_string());
        }
    }

    // Drop each line.
    let lines: Vec<&str> = src.lines().collect();
    for skip in 0..lines.len() {
        let kept: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(idx, _)| *idx != skip)
            .map(|(_, l)| *l)
            .collect();
        out.push(kept.join("\n"));
    }

    // Delete each single character (subsumes removing a stray quote / bracket /
    // operator, the most common single-keystroke corruption).
    let mut idx = 0;
    while idx < src.len() {
        let ch_len = src[idx..].chars().next().map(char::len_utf8).unwrap_or(1);
        out.push(without_range(src, idx, ch_len));
        idx += ch_len;
    }

    // Remove each whole block keyword occurrence (one at a time).
    for kw in ["end", "do", "class", "module", "def"] {
        let mut from = 0;
        while let Some(rel) = src[from..].find(kw) {
            let at = from + rel;
            out.push(without_range(src, at, kw.len()));
            from = at + kw.len();
        }
    }

    out
}

/// Extract every ` ```ruby ` fenced block from the scenario corpus — hundreds
/// of diverse, real-world constructs (DSLs, Rails, Sorbet, pattern matching, …)
/// that we get to mutate for free.
fn scenario_ruby_blocks() -> Vec<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let mut files = Vec::new();
    collect_markdown(&root, &mut files);
    let mut blocks = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let mut in_ruby = false;
        let mut current = String::new();
        for line in text.lines() {
            let trimmed = line.trim_start();
            if in_ruby {
                if trimmed.starts_with("```") {
                    if !current.trim().is_empty() {
                        blocks.push(std::mem::take(&mut current));
                    }
                    current.clear();
                    in_ruby = false;
                } else {
                    current.push_str(line);
                    current.push('\n');
                }
            } else if trimmed == "```ruby" {
                in_ruby = true;
            }
        }
    }
    blocks
}

fn collect_markdown(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

/// Bounded mutations for the (large) scenario corpus: evenly-spaced prefix
/// truncations (incremental typing) plus removing each `end` / `"`. Kept light
/// so sweeping hundreds of blocks stays fast.
fn bounded_mutations(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let boundaries: Vec<usize> = (0..=src.len())
        .filter(|i| src.is_char_boundary(*i))
        .collect();
    let step = boundaries.len().div_ceil(16).max(1);
    for (n, &i) in boundaries.iter().enumerate() {
        if n % step == 0 {
            out.push(src[..i].to_string());
        }
    }
    let mut from = 0;
    while let Some(rel) = src[from..].find("end") {
        let at = from + rel;
        out.push(without_range(src, at, 3));
        from = at + 3;
    }
    for (idx, _) in src.match_indices('"') {
        out.push(without_range(src, idx, 1));
    }
    out
}

/// Substitute each character with a delimiter (`[`, `"`, `(`) — corruptions
/// that deletion / truncation can't produce, targeting the delimiter-slicing
/// bug class (`Array[`, lone `"`, `params(`).
fn delimiter_substitutions(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut idx = 0;
    while idx < src.len() {
        let ch_len = src[idx..].chars().next().map(char::len_utf8).unwrap_or(1);
        for repl in ['[', '"', '('] {
            let mut s = String::with_capacity(src.len());
            s.push_str(&src[..idx]);
            s.push(repl);
            s.push_str(&src[idx + ch_len..]);
            out.push(s);
        }
        idx += ch_len;
    }
    out
}

#[test]
fn inference_survives_mutated_scenario_corpus_without_panic() {
    let loader = loader();
    let _ = analyze_guarded(&loader, "class A\nend\n"); // warm cache

    let inputs: Vec<String> = scenario_ruby_blocks()
        .iter()
        .flat_map(|block| bounded_mutations(block))
        .collect();
    let total = inputs.len();

    // Suppress the per-panic hook during the sweep; offenders are surfaced
    // below. The sweep is parallel (each input is independent) to keep this
    // large corpus fast.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let panicked: Vec<String> = inputs
        .par_iter()
        .filter_map(|mutated| {
            analyze_guarded(&loader, mutated)
                .err()
                .map(|()| mutated.clone())
        })
        .collect();

    std::panic::set_hook(prev_hook);

    assert!(
        panicked.is_empty(),
        "analysis panicked on {} of {total} mutated scenario inputs. First offender:\n---\n{}\n---",
        panicked.len(),
        panicked.first().map(String::as_str).unwrap_or(""),
    );
}

/// Heavy on-demand fuzz: the *full* mutation set (incl. every single-char
/// deletion) over the entire scenario corpus. Ignored by default — it is large.
/// Run it (CPU-capped) when stressing the analyzer:
///   RAYON_NUM_THREADS=4 nice -n 19 cargo test --release --test mutation_robustness \
///     inference_survives_aggressive_mutated_scenario_corpus -- --ignored --nocapture
#[test]
#[ignore = "heavy fuzz sweep; run on demand with RAYON_NUM_THREADS capped"]
fn inference_survives_aggressive_mutated_scenario_corpus() {
    let loader = loader();
    let _ = analyze_guarded(&loader, "class A\nend\n");

    let inputs: Vec<String> = scenario_ruby_blocks()
        .iter()
        .flat_map(|block| {
            let mut m = mutations(block);
            m.extend(delimiter_substitutions(block));
            m
        })
        .collect();
    let total = inputs.len();
    eprintln!("aggressive sweep: {total} mutated inputs");

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let panicked: Vec<String> = inputs
        .par_iter()
        .filter_map(|mutated| {
            analyze_guarded(&loader, mutated)
                .err()
                .map(|()| mutated.clone())
        })
        .collect();

    std::panic::set_hook(prev_hook);

    for offender in panicked.iter().take(5) {
        eprintln!("--- PANIC ON ---\n{offender}\n---");
    }
    assert!(
        panicked.is_empty(),
        "analysis panicked on {} of {total} aggressively mutated inputs",
        panicked.len(),
    );
}

#[test]
fn inference_survives_mutated_inputs_without_panic() {
    let loader = loader();

    // Warm the stdlib cache once so per-mutation timing reflects analysis only.
    let _ = analyze_guarded(&loader, CORPUS[0]);

    let inputs: Vec<String> = CORPUS.iter().flat_map(|src| mutations(src)).collect();
    let total = inputs.len();

    // Silence the per-panic hook during the sweep; we surface the offending
    // input ourselves with a clear message. Each mutation is independent, so
    // sweep in parallel like the scenario-corpus test (the serial loop was
    // several minutes of CI wall clock).
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    enum Failure {
        Panic(String),
        Slow(Duration, String),
    }

    let failures: Vec<Failure> = inputs
        .par_iter()
        .filter_map(|mutated| match analyze_guarded(&loader, mutated) {
            Ok(elapsed) if elapsed > Duration::from_secs(2) => {
                Some(Failure::Slow(elapsed, mutated.clone()))
            }
            Ok(_) => None,
            Err(()) => Some(Failure::Panic(mutated.clone())),
        })
        .collect();

    std::panic::set_hook(prev_hook);

    let panicked: Vec<&str> = failures
        .iter()
        .filter_map(|f| match f {
            Failure::Panic(s) => Some(s.as_str()),
            Failure::Slow(..) => None,
        })
        .collect();
    let slow: Vec<(Duration, &str)> = failures
        .iter()
        .filter_map(|f| match f {
            Failure::Slow(d, s) => Some((*d, s.as_str())),
            Failure::Panic(_) => None,
        })
        .collect();

    assert!(
        panicked.is_empty(),
        "analysis panicked on {} of {total} mutated inputs. First offender:\n---\n{}\n---",
        panicked.len(),
        panicked.first().copied().unwrap_or(""),
    );
    assert!(
        slow.is_empty(),
        "analysis exceeded the time bound on {} mutated inputs. First (took {:?}):\n---\n{}\n---",
        slow.len(),
        slow.first().map(|(d, _)| *d).unwrap_or_default(),
        slow.first().map(|(_, s)| *s).unwrap_or(""),
    );
}
