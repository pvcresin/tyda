use std::fs;
use std::process::Command;

fn tyda_bin() -> Command {
    let bin = env!("CARGO_BIN_EXE_tyda");
    Command::new(bin)
}

const RB1: &str = "class A\n  def foo\n    'hello'\n  end\nend\n";
const RB2: &str = "class A\n  def foo\n    42\n  end\nend\n";
const RB3: &str = "class B\n  def foo\n    'x'\n  end\nend\n";
const RB4: &str = "class A\n  def foo(x, y)\n    x + y\n  end\nend\n";
const AASM_RB: &str =
    "class A\n  aasm do\n    state :draft, initial: true\n    event :pay do\n    end\n  end\nend\n";
const RB5: &str = "class A\n  def foo(x)\n    x.unknown_call\n  end\nend\n";

/// The published ruby/vscode-typeprof extension validates the server by matching
/// the *whole* `--version` output against `/^typeprof (\d+.\d+.\d+)$/`, version
/// >= 0.30.1. Assert the output is exactly such a line.
fn assert_typeprof_compatible(stdout: &str) {
    let line = stdout.trim();
    let rest = line
        .strip_prefix("typeprof ")
        .unwrap_or_else(|| panic!("output must be `typeprof X.Y.Z`, got: {stdout:?}"));
    let parts: Vec<u32> = rest.split('.').filter_map(|p| p.parse().ok()).collect();
    assert_eq!(parts.len(), 3, "expected `typeprof X.Y.Z`, got: {stdout:?}");
    assert!(
        parts >= vec![0, 30, 1],
        "typeprof version must be >= 0.30.1, got: {stdout:?}"
    );
}

#[test]
fn version_long_flag() {
    let output = tyda_bin().arg("--version").output().expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_typeprof_compatible(&stdout);
}

#[test]
fn version_short_flag() {
    let output = tyda_bin().arg("-v").output().expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_typeprof_compatible(&stdout);
}

#[test]
fn lsp_version_is_typeprof_compatible() {
    let output = tyda_bin()
        .arg("lsp")
        .arg("--version")
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_typeprof_compatible(&stdout);
}

#[test]
fn no_args_exits_with_error() {
    let output = tyda_bin().output().expect("failed to run");
    assert!(!output.status.success(), "should exit with error code");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Usage"),
        "expected usage message, got: {stderr}"
    );
}

#[test]
fn analyze_single_file() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("a.rb");
    fs::write(&rb_file, RB1).expect("failed to write");

    let output = tyda_bin()
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("class A"),
        "expected RBS output, got: {stdout}"
    );
    assert!(
        stdout.contains("def foo"),
        "expected method sig, got: {stdout}"
    );
}

#[test]
fn analyze_single_file_uses_rbs_collection_config() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let collection_dir = dir.path().join("vendor").join("rbs_collection");
    fs::create_dir_all(&collection_dir).expect("failed to create collection");
    fs::write(
        dir.path().join("rbs_collection.yaml"),
        "path: vendor/rbs_collection\ngems: []\n",
    )
    .expect("failed to write collection config");
    fs::write(
        collection_dir.join("external_source.rbs"),
        "class ExternalSource\n  def label: () -> String\nend\n",
    )
    .expect("failed to write rbs");
    let rb_file = dir.path().join("a.rb");
    fs::write(
        &rb_file,
        "class App\n  def collection_label(source)\n    source.label\n  end\nend\n\nApp.new.collection_label(ExternalSource.new)\n",
    )
    .expect("failed to write ruby");

    let output = tyda_bin()
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("def collection_label: (ExternalSource source) -> String"),
        "expected RBS collection method type, got: {stdout}"
    );
}

#[test]
fn analyze_renders_struct_in_default_compact_scan() {
    // Regression: the default (non-verbose) CLI renders the workspace registry
    // built by the compact scan, whose `retain_file_facts` keeps a class only
    // when its members carry a matching `file_path`. `Struct.new`/`Data.define`
    // generated members must therefore be associated with the file, or the
    // whole type silently disappears from `tyda file.rb` output.
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("shapes.rb");
    fs::write(
        &rb_file,
        "Point = Struct.new(:x, :y) do\n  def dist = x + y\nend\nMeasure = Data.define(:amount, :unit)\n",
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("class Point"),
        "missing Struct class: {stdout}"
    );
    assert!(
        stdout.contains("def dist"),
        "missing Struct block method: {stdout}"
    );
    assert!(
        stdout.contains("def self.members"),
        "missing Struct members: {stdout}"
    );
    assert!(
        stdout.contains("class Measure"),
        "missing Data class: {stdout}"
    );
}

#[test]
fn analyze_directory() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::write(dir.path().join("a.rb"), RB2).expect("failed to write");
    fs::write(dir.path().join("b.rb"), RB3).expect("failed to write");

    let output = tyda_bin()
        .arg(dir.path().to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class A"), "missing A: {stdout}");
    assert!(stdout.contains("class B"), "missing B: {stdout}");
}

#[test]
fn verbose_shows_file_names() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("a.rb");
    fs::write(&rb_file, RB4).expect("failed to write");

    let output = tyda_bin()
        .arg("--verbose")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("a.rb"),
        "verbose should print file name to stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("ms"),
        "verbose should print timing, got: {stderr}"
    );
}

#[test]
fn debug_shows_timing_report() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("holes.rb");
    fs::write(&rb_file, RB5).expect("failed to write");

    let output = tyda_bin()
        .arg("--debug")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DEBUG summary"),
        "debug should print summary, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG file"),
        "debug should print file timings, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG holes"),
        "debug should print hole summary, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG hole"),
        "debug should print per-hole explain, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG scenario_seed_begin"),
        "debug should print scenario seed, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG hole_heat rank="),
        "debug should print hole heatmap, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG unresolved rank="),
        "debug should print unresolved ranking, got: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG collectors enabled="),
        "debug should print collector summary, got: {stderr}"
    );
}

#[test]
fn diagnostics_flag_outputs_json_lines() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("missing.rb");
    fs::write(
        &rb_file,
        "class Widget\n  def identity = object_id\nend\n\nWidget.new.missing\n",
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("class A"),
        "diagnostics mode should not render RBS, got: {stdout}"
    );
    let first_line = stdout.lines().next().expect("expected diagnostic JSON");
    assert_eq!(
        stdout.lines().count(),
        1,
        "universal Object methods should not be reported as missing: {stdout}"
    );
    let diagnostic: serde_json::Value =
        serde_json::from_str(first_line).expect("diagnostic should be JSON");
    assert_eq!(diagnostic["code"], "missing_method");
    assert_eq!(diagnostic["severity"], "warning");
    assert_eq!(diagnostic["method_name"], "missing");
    assert_eq!(diagnostic["line"], 5);
    assert!(
        diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains("missing")),
        "diagnostic message should name missing method: {diagnostic:?}"
    );
}

#[test]
fn diagnostics_flag_honors_line_ignore_comments() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("ignored.rb");
    fs::write(
        &rb_file,
        r#"class Widget
  #: (String) -> Integer
  def foo(s)
    s.length
  end
end

Widget.new.missing # tyda: ignore[missing_method]
Widget.new.foo(1) # tyda: ignore[argument_type_mismatch]
Widget.new.missing
Widget.new.foo(1)
"#,
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let diagnostics: Vec<(String, u64)> = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("diagnostic JSON"))
        .map(|diagnostic| {
            (
                diagnostic["code"].as_str().unwrap().to_string(),
                diagnostic["line"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        diagnostics,
        vec![
            ("missing_method".to_string(), 10),
            ("argument_type_mismatch".to_string(), 11),
        ],
        "only the unsuppressed lines should remain: {stdout}"
    );
}

#[test]
fn diagnostics_flag_reports_unused_line_ignore_comments() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("unused_ignore.rb");
    fs::write(
        &rb_file,
        r#"class Widget
  #: (String) -> Integer
  def foo(s)
    s.length
  end
end

Widget.new.missing # tyda: ignore[argument_type_mismatch]
Widget.new.foo(1) # tyda: ignore[missing_method]
Widget.new.foo("ok") # tyda: ignore
"#,
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let diagnostics: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("diagnostic JSON"))
        .collect();
    assert_eq!(
        diagnostics.len(),
        5,
        "unused ignores must be reported: {stdout}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "missing_method" && diagnostic["line"] == 8)
    );
    assert!(diagnostics.iter().any(
        |diagnostic| diagnostic["code"] == "argument_type_mismatch" && diagnostic["line"] == 9
    ));
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic["code"] == "unused_ignore")
            .count(),
        3,
        "all non-matching ignores must be reported: {stdout}"
    );
}

#[test]
fn diagnostics_flag_suppresses_class_body_dsl_noise() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("dsl.rb");
    fs::write(
        &rb_file,
        r#"class Project < ApplicationRecord
  belongs_to :account
  has_many :items
  validates :name, presence: true
  before_validation :normalize
  scope :enabled, -> { nil }

  def run
    missing_runtime
    optional
  end
end

class API < Grape::API
  desc "list"
  tags ["items"]
  params do
    requires :id, type: Integer
    optional :name, type: String
  end
  route_setting :auth, true
  get ":id" do
    present :item, {}
  end
end
"#,
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let method_names: Vec<String> = stdout
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .expect("diagnostic should be JSON")["method_name"]
                .as_str()
                .expect("diagnostic should include method_name")
                .to_string()
        })
        .collect();
    assert_eq!(
        method_names,
        vec!["missing_runtime".to_string(), "optional".to_string()],
        "class-body DSL calls should be suppressed without hiding method-body calls: {stdout}"
    );
}

#[test]
fn diagnostics_flag_resolves_rails_hash_like_accessors() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::write(dir.path().join("Gemfile"), "gem \"rails\"\n").expect("failed to write Gemfile");
    let controllers_dir = dir.path().join("app").join("controllers");
    let models_dir = dir.path().join("app").join("models");
    fs::create_dir_all(&controllers_dir).expect("failed to create controllers dir");
    fs::create_dir_all(&models_dir).expect("failed to create models dir");
    fs::write(
        controllers_dir.join("items_controller.rb"),
        r#"class ItemsController < ActionController::Base
  def show
    params[:id]
    params.fetch(:name)
    request.remote_ip
    flash[:notice]
    session[:user_id]
    respond_to do |format|
      format.html { render }
    end
    missing_runtime
  end
end
"#,
    )
    .expect("failed to write controller");
    fs::write(
        models_dir.join("store.rb"),
        r#"class HashWithIndifferentAccess
end

class Store
  def options = HashWithIndifferentAccess.new

  def read
    options[:name]
    options.fetch(:name)
    missing_model
  end

  def duration_seconds = 5.minutes.to_i

  def duration_ago = 5.minutes.ago

  def title = I18n.t(:title)

  def localized = I18n.with_locale(:ja) { I18n.t(:title) }

  def rails_config = Rails.configuration

  def rails_cache = Rails.cache

  def rails_logger = Rails.logger

  def custom_config = Rails.configuration.x

  def cached = Rails.cache.fetch("key")

  def delete_cached = Rails.cache.delete("key")
end
"#,
    )
    .expect("failed to write model");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(dir.path().to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let diagnostics: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("diagnostic should be JSON"))
        .collect();
    let method_names: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| {
            diagnostic["method_name"]
                .as_str()
                .expect("diagnostic should include method_name")
                .to_string()
        })
        .collect();
    assert!(
        method_names.contains(&"missing_runtime".to_string())
            && method_names.contains(&"missing_model".to_string()),
        "real missing methods should still be reported: {stdout}"
    );
    assert!(
        !method_names.contains(&"[]".to_string())
            && !method_names.contains(&"fetch".to_string())
            && !method_names.contains(&"to_i".to_string())
            && !method_names.contains(&"ago".to_string())
            && !method_names.contains(&"t".to_string())
            && !method_names.contains(&"with_locale".to_string())
            && !method_names.contains(&"configuration".to_string())
            && !method_names.contains(&"cache".to_string())
            && !method_names.contains(&"logger".to_string())
            && !method_names.contains(&"x".to_string())
            && !method_names.contains(&"delete".to_string())
            && !method_names.contains(&"request".to_string())
            && !method_names.contains(&"flash".to_string())
            && !method_names.contains(&"session".to_string())
            && !method_names.contains(&"respond_to".to_string()),
        "Rails hash-like, duration, I18n, singleton, and controller helper methods should not be reported as missing: {stdout}"
    );
}

#[test]
fn nonexistent_path_still_completes() {
    let output = tyda_bin()
        .arg("/tmp/tyda-nonexistent-path-xyz")
        .output()
        .expect("failed to run");
    assert!(output.status.success());
}

#[test]
fn help_flag() {
    let output = tyda_bin().arg("--help").output().expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--verbose"), "help should list --verbose");
    assert!(stdout.contains("--debug"), "help should list --debug");
    assert!(
        stdout.contains("--diagnostics"),
        "help should list --diagnostics"
    );
    assert!(
        stdout.contains("--capability-matrix"),
        "help should list --capability-matrix"
    );
    assert!(stdout.contains("--lsp"), "help should list --lsp");
}

#[test]
fn capability_matrix_flag_prints_matrix() {
    let output = tyda_bin()
        .arg("--capability-matrix")
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("# Capability Matrix"),
        "expected capability matrix output, got: {stdout}"
    );
    assert!(
        stdout.contains("Ruby source inference"),
        "expected a capability row, got: {stdout}"
    );
}

#[test]
fn positional_plus_kwarg_forwarding_recursion_terminates() {
    // Regression test: analyzing a class where three methods forward a
    // keyword argument alongside a positional arg and the last method
    // recursively instantiates the class used to hang indefinitely because
    // `resolve_keyword_param_type` spawned a fresh recursion guard on each
    // entry, defeating the visiting-set cycle detection in
    // `resolve_method_params_with_caller_context`.
    const RECURSIVE_KWARG_FORWARD: &str = "class H\n  def a(x, nested: false)\n    b(x, nested:)\n  end\n  def b(x, nested: false)\n    c(x, nested:)\n  end\n  def c(x, nested: false)\n    H.new.a(x, nested: true)\n  end\nend\n";
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("recursion.rb");
    fs::write(&rb_file, RECURSIVE_KWARG_FORWARD).expect("failed to write");

    // Wall-clock cap: pre-fix, this ran for hours. A healthy run finishes
    // in under 100ms; 5s is orders of magnitude over that while still
    // detecting a hang reliably.
    let start = std::time::Instant::now();
    let output = tyda_bin()
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "tyda hung on recursive kwarg forwarding: took {elapsed:?}"
    );
    assert!(output.status.success());
}

#[test]
fn dsl_flag_can_disable_auto_detected_collector() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("a.rb");
    fs::write(&rb_file, AASM_RB).expect("failed to write");

    let auto_output = tyda_bin()
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(auto_output.status.success());
    let auto_stdout = String::from_utf8_lossy(&auto_output.stdout);
    assert!(
        auto_stdout.contains("def draft?"),
        "auto detection should enable AASM collector: {auto_stdout}"
    );

    let disabled_output = tyda_bin()
        .arg("--dsl=-aasm")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(disabled_output.status.success());
    let disabled_stdout = String::from_utf8_lossy(&disabled_output.stdout);
    assert!(
        !disabled_stdout.contains("def draft?"),
        "--dsl -aasm should disable AASM collector: {disabled_stdout}"
    );
}

#[test]
fn diagnostics_flag_skips_missing_methods_on_undefined_constants() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("undef.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  def bare = bogus_local_helper\n", // A is known -> flagged
            "  def chained = Foo.new.f\n",       // Foo is undefined -> must NOT flag `f`
            "end\n",
            "class Empty\n",
            "end\n",
            "Empty.new.missing\n", // declared (empty) class -> still flagged
        ),
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let messages: Vec<String> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "missing_method")
        .filter_map(|d| d["message"].as_str().map(str::to_string))
        .collect();

    // The real problem is the undefined constant `Foo`, not the method `f` —
    // don't claim `f` is missing on a class we don't know.
    assert!(
        !messages.iter().any(|m| m.contains("`f`")),
        "method on an undefined constant must not be flagged: {messages:?}"
    );
    // A missing method on a *known* class (user-defined, even empty) is flagged.
    assert!(
        messages.iter().any(|m| m.contains("bogus_local_helper")),
        "missing method on a known class should be flagged: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("`missing`")),
        "missing method on a declared empty class should be flagged: {messages:?}"
    );
}

// A class whose full ancestor chain is known (only `Object` above it) must keep
// reporting a typo'd call. This is the true-positive guard: gating must not mute
// missing-method detection on receivers whose method surface is fully knowable.
#[test]
fn diagnostics_flag_reports_typo_on_fully_known_class() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("known.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  def identity = object_id\n",
            "  def run = definitly_a_typo\n", // A's ancestors are all known -> flag
            "end\n",
        ),
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let messages: Vec<String> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "missing_method")
        .filter_map(|d| d["message"].as_str().map(str::to_string))
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("definitly_a_typo")),
        "typo on a fully-known class must still be reported: {messages:?}"
    );
}

// When a receiver's ancestor chain has an unresolvable edge, the method surface
// is unknowable, so a missing-method diagnostic would be a guess, not a proven
// "No". These three shapes must stay silent.
#[test]
fn diagnostics_flag_gates_missing_method_on_incomplete_ancestors() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");

    // (1) Unresolved mixin edge: `include UnknownMixin` (never declared).
    let mixin_file = dir.path().join("mixin.rb");
    fs::write(
        &mixin_file,
        concat!(
            "class Host\n",
            "  include UnknownMixin\n",
            "  def run = maybe_from_mixin\n",
            "end\n",
        ),
    )
    .expect("failed to write mixin file");

    // (2) `instance_eval`-style block where `self` degrades to bare `Object`:
    // the DSL block receiver is unknown, so a bare call must not be flagged.
    let block_file = dir.path().join("block.rb");
    fs::write(
        &block_file,
        concat!("Thing.configure do\n", "  config.setting = 1\n", "end\n"),
    )
    .expect("failed to write block file");

    for (label, file) in [("mixin", &mixin_file), ("block", &block_file)] {
        let output = tyda_bin()
            .arg("--diagnostics")
            .arg(file.to_str().unwrap())
            .output()
            .expect("failed to run");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let missing: Vec<&str> = stdout
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|d| d["code"] == "missing_method")
            .filter_map(|d| d["method_name"].as_str().map(|_| ""))
            .collect();
        assert!(
            missing.is_empty(),
            "{label}: missing_method must be gated when ancestors are incomplete: {stdout}"
        );
    }
}

// In a Sorbet project (`sorbet/rbi/` present) a framework-base subclass owes
// part of its method surface to generated RBI (tapioca dsl: schema attributes,
// scopes). When no declaration-backed methods were merged for the receiver,
// that surface cannot be proven, so a missing method stays Unknown and must
// not be reported. Without the RBI source
// (see diagnostics_flag_suppresses_class_body_dsl_noise) the code is the whole
// surface, and with the receiver's dsl RBI merged
// (see diagnostics_flag_reports_typo_on_rbi_modeled_framework_subclass) the
// surface is provable again — real typos keep being reported in both.
#[test]
fn diagnostics_flag_gates_framework_base_subclass_in_sorbet_project() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rbi_dir = dir.path().join("sorbet").join("rbi");
    fs::create_dir_all(&rbi_dir).expect("failed to create rbi dir");
    fs::write(rbi_dir.join("shim.rbi"), "class SomeShim\nend\n").expect("failed to write rbi");
    let rb_file = dir.path().join("model.rb");
    fs::write(
        &rb_file,
        concat!(
            "class Item < ApplicationRecord\n",
            "  def summary = created_at_label\n",
            "end\n",
        ),
    )
    .expect("failed to write model file");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("missing_method"),
        "framework-base subclass surface is unknowable with generated RBI present, must gate: {stdout}"
    );
}

// Generated declarations compose with the source definition: a Ruby-defined
// model whose tapioca dsl RBI is present resolves the generated methods (no
// FP) and its surface becomes provable, so a real typo is reported again.
#[test]
fn diagnostics_flag_reports_typo_on_rbi_modeled_framework_subclass() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let dsl_dir = dir.path().join("sorbet").join("rbi").join("dsl");
    fs::create_dir_all(&dsl_dir).expect("failed to create dsl dir");
    fs::write(
        dsl_dir.join("item.rbi"),
        concat!(
            "class Item\n",
            "  sig { returns(Integer) }\n",
            "  def person_id; end\n",
            "end\n",
        ),
    )
    .expect("failed to write dsl rbi");
    let rb_file = dir.path().join("model.rb");
    fs::write(
        &rb_file,
        concat!(
            "class Item < ApplicationRecord\n",
            "  def summary = person_id\n",
            "  def oops = person_id_typo_xyz\n",
            "end\n",
        ),
    )
    .expect("failed to write model file");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("person_id\""),
        "dsl RBI method must resolve on the source-defined model: {stdout}"
    );
    assert!(
        stdout.contains("person_id_typo_xyz"),
        "a modeled surface must keep reporting real typos: {stdout}"
    );
}

// An association scope lambda (`has_many :x, -> { scope }, class_name: 'Y'`) is
// instance_exec'd on the target class's relation at runtime, so a bare call in
// the lambda resolves against the target class, not the declaring class. With
// generated RBI making both classes' surfaces provable, the target's scope
// resolves (no FP) while a genuinely undefined name is still reported — on the
// target class, matching the runtime self.
#[test]
fn diagnostics_flag_resolves_association_scope_lambda_on_target_class() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let dsl_dir = dir.path().join("sorbet").join("rbi").join("dsl");
    fs::create_dir_all(&dsl_dir).expect("failed to create dsl dir");
    fs::write(
        dsl_dir.join("models.rbi"),
        concat!(
            "class ShopUser\n",
            "  sig { returns(Integer) }\n",
            "  def person_id; end\n",
            "end\n",
            "class Membership\n",
            "  sig { returns(Membership) }\n",
            "  def self.owner_shared(pid); end\n",
            "end\n",
        ),
    )
    .expect("failed to write dsl rbi");
    let rb_file = dir.path().join("model.rb");
    fs::write(
        &rb_file,
        concat!(
            "class ShopUser < ApplicationRecord\n",
            "  has_many :shared, ->(u) { owner_shared(u.person_id) }, class_name: 'Membership'\n",
            "  has_many :bogus, -> { totally_undefined_scope }, class_name: 'Membership'\n",
            "end\n",
            "class Membership < ApplicationRecord\n",
            "end\n",
        ),
    )
    .expect("failed to write model file");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("owner_shared"),
        "target-class scope must resolve inside the association scope lambda: {stdout}"
    );
    // A truly undefined name is reported against the target class (runtime self),
    // never against the declaring `ShopUser`.
    assert!(
        !stdout.contains("not found for `ShopUser`"),
        "the declaring class must not be the receiver for scope-lambda bare calls: {stdout}"
    );
}

// Many files referencing the same external RBI class share one built shape, yet
// each file's diagnostics must be identical to analyzing it in isolation and
// stable across runs. Guards the shared external-closure layer: a fully-known
// external class flags typos, its declared methods resolve, and the batch is
// deterministic regardless of the merge-sharing order.
#[test]
fn diagnostics_shared_external_rbi_shape_is_stable_across_many_files() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rbi_dir = dir.path().join("sorbet").join("rbi");
    fs::create_dir_all(&rbi_dir).expect("failed to create rbi dir");
    // A fully-defined external class (no generated-DSL ancestor), so typos on
    // it are reported while its declared methods resolve.
    fs::write(
        rbi_dir.join("widget.rbi"),
        concat!(
            "class Widget\n",
            "  sig { returns(String) }\n",
            "  def render; end\n",
            "  sig { returns(Integer) }\n",
            "  def width; end\n",
            "end\n",
        ),
    )
    .expect("failed to write rbi");

    let app_dir = dir.path().join("app");
    fs::create_dir_all(&app_dir).expect("failed to create app dir");
    const FILE_COUNT: usize = 12;
    let files: Vec<std::path::PathBuf> = (0..FILE_COUNT)
        .map(|i| {
            let path = app_dir.join(format!("use_{i}.rb"));
            fs::write(
                &path,
                format!(
                    concat!(
                        "class Use{i}\n",
                        "  def ok\n",
                        "    w = Widget.new\n",
                        "    w.render\n", // declared -> no diagnostic
                        "    w.width\n",  // declared -> no diagnostic
                        "  end\n",
                        "  def bad\n",
                        "    Widget.new.no_such_widget_method\n", // typo -> flagged
                        "  end\n",
                        "end\n",
                    ),
                    i = i
                ),
            )
            .expect("failed to write app file");
            path
        })
        .collect();

    let run = || {
        let mut cmd = tyda_bin();
        cmd.arg("--diagnostics");
        for file in &files {
            cmd.arg(file.to_str().unwrap());
        }
        let output = cmd.output().expect("failed to run");
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).into_owned()
    };

    let first = run();
    let second = run();
    assert_eq!(
        first, second,
        "batch output must be deterministic across runs"
    );

    let diags: Vec<serde_json::Value> = first
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    // Exactly one missing-method diagnostic per file (the typo), and none for
    // the declared `render` / `width` calls resolved through the shared shape.
    let missing: Vec<&serde_json::Value> = diags
        .iter()
        .filter(|d| d["code"] == "missing_method")
        .collect();
    assert_eq!(
        missing.len(),
        FILE_COUNT,
        "each file reports its own typo, declared methods resolve: {first}"
    );
    assert!(
        missing
            .iter()
            .all(|d| d["method_name"] == "no_such_widget_method"),
        "only the typo is flagged: {first}"
    );
}

#[test]
fn diagnostics_flag_reports_unresolved_constant_receiver() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("const.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  def chained = Foo.new.f\n", // Foo undefined -> flag Foo, NOT f
            "  def known = String.new\n",  // String is stdlib -> no flag
            "  def later = Bar.new\n",     // Bar defined below (forward ref) -> no flag
            "  def env = ENV.fetch(\"X\")\n", // ENV is a builtin special -> no flag
            "  def gc = GC.stat\n",        // GC is a builtin special -> no flag
            "end\n",
            "class Bar\n",
            "end\n",
        ),
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let diags: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect();

    // `f` must NOT be flagged as a missing method (the constant is the problem).
    assert!(
        !diags.iter().any(|d| d["code"] == "missing_method"
            && d["message"].as_str().unwrap_or("").contains("`f`")),
        "method on an undefined constant must not be flagged: {diags:?}"
    );

    let constants: Vec<&serde_json::Value> = diags
        .iter()
        .filter(|d| d["code"] == "unresolved_constant")
        .collect();
    // Exactly one: `Foo`. `String` (stdlib) and `Bar` (declared below) are known.
    assert_eq!(constants.len(), 1, "only `Foo` is unresolved: {diags:?}");
    let foo = constants[0];
    assert_eq!(foo["severity"], "information");
    assert_eq!(foo["line"], 2);
    assert!(
        foo["message"].as_str().unwrap_or("").contains("Foo"),
        "message should name the constant: {foo:?}"
    );
}

// Default gems bundled with Ruby whose rbs gem doesn't ship RBS (irb / reline, etc.)
// are always present in the runtime environment, so we don't report `unresolved_constant`
// for them. A genuinely undefined constant (Foo) is still reported.
#[test]
fn diagnostics_suppress_unresolved_constant_for_rbs_unshipped_default_gems() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    // Rails project detection loads known namespaces into the registry (existing suppression path).
    fs::write(dir.path().join("Gemfile"), "gem \"rails\"\n").expect("write Gemfile");
    let rb_file = dir.path().join("irb_conf.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  def a = IRB.conf[:SAVE_HISTORY]\n", // irb: RBS not shipped but bundled -> no flag
            "  def b = Reline.readline(\"> \")\n", // reline: same as above -> no flag
            "  def c = Foo.new\n",                 // genuinely undefined -> flag
            "end\n",
        ),
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(dir.path().to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let constants: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "unresolved_constant")
        .collect();
    assert_eq!(
        constants.len(),
        1,
        "only `Foo` is unresolved: {constants:?}"
    );
    assert!(
        constants[0]["message"]
            .as_str()
            .unwrap_or("")
            .contains("Foo"),
        "message should name the constant: {constants:?}"
    );
}

// A bare constant inside a class whose superclass is unresolved might be
// inherited through that ancestor, so it must not be flagged as undefined.
// A bare constant inside a fully-known scope is still flagged.
#[test]
fn diagnostics_flag_gates_unresolved_constant_on_incomplete_scope() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");

    // Enclosing class has an unresolvable superclass -> bare constant gated.
    let gated = dir.path().join("gated.rb");
    fs::write(
        &gated,
        "class Parent < UnknownBase\n  def run = SomeConst.new\nend\n",
    )
    .expect("failed to write gated");

    // Enclosing class's chain is fully known -> bare constant still flagged.
    let flagged = dir.path().join("flagged.rb");
    fs::write(&flagged, "class Plain\n  def run = MissingConst.new\nend\n")
        .expect("failed to write flagged");

    let gated_out = diagnostics_for_target(&gated);
    assert!(
        !gated_out
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .any(|d| d["code"] == "unresolved_constant"),
        "constant under an unresolved superclass must not be flagged: {gated_out}"
    );

    let flagged_out = diagnostics_for_target(&flagged);
    assert!(
        flagged_out
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .any(|d| d["code"] == "unresolved_constant"
                && d["message"].as_str().unwrap_or("").contains("MissingConst")),
        "constant in a fully-known scope must still be flagged: {flagged_out}"
    );
}

// Constant references on a dynamic self / variable receiver (`self::CONST` /
// `self.class::CONST` / a variable receiver) have a flow-dependent, indeterminate
// owner, so we don't report `unresolved_constant` for them. A genuinely undefined
// constant with a static owner is still reported as before.
#[test]
fn diagnostics_suppress_unresolved_constant_dynamic_receiver() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("dynamic.rb");
    fs::write(
        &rb_file,
        concat!(
            "module Attrs\n",
            "  module ClassMethods\n",
            "    def a = self::ACCESSIBLE_ATTRIBUTES.include?(:x)\n", // self:: -> suppressed
            "    def b = self.class::ACCESSIBLE_ATTRIBUTES.include?(:x)\n", // self.class:: -> suppressed
            "  end\n",
            "end\n",
            "class Loader\n",
            "  def build(klass) = klass::CONFIG.fetch(:x)\n", // untyped variable -> suppressed
            "  def bad = RealUndefinedConst.new\n",           // static owner -> reported
            "end\n",
        ),
    )
    .expect("failed to write");

    let stdout = diagnostics_for_target(&rb_file);
    let constants: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|d| d["code"] == "unresolved_constant")
        .collect();
    // The 3 dynamic-owner cases are suppressed; only the static `RealUndefinedConst` remains.
    assert_eq!(
        constants.len(),
        1,
        "only the static-owner undefined constant is reported: {stdout}"
    );
    assert!(
        constants[0]["message"]
            .as_str()
            .unwrap_or("")
            .contains("RealUndefinedConst"),
        "the reported constant is the static-owner one: {constants:?}"
    );
}

// A variable receiver whose owner is statically resolved to a single concrete class
// (an annotated `singleton(Foo)` param / a narrowed local) plugs into normal resolution
// as `Foo::CONST`. It's reported only if genuinely undefined on the resolved owner.
#[test]
fn diagnostics_resolve_unresolved_constant_static_singleton_receiver() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("resolve.rb");
    fs::write(
        &rb_file,
        concat!(
            "class Foo\n",
            "  CONFIG = { a: 1 }\n",
            "end\n",
            "class Loader\n",
            "  #: (singleton(Foo)) -> untyped\n",
            "  def good(klass) = klass::CONFIG.fetch(:a)\n", // Foo::CONFIG exists -> no report
            "  #: (singleton(Foo)) -> untyped\n",
            "  def bad(klass) = klass::MISSING.fetch(:a)\n", // Foo::MISSING is undefined -> report
            "  def narrowed\n",
            "    klass = Foo\n",
            "    klass::CONFIG.fetch(:a)\n", // already narrowed -> no report
            "  end\n",
            "end\n",
        ),
    )
    .expect("failed to write");

    let stdout = diagnostics_for_target(&rb_file);
    let constants: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|d| d["code"] == "unresolved_constant")
        .collect();
    assert_eq!(
        constants.len(),
        1,
        "only the resolved-owner missing constant is reported: {stdout}"
    );
    assert!(
        constants[0]["message"]
            .as_str()
            .unwrap_or("")
            .contains("Foo::MISSING"),
        "dynamic owner resolved to the concrete class `Foo`: {constants:?}"
    );
}

// `--debug --diagnostics` reports per-reason gating suppression counts on
// stderr, used to measure the gating effect and catch over-silencing.
#[test]
fn diagnostics_debug_reports_gating_suppression_counts() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    // Object-receiver suppression: `self` degrades to bare Object in a block.
    let block = dir.path().join("block.rb");
    fs::write(&block, "Thing.configure do\n  config.x = 1\nend\n").expect("write block");
    // Incomplete-ancestors suppression: unresolved mixin edge.
    let mixin = dir.path().join("mixin.rb");
    fs::write(
        &mixin,
        "class Host\n  include UnknownMixin\n  def run = maybe\nend\n",
    )
    .expect("write mixin");

    let output = tyda_bin()
        .arg("--debug")
        .arg("--diagnostics")
        .arg(block.to_str().unwrap())
        .arg(mixin.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DEBUG gating_suppressed reason=object_receiver count="),
        "debug output should report object_receiver suppression count: {stderr}"
    );
    assert!(
        stderr.contains("DEBUG gating_suppressed reason=incomplete_ancestors count="),
        "debug output should report incomplete_ancestors suppression count: {stderr}"
    );
}

/// Run `--diagnostics` on `rb_file` and return only argument-type-mismatch
/// diagnostics, parsed as JSON.
fn argument_type_mismatches(rb_file: &std::path::Path) -> Vec<serde_json::Value> {
    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "argument_type_mismatch")
        .collect()
}

#[test]
fn experimental_arity_check_is_off_by_default_and_on_with_env() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("arity.rb");
    fs::write(
        &rb_file,
        "#: (Integer, Integer) -> Integer\ndef add(a, b)\n  a + b\nend\n\n#: (name: String) -> String\ndef greet(name:)\n  name\nend\n\nadd(1)\nadd(1, 2, 3)\nadd(1, 2)\ngreet\ngreet(name: \"x\")\n",
    )
    .expect("failed to write");

    // The experimental check is off by default (no env var set).
    let off = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(off.status.success());
    let off_stdout = String::from_utf8_lossy(&off.stdout);
    assert!(
        !off_stdout.contains("arity_mismatch"),
        "arity check must be off by default: {off_stdout}"
    );

    // Setting the env var turns on arity reporting.
    let on = tyda_bin()
        .env("TYDA_EXPERIMENTAL_CHECKS", "1")
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(on.status.success());
    let on_stdout = String::from_utf8_lossy(&on.stdout);
    let arity: Vec<serde_json::Value> = on_stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "arity_mismatch")
        .collect();
    // 3 cases: add(1) too few / add(1,2,3) too many / greet missing required keyword.
    // add(1,2) and greet(name:) are correct, so they don't trigger a diagnostic.
    assert_eq!(arity.len(), 3, "expected 3 arity diagnostics: {arity:?}");
    let messages: Vec<String> = arity
        .iter()
        .map(|d| d["message"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("given 1, expected 2")),
        "too few: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("given 3, expected 2")),
        "too many: {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("missing keyword: :name")),
        "missing keyword: {messages:?}"
    );
}

#[test]
fn experimental_union_member_missing_method_is_off_by_default_and_on_with_env() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("union_member.rb");
    fs::write(
        &rb_file,
        "class Corporation\n  #: () -> String\n  def name\n    \"x\"\n  end\nend\n\n#: (Corporation?) -> void\ndef process(corp)\n  corp.name\nend\n",
    )
    .expect("failed to write");

    // The experimental check is off by default (no env var set).
    let off = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(off.status.success());
    let off_stdout = String::from_utf8_lossy(&off.stdout);
    assert!(
        !off_stdout.contains("union_member_missing_method"),
        "union member check must be off by default: {off_stdout}"
    );

    // Setting the env var reports the missing nil member at information severity.
    let on = tyda_bin()
        .env("TYDA_EXPERIMENTAL_CHECKS", "1")
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(on.status.success());
    let on_stdout = String::from_utf8_lossy(&on.stdout);
    let diags: Vec<serde_json::Value> = on_stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "union_member_missing_method")
        .collect();
    assert_eq!(
        diags.len(),
        1,
        "expected 1 union member diagnostic: {diags:?}"
    );
    assert_eq!(diags[0]["severity"], "information");
    let message = diags[0]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("Method `name` not found for union member `nil`")
            && message.contains("receiver `Corporation | nil`"),
        "message shape: {message}"
    );
}

// Plain resolution misinfers `Oj.load` as `Kernel#load -> bool` via the universal
// fallback, causing a false positive when a `bool` flows into an argument check that
// expects a `Hash`. When the oj gem is declared, the plugin resolves it to untyped
// (a JSON value can be anything), and the false positive disappears.
#[test]
fn diagnostics_suppress_argument_mismatch_for_oj_load_with_gem() {
    let source = concat!(
        "class Consumer\n",
        "  #: (Hash[String, untyped]) -> untyped\n",
        "  def consume(payload)\n",
        "    payload\n",
        "  end\n",
        "\n",
        "  def run(raw)\n",
        "    consume(Oj.load(raw))\n",
        "  end\n",
        "end\n",
    );

    // oj gem declared: the plugin resolves Oj.load -> untyped, so no false positive.
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::write(
        dir.path().join("Gemfile.lock"),
        "GEM\n  specs:\n    oj (3.16.1)\n\nDEPENDENCIES\n  oj\n",
    )
    .expect("write Gemfile.lock");
    let rb_file = dir.path().join("codec.rb");
    fs::write(&rb_file, source).expect("failed to write");
    let diags = argument_type_mismatches(&rb_file);
    assert!(
        diags.is_empty(),
        "Oj.load must not resolve to Kernel#load bool: {diags:?}"
    );

    // gem not declared: the plugin stays disabled (old behavior = bool
    // misinference causes the mismatch). This confirms the plugin's gem gate works.
    let dir_no_gem = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file_no_gem = dir_no_gem.path().join("codec.rb");
    fs::write(&rb_file_no_gem, source).expect("failed to write");
    let diags_no_gem = argument_type_mismatches(&rb_file_no_gem);
    assert_eq!(
        diags_no_gem.len(),
        1,
        "without the gem the old bool misinference remains: {diags_no_gem:?}"
    );
}

#[test]
fn diagnostics_flag_reports_inline_rbs_argument_type_mismatch() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("inline.rb");
    fs::write(
        &rb_file,
        "class A\n  #: (String) -> Integer\n  def foo(s)\n    s.length\n  end\nend\n\na = A.new\na.foo(1)\na.foo(\"ok\")\n",
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert_eq!(
        diags.len(),
        1,
        "only the Integer argument should be flagged: {diags:?}"
    );
    let d = &diags[0];
    assert_eq!(d["severity"], "error");
    assert_eq!(d["line"], 9);
    assert_eq!(d["expected_type"], "String");
    assert_eq!(d["param_name"], "s");
    assert_eq!(d["method_name"], "foo");
}

// An unannotated literal default (`= :invalid` next to a `T.untyped` sig
// param) leaks into the resolved param type as a literal singleton. A single
// literal is not a constraint — any argument must pass without a mismatch.
#[test]
fn diagnostics_flag_ignores_literal_default_as_param_constraint() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::create_dir_all(dir.path().join("sorbet")).expect("failed to create sorbet dir");
    fs::write(dir.path().join("sorbet").join("config"), ".\n").expect("failed to write config");
    let rb_file = dir.path().join("emit.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  sig { params(kind: T.untyped).returns(T.untyped) }\n",
            "  def emit(kind = :invalid)\n",
            "    kind\n",
            "  end\n",
            "end\n",
            "\n",
            "A.new.emit(:too_long)\n",
            "A.new.emit(\"custom message\")\n",
        ),
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert!(
        diags.is_empty(),
        "a literal default must not act as a declared constraint: {diags:?}"
    );
}

#[test]
fn diagnostics_flag_reports_sig_argument_type_mismatch() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::create_dir_all(dir.path().join("sorbet")).expect("failed to create sorbet dir");
    fs::write(dir.path().join("sorbet").join("config"), ".\n").expect("failed to write config");
    let rb_file = dir.path().join("sig.rb");
    fs::write(
        &rb_file,
        "class A\n  sig { params(s: String).returns(Integer) }\n  def foo(s)\n    s.length\n  end\nend\n\nA.new.foo(42)\n",
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert_eq!(
        diags.len(),
        1,
        "sig param mismatch should be flagged: {diags:?}"
    );
    assert_eq!(diags[0]["expected_type"], "String");
    assert_eq!(diags[0]["param_name"], "s");
}

#[test]
fn diagnostics_flag_reports_rbs_file_argument_type_mismatch() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::create_dir_all(dir.path().join("sig")).expect("failed to create sig dir");
    fs::write(
        dir.path().join("sig").join("greeter.rbs"),
        "class Greeter\n  def greet: (String) -> String\nend\n",
    )
    .expect("failed to write rbs");
    let rb_file = dir.path().join("app.rb");
    fs::write(&rb_file, "Greeter.new.greet(123)\n").expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert_eq!(
        diags.len(),
        1,
        "external .rbs param mismatch should be flagged: {diags:?}"
    );
    assert_eq!(diags[0]["expected_type"], "String");
}

// RBS's `path` (= `string | _ToPath`) is structural: any type with to_path / to_str
// conforms. Pathname / String are accepted, and only a surface-complete user class
// without to_path is flagged as a genuine new true positive.
#[test]
fn diagnostics_flag_structural_path_argument_conformance() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::create_dir_all(dir.path().join("sig")).expect("failed to create sig dir");
    fs::write(
        dir.path().join("sig").join("store.rbs"),
        "class PathStore\n  def store: (path) -> void\nend\n",
    )
    .expect("failed to write rbs");
    let rb_file = dir.path().join("app.rb");
    fs::write(
        &rb_file,
        concat!(
            "require \"pathname\"\n",
            "class Plain\n",
            "  def render; end\n",
            "end\n",
            "PathStore.new.store(Pathname.new(\"x\"))\n", // has to_path -> no flag
            "PathStore.new.store(\"str\")\n",             // String -> no flag
            "PathStore.new.store(Plain.new)\n",           // no to_path/to_str -> flag
        ),
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert_eq!(
        diags.len(),
        1,
        "only the to_path-less user class should be flagged: {diags:?}"
    );
    assert_eq!(diags[0]["line"], 7);
    assert_eq!(diags[0]["expected_type"], "path");
}

// overload: a call that can match either overload stays silent.
// An Integer argument against `(Integer) | (String)` matches the first, so no flag.
#[test]
fn diagnostics_overload_silent_when_one_overload_matches() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("overload_ok.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  #: (Integer) -> String\n",
            "  #: (String) -> String\n",
            "  def foo(x) = x.to_s\n",
            "end\n",
            "A.new.foo(1)\n",      // matches the Integer overload -> no flag
            "A.new.foo(\"ok\")\n", // matches the String overload -> no flag
        ),
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert!(
        diags.is_empty(),
        "an argument matching either overload must be silent: {diags:?}"
    );
}

// overload: only a call that fails to match every overload is reported.
// Passing a Symbol to `(Integer) | (String)` matches neither, so it's flagged.
#[test]
fn diagnostics_overload_reports_when_all_overloads_fail() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("overload_bad.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  #: (Integer) -> String\n",
            "  #: (String) -> String\n",
            "  def foo(x) = x.to_s\n",
            "end\n",
            "A.new.foo(:sym)\n", // matches neither overload -> flag
        ),
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert_eq!(
        diags.len(),
        1,
        "a symbol matching no overload must be reported: {diags:?}"
    );
    assert_eq!(diags[0]["method_name"], "foo");
    assert_eq!(diags[0]["param_name"], "x");
}

// overload: a call whose argument count matches no overload (arity mismatch only)
// does not trigger argument_type_mismatch (arity is the experimental check's domain).
#[test]
fn diagnostics_overload_silent_when_only_arity_mismatches() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("overload_arity.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  #: (Integer) -> String\n",
            "  #: (Integer, Integer) -> String\n",
            "  def foo(a, b = 0) = \"x\"\n",
            "end\n",
            "A.new.foo(1, 2, 3)\n", // 3 args matches no overload's arity
        ),
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert!(
        diags.is_empty(),
        "an arity-only mismatch must not be an argument_type_mismatch: {diags:?}"
    );
}

// overload: stays silent when the actual argument type is Unknown (e.g. untyped),
// since it can't be ruled out as a match (not definitely a No).
#[test]
fn diagnostics_overload_silent_when_actual_is_unknown() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("overload_unknown.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  #: (Integer) -> String\n",
            "  #: (String) -> String\n",
            "  def foo(x) = x.to_s\n",
            "end\n",
            "def run(v)\n",
            "  A.new.foo(v)\n", // v is untyped -> Unknown against every overload -> silent
            "end\n",
        ),
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert!(
        diags.is_empty(),
        "an unknown actual type must be silent under overloads: {diags:?}"
    );
}

// overload: regression check that behavior is unchanged for a single sig (no overload).
#[test]
fn diagnostics_single_sig_behavior_unchanged_alongside_overloads() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("single_sig.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  #: (String) -> Integer\n",
            "  def foo(s) = s.length\n",
            "end\n",
            "A.new.foo(1)\n", // single-sig mismatch -> flagged as before
        ),
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert_eq!(
        diags.len(),
        1,
        "single-sig mismatch must still be reported: {diags:?}"
    );
    assert_eq!(diags[0]["expected_type"], "String");
    assert_eq!(diags[0]["param_name"], "s");
}

// In `a, b = s.split('.')` on an untyped param `s`, the base (`s.split`'s return
// type) is unresolved, so slots beyond the first must not narrow to nil (it could be
// an array element, so untyped is correct). Narrowing to nil would produce a false
// argument_type_mismatch wherever `b` is passed into a typed param.
#[test]
fn diagnostics_does_not_flag_destructured_slot_from_unknown_base() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("destructure.rb");
    fs::write(
        &rb_file,
        concat!(
            "class Sink\n",
            "  #: (String) -> void\n",
            "  def take(s); end\n",
            "end\n",
            "def run(raw)\n",
            "  first, second = raw.split(\",\")\n",
            "  Sink.new.take(second)\n",
            "end\n",
        ),
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert!(
        diags.is_empty(),
        "destructured slot from an unknown base is untyped, not nil: {diags:?}"
    );
}

#[test]
fn diagnostics_flag_checks_array_optional_keyword_argument_shapes() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("shapes.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  #: (Array[Integer]) -> void\n",
            "  def arr(xs); end\n",
            "  #: (String?) -> void\n",
            "  def opt(s); end\n",
            "  #: (name: String) -> void\n",
            "  def kw(name:); end\n",
            "end\n",
            "a = A.new\n",
            "a.arr([1, 2])\n",     // ok
            "a.arr([\"x\"])\n",    // flag: element String vs Integer
            "a.opt(nil)\n",        // ok: optional
            "a.opt(\"x\")\n",      // ok
            "a.opt(1)\n",          // flag: Integer vs String?
            "a.kw(name: \"x\")\n", // ok
            "a.kw(name: 7)\n",     // flag: Integer vs String
        ),
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    let lines: Vec<i64> = diags
        .iter()
        .map(|d| d["line"].as_i64().expect("line"))
        .collect();
    assert_eq!(
        lines,
        vec![11, 14, 16],
        "array element, optional, and keyword mismatches should be flagged once each: {diags:?}"
    );
}

#[test]
fn diagnostics_flag_does_not_flag_correct_or_inferred_arguments() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("clean.rb");
    fs::write(
        &rb_file,
        concat!(
            // Inferred (unannotated) params must never be checked.
            "class Inferred\n",
            "  def foo(s); s; end\n",
            "end\n",
            "Inferred.new.foo(1)\n",
            "Inferred.new.foo(\"x\")\n",
            // Annotated method called with correct types and a subclass.
            "class Animal; end\n",
            "class Dog < Animal; end\n",
            "class Zoo\n",
            "  #: (Animal) -> void\n",
            "  def admit(a); end\n",
            "end\n",
            "Zoo.new.admit(Dog.new)\n",
        ),
    )
    .expect("failed to write");

    let diags = argument_type_mismatches(&rb_file);
    assert!(
        diags.is_empty(),
        "inferred params, correct types, and subclasses must not be flagged: {diags:?}"
    );
}

#[test]
fn diagnostics_flag_missing_method_output_omits_argument_fields() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("missing.rb");
    fs::write(&rb_file, "class A\nend\nA.new.nope\n").expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().expect("expected a diagnostic");
    let diag: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
    assert_eq!(diag["code"], "missing_method");
    // Argument-only fields must be skipped for non-argument diagnostics.
    assert!(diag.get("expected_type").is_none(), "got: {diag}");
    assert!(diag.get("param_name").is_none(), "got: {diag}");
}

/// A concern that exposes class methods via a nested `ClassMethods` module
/// (the `extend ActiveSupport::Concern` + `module ClassMethods` form) must
/// resolve on an including class — including across files and through an
/// absolute `include ::Concern` reference — so the class methods aren't
/// reported as missing.
#[test]
fn diagnostics_resolve_concern_class_methods_cross_file() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::write(dir.path().join("Gemfile"), "gem \"rails\"\n").expect("write Gemfile");
    let models = dir.path().join("app").join("models");
    let concerns = models.join("concerns");
    fs::create_dir_all(&concerns).expect("create dirs");
    fs::write(
        concerns.join("loggable.rb"),
        r#"module Loggable
  extend ActiveSupport::Concern

  module ClassMethods
    def create_log(kind)
      kind
    end
  end
end
"#,
    )
    .expect("write concern");
    fs::write(
        models.join("activity.rb"),
        r#"class Activity
  include ::Loggable

  def self.run
    create_log(1)
  end
end

class Caller
  def go
    Activity.create_log(2)
  end
end
"#,
    )
    .expect("write model");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(dir.path().to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("create_log"),
        "concern ClassMethods must resolve on the including class: {stdout}"
    );
}

/// `sig` (and other `sorbet-runtime` markers) must resolve only where `T::Sig`
/// is actually mixed in — per-class `extend T::Sig` or a global
/// `class Module; include T::Sig` — and otherwise be reported as undefined.
#[test]
fn diagnostics_resolve_sig_only_when_t_sig_mixed_in() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("sig_mix.rb");
    fs::write(
        &rb_file,
        concat!(
            "class WithExtend\n",
            "  extend T::Sig\n",
            "  sig { void }\n", // resolved via per-class extend
            "  def a = 1\n",
            "end\n",
            "class Module\n",
            "  include T::Sig\n",
            "end\n",
            "class ViaGlobal\n",
            "  sig { void }\n", // resolved via global Module include
            "  def b = 2\n",
            "end\n",
        ),
    )
    .expect("write");
    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("`sig`"),
        "sig must resolve where T::Sig is mixed in: {stdout}"
    );

    // No T::Sig anywhere → `sig` is genuinely undefined and must be flagged.
    // Use a separate workspace so the global `class Module; include T::Sig`
    // above is not picked up as a context definition.
    let bare_dir = tempfile::tempdir().expect("failed to create tempdir");
    let bare = bare_dir.path().join("sig_bare.rb");
    fs::write(&bare, "class Foo\n  sig { void }\n  def a = 1\nend\n").expect("write");
    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(bare.to_str().unwrap())
        .output()
        .expect("run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Method `sig` not found"),
        "sig must be flagged when T::Sig is not mixed in: {stdout}"
    );
}

/// A `sig { ... }` block body is type DSL, not runtime code: `returns` / `void` /
/// `params` inside it must never be diagnosed as calls on the enclosing class,
/// for both the global `class Module; include T::Sig` mixin and per-class
/// `extend T::Sig`. A genuine typo elsewhere in the file must still be flagged.
#[test]
fn diagnostics_do_not_flag_sig_block_body_as_code() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("sig_body.rb");
    fs::write(
        &rb_file,
        concat!(
            "class Module\n",
            "  include T::Sig\n",
            "end\n",
            "class Helper\n",
            "  def real_method\n",
            "    1\n",
            "  end\n",
            "end\n",
            "class SigUser\n",
            "  sig { void }\n",
            "  def a\n",
            "  end\n",
            "  sig { returns(T.nilable(Integer)) }\n",
            "  def b\n",
            "    nil\n",
            "  end\n",
            "  sig { params(name: String).returns(String) }\n",
            "  def c(name)\n",
            "    name\n",
            "  end\n",
            "  def typo_caller\n",
            "    Helper.new.nope_typo\n",
            "  end\n",
            "end\n",
            "class ExtendSigUser\n",
            "  extend T::Sig\n",
            "  sig { returns(Integer) }\n",
            "  def d\n",
            "    1\n",
            "  end\n",
            "end\n",
        ),
    )
    .expect("write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.contains("Method `returns` not found"),
        "sig block `returns` must not be diagnosed as a call: {stdout}"
    );
    assert!(
        !stdout.contains("Method `void` not found"),
        "sig block `void` must not be diagnosed as a call: {stdout}"
    );
    assert!(
        !stdout.contains("Method `params` not found"),
        "sig block `params` must not be diagnosed as a call: {stdout}"
    );
    assert!(
        stdout.contains("Method `nope_typo` not found for `Helper`"),
        "a genuine typo on a fully-known class must still be reported: {stdout}"
    );
}

// A class referenced before it is known is left in the workspace registry as
// an empty speculative stub. That stub must not shadow the authoritative
// definition the project's `sorbet/rbi/` carries — otherwise every tapioca
// model surfaces as an undefined constant. See `ClassData::has_type_substance`.
#[test]
fn diagnostics_loads_class_defined_only_in_sorbet_rbi() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rbi_dir = dir.path().join("sorbet").join("rbi").join("dsl");
    fs::create_dir_all(&rbi_dir).expect("failed to create rbi dir");
    fs::write(
        rbi_dir.join("foo_direct.rbi"),
        "# typed: true\nclass FooDirect\n  def hello; end\nend\n",
    )
    .expect("failed to write rbi");
    let rb_file = dir.path().join("main.rb");
    fs::write(&rb_file, "FooDirect.new.hello\n").expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "constant and method defined in sorbet/rbi must resolve, got: {stdout}"
    );
}

// Tapioca emits `include GeneratedAttributeMethods` *before* the nested module
// it defines, so the mixin edge stores the bare name while the methods live at
// `Owner::GeneratedAttributeMethods`. Loading the owner from `sorbet/rbi/` must
// pull that nested module in, or every generated column/dirty-tracking method
// is a false positive.
#[test]
fn diagnostics_resolves_tapioca_nested_generated_methods() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rbi_dir = dir
        .path()
        .join("sorbet")
        .join("rbi")
        .join("dsl")
        .join("account");
    fs::create_dir_all(&rbi_dir).expect("failed to create rbi dir");
    fs::write(
        rbi_dir.join("shop_user.rbi"),
        "# typed: true\n\
         class Account::ShopUser\n\
        \x20 include GeneratedAttributeMethods\n\n\
        \x20 module GeneratedAttributeMethods\n\
        \x20   def person_id; end\n\
        \x20   def person_id_changed?; end\n\
        \x20 end\n\
         end\n",
    )
    .expect("failed to write rbi");
    let rb_file = dir.path().join("main.rb");
    fs::write(
        &rb_file,
        "u = Account::ShopUser.new\nu.person_id\nu.person_id_changed?\nu.no_such_method\n",
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("person_id"),
        "person_id / person_id_changed? from the nested module must resolve, got: {stdout}"
    );
    assert!(
        stdout.contains("Method `no_such_method` not found"),
        "a genuinely undefined method must still be flagged, got: {stdout}"
    );
}

// Passing a subset of a project's `.rb` files to `--diagnostics` must still
// resolve definitions living in the other project files. The other files are
// scanned as definitions-only *context* (skeleton merged, no diagnostics
// emitted). The four cases below reduce the FP patterns observed in a real
// Sorbet-clean app to minimal names.

fn diagnostics_for_target(target: &std::path::Path) -> String {
    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(target.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).into_owned()
}

// A cwd-relative target inside a project subdirectory must still discover the
// workspace: the root climb from a relative parent reaches the empty path,
// which must normalize to `.` instead of yielding zero context files.
#[test]
fn diagnostics_resolve_cross_file_with_relative_target_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("Gemfile"), "source 'https://example.org'\n").expect("write gemfile");
    let app = dir.path().join("app");
    fs::create_dir_all(&app).expect("mkdir app");
    fs::write(app.join("a.rb"), "class A\n  def call = B.new.hello\nend\n").expect("write a");
    fs::write(app.join("b.rb"), "class B\n  def hello = 1\nend\n").expect("write b");

    let output = tyda_bin()
        .current_dir(dir.path())
        .arg("--diagnostics")
        .arg("app/a.rb")
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "relative target must discover workspace context from cwd: {stdout}"
    );
}

// A constant and instance method defined in another, unpassed file must resolve
// from context so the target file shows no unresolved_constant / missing_method.
#[test]
fn diagnostics_resolve_cross_file_constant_reference() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a.rb");
    fs::write(&a, "class A\n  def call = B.new.hello\nend\n").expect("write a");
    fs::write(dir.path().join("b.rb"), "class B\n  def hello = 1\nend\n").expect("write b");

    let stdout = diagnostics_for_target(&a);
    assert!(
        stdout.trim().is_empty(),
        "cross-file constant B and method hello must resolve from context: {stdout}"
    );
}

// A parent class (and its inherited method) defined in another, unpassed file
// must resolve so the target's subclass reference shows no FP.
#[test]
fn diagnostics_resolve_cross_file_parent_class_method() {
    let dir = tempfile::tempdir().expect("tempdir");
    let g = dir.path().join("g.rb");
    fs::write(
        &g,
        "class G\n  def run\n    obj = C.new\n    obj.shared_method\n  end\nend\n",
    )
    .expect("write g");
    fs::write(dir.path().join("c.rb"), "class C < Base\nend\n").expect("write c");
    fs::write(
        dir.path().join("base.rb"),
        "class Base\n  def shared_method = 1\nend\n",
    )
    .expect("write base");

    let stdout = diagnostics_for_target(&g);
    assert!(
        stdout.trim().is_empty(),
        "subclass C, parent Base, and inherited shared_method must resolve from context: {stdout}"
    );
}

// `attr_accessor` with multiple symbols in another, unpassed file must generate
// the readers so callers in the target file resolve them.
#[test]
fn diagnostics_resolve_cross_file_attr_accessor_symbols() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path().join("d.rb");
    fs::write(
        &d,
        "class D\n  def run\n    h = Holder.new\n    h.foo\n    h.bar\n  end\nend\n",
    )
    .expect("write d");
    fs::write(
        dir.path().join("holder.rb"),
        "class Holder\n  attr_accessor :foo, :bar\nend\n",
    )
    .expect("write holder");

    let stdout = diagnostics_for_target(&d);
    assert!(
        stdout.trim().is_empty(),
        "constant Holder and both attr_accessor readers must resolve from context: {stdout}"
    );
}

// A Rails `scope` (a DSL plugin expansion) in another, unpassed model must
// generate the singleton scope method so the target's call resolves.
#[test]
fn diagnostics_resolve_cross_file_rails_scope() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("Gemfile"), "gem \"rails\"\n").expect("write Gemfile");
    let models = dir.path().join("app").join("models");
    fs::create_dir_all(&models).expect("models dir");
    fs::write(
        models.join("application_record.rb"),
        "class ApplicationRecord < ActiveRecord::Base\nend\n",
    )
    .expect("write application_record");
    fs::write(
        models.join("f.rb"),
        "class F < ApplicationRecord\n  scope :active, -> { where(active: true) }\nend\n",
    )
    .expect("write f");
    let e = models.join("e.rb");
    fs::write(&e, "class E\n  def run = F.active\nend\n").expect("write e");

    let stdout = diagnostics_for_target(&e);
    assert!(
        stdout.trim().is_empty(),
        "constant F and its cross-file scope :active must resolve from context: {stdout}"
    );
}

// Context files are scanned for definitions only and never contribute
// diagnostics: an error inside an unpassed file must not appear in the output
// for the passed target.
#[test]
fn diagnostics_do_not_leak_from_context_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a.rb");
    fs::write(&a, "class A\n  def call = B.new.hello\nend\n").expect("write a");
    fs::write(
        dir.path().join("b.rb"),
        "class B\n  def hello = 1\n  def oops = totally_missing_helper\nend\n",
    )
    .expect("write b");

    let stdout = diagnostics_for_target(&a);
    assert!(
        !stdout.contains("totally_missing_helper"),
        "a context file's own missing method must not be reported: {stdout}"
    );
    assert!(
        !stdout.contains("b.rb"),
        "no diagnostic should reference the context file b.rb: {stdout}"
    );
}

/// Pins down the fix for a quality gap on the deferred (CLI batch) path: param-receiver
/// calls whose caller lives in a different file reach the same concrete type as on the
/// full path (4 cases: scalar / generic element / block / record key).
#[test]
fn analyze_batch_resolves_param_receiver_calls_across_files() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::write(
        dir.path().join("sample.rb"),
        concat!(
            "class Sample\n",
            "  def up(s) = s.upcase\n",
            "  def first_of(arr) = arr.first\n",
            "  def mapped(arr) = arr.map { |x| x }\n",
            "  def keyed(h) = h[:key]\n",
            "end\n",
        ),
    )
    .expect("failed to write sample.rb");
    fs::write(
        dir.path().join("caller.rb"),
        concat!(
            "class Caller\n",
            "  def use\n",
            "    sample = Sample.new\n",
            "    sample.up(\"a\")\n",
            "    sample.first_of([1, 2])\n",
            "    sample.mapped([1, 2])\n",
            "    sample.keyed({ key: 1 })\n",
            "  end\n",
            "end\n",
        ),
    )
    .expect("failed to write caller.rb");

    let output = tyda_bin()
        .arg(dir.path().to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "def up: (String s) -> String",
        "def first_of: (Array[Integer] arr) -> Integer?",
        "def mapped: (Array[Integer] arr) -> Array[Integer]",
        "def keyed: ({ key: Integer } h) -> Integer",
    ] {
        assert!(
            stdout.contains(expected),
            "expected `{expected}` in batch output, got: {stdout}"
        );
    }
}

fn diagnostic_method_names(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "missing_method")
        .map(|d| d["method_name"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// Fix 1: a bare call inside a method of `M::ClassMethods` is not treated as missing
/// if it can be resolved on the singleton face of a class that includes M. Here
/// `configured_value`, called from a method of `Ext::ClassMethods`, resolves via
/// includer `C`'s singleton (coming from `Provider::ClassMethods`).
#[test]
fn diagnostics_resolves_class_methods_bare_call_via_includer_singleton() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("class_methods_lift.rb");
    fs::write(
        &rb_file,
        r#"module Ext
  module ClassMethods
    def build = configured_value
  end
end

module Provider
  module ClassMethods
    def configured_value = 42
  end
end

class C
  include Ext
  include Provider
end
"#,
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !diagnostic_method_names(&stdout).contains(&"configured_value".to_string()),
        "class-method bare call resolvable via includer singleton must not be reported: {stdout}"
    );
}

/// Fix 4: a bare call in a module with no static mixin edge (include / prepend /
/// extend) is suppressed since the runtime host is unknown. When there are includers
/// but the call resolves on none of them, it's still reported as before (checks both
/// directions).
#[test]
fn diagnostics_suppresses_bare_call_in_module_without_static_mixin_edge() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");

    // Direction A: zero includers -> suppressed.
    let lonely = dir.path().join("lonely.rb");
    fs::write(
        &lonely,
        "module Lonely\n  def work\n    totally_absent_helper\n  end\nend\n",
    )
    .expect("failed to write");
    let out_a = tyda_bin()
        .arg("--diagnostics")
        .arg(lonely.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(out_a.status.success());
    let stdout_a = String::from_utf8_lossy(&out_a.stdout);
    assert!(
        !diagnostic_method_names(&stdout_a).contains(&"totally_absent_helper".to_string()),
        "bare call in a module with no static mixin edge must be suppressed: {stdout_a}"
    );

    // Direction B: includers exist but the call resolves on none of them -> reported.
    let shared = dir.path().join("shared.rb");
    fs::write(
        &shared,
        "module Shared\n  def work\n    totally_absent_helper\n  end\nend\n\nclass Host\n  include Shared\nend\n",
    )
    .expect("failed to write");
    let out_b = tyda_bin()
        .arg("--diagnostics")
        .arg(shared.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(out_b.status.success());
    let stdout_b = String::from_utf8_lossy(&out_b.stdout);
    assert!(
        diagnostic_method_names(&stdout_b).contains(&"totally_absent_helper".to_string()),
        "bare call unresolved on all includers must still be reported: {stdout_b}"
    );
}

/// Fix 2: does not raise argument_type_mismatch when the actual is an unresolved type
/// variable (a phantom nominalized from Sorbet's unbound `T.type_parameter(:U)`).
/// Confirms, as a control, that a concrete mismatch (Integer -> String) on the same
/// method is still reported.
#[test]
fn diagnostics_silences_argument_mismatch_for_unresolved_type_variable() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("type_var_arg.rb");
    fs::write(
        &rb_file,
        r#"# typed: true
class Gen
  extend T::Sig
  sig { type_parameters(:U).params(blk: T.proc.returns(T.type_parameter(:U))).returns(T.type_parameter(:U)) }
  def self.run(&blk)
    yield
  end
end

class Consumer
  extend T::Sig
  sig { params(s: String).void }
  def self.take(s); end

  def self.go
    v = Gen.run { Object.new }
    take(v)
    take(123)
  end
end
"#,
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mismatches: Vec<serde_json::Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "argument_type_mismatch")
        .collect();
    // The type-variable actual stays silent; only the concrete mismatch (123) is reported.
    assert_eq!(
        mismatches.len(),
        1,
        "only the concrete Integer mismatch should be reported: {stdout}"
    );
    assert_eq!(mismatches[0]["actual_type"], "123");
}

// A bare `Class` / `Module` receiver's actual class is unknown, so a call to a
// method not on its instance surface is undecidable (metaprogramming can delegate to
// the real class), and missing_method is suppressed. A genuine Class instance method
// (e.g. `.name`) resolves and is never reported.
#[test]
fn diagnostics_flag_suppresses_missing_method_on_bare_class_or_module_receiver() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("class_recv.rb");
    fs::write(
        &rb_file,
        concat!(
            "class A\n",
            "  #: (Class klass) -> void\n",
            "  def f(klass)\n",
            "    klass.reflect_on_association(:foo)\n", // not on the Class surface -> suppressed
            "  end\n",
            "  #: (Module m) -> void\n",
            "  def g(m)\n",
            "    m.totally_made_up_method\n", // not on the Module surface -> suppressed
            "  end\n",
            "  #: (Class k) -> void\n",
            "  def h(k)\n",
            "    k.definitly_a_typo\n", // not on the Class surface -> suppressed
            "  end\n",
            "end\n",
        ),
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let messages: Vec<String> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "missing_method")
        .filter_map(|d| d["message"].as_str().map(str::to_string))
        .collect();
    assert!(
        messages.is_empty(),
        "missing methods on bare Class / Module receivers must be suppressed: {messages:?}"
    );
}

// A qualified constant path's head goes through the same 3-phase resolution as a
// bare constant (lexical -> ancestor -> top-level). Confirms that a qualified path
// whose head names a nested namespace in an included module resolves, and doesn't
// trigger a false `unresolved_constant`.
#[test]
fn diagnostics_flag_resolves_qualified_path_head_through_include() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("const_head.rb");
    fs::write(
        &rb_file,
        concat!(
            "module Formats\n",
            "  module MyBridge\n",
            "    HEADERS = [1, 2]\n",
            "  end\n",
            "end\n",
            "class Reader\n",
            "  include Formats\n",
            "  def headers = MyBridge::HEADERS\n", // head resolves via include -> no uc
            "end\n",
        ),
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let unresolved: Vec<String> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "unresolved_constant")
        .filter_map(|d| d["message"].as_str().map(str::to_string))
        .collect();
    assert!(
        unresolved.is_empty(),
        "qualified path head resolvable through an include must not be flagged: {unresolved:?}"
    );
}

/// Draper `delegate_all` decorator: an undefined bare call inside a decorator
/// method is delegated to the decorated model via `method_missing` at runtime,
/// so it must not be reported as a missing method — even when the decorated
/// model lives in another file (the eager method-copy cannot see it yet).
/// A decorator WITHOUT `delegate_all` still reports its own unresolved calls.
#[test]
fn diagnostics_flag_suppresses_draper_delegate_all_bare_calls() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    fs::write(dir.path().join("Gemfile"), "gem \"draper\"\n").expect("failed to write Gemfile");
    let models = dir.path().join("app").join("models");
    let decorators = dir.path().join("app").join("decorators");
    fs::create_dir_all(&models).expect("mkdir models");
    fs::create_dir_all(&decorators).expect("mkdir decorators");
    // A substantive `Draper::Decorator` base so the decorator's ancestor chain
    // is fully known (mirrors a project shipping the gem's RBI). Without a known
    // base the surface is "unknown" and every decorator is suppressed anyway, so
    // this is what makes the delegate_all vs plain distinction observable.
    fs::write(
        dir.path().join("draper.rb"),
        "module Draper\n  class Decorator\n  end\nend\n",
    )
    .expect("write draper base");
    fs::write(
        models.join("widget.rb"),
        "class Widget\n  def account_creatable? = true\nend\n",
    )
    .expect("write model");
    fs::write(
        decorators.join("widget_decorator.rb"),
        r#"class WidgetDecorator < Draper::Decorator
  delegate_all
  def account_creatable
    account_creatable? ? 1 : 0
  end
end
"#,
    )
    .expect("write delegate_all decorator");
    fs::write(
        decorators.join("plain_decorator.rb"),
        r#"class PlainDecorator < Draper::Decorator
  def broken
    definitely_missing
  end
end
"#,
    )
    .expect("write plain decorator");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(dir.path().join("draper.rb").to_str().unwrap())
        .arg(models.join("widget.rb").to_str().unwrap())
        .arg(decorators.join("widget_decorator.rb").to_str().unwrap())
        .arg(decorators.join("plain_decorator.rb").to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let missing: Vec<String> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "missing_method")
        .filter_map(|d| d["unresolved_method"].as_str().map(str::to_string))
        .collect();
    assert!(
        !missing.iter().any(|m| m.starts_with("WidgetDecorator#")),
        "delegate_all decorator bare calls must be suppressed: {missing:?}"
    );
    assert!(
        missing
            .iter()
            .any(|m| m == "PlainDecorator#definitely_missing"),
        "a decorator without delegate_all still reports unresolved calls: {missing:?}"
    );
}

/// `x.instance_eval { m }` switches `self` to the receiver. When the receiver
/// type is unknown (an untyped param), the bare call's owner cannot be named,
/// so no missing-method verdict is possible — the call must be suppressed
/// instead of being reported against the enclosing class.
#[test]
fn diagnostics_flag_suppresses_instance_eval_bare_calls_on_unknown_receiver() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("report.rb");
    fs::write(
        &rb_file,
        r#"class Report
  def useless?(profile)
    profile.instance_eval do
      blank? || full_name.blank?
    end
  end
end
"#,
    )
    .expect("failed to write");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(rb_file.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let missing: Vec<String> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "missing_method")
        .filter_map(|d| d["unresolved_method"].as_str().map(str::to_string))
        .collect();
    assert!(
        missing.is_empty(),
        "instance_eval bare calls on an unknown receiver must not be reported as missing on the enclosing class: {missing:?}"
    );
}

// Resolves a nested module constant in an included module (`Formats::MyBridge::HEADERS`)
// even when the defining file and the referencing file differ. On the per-file
// diagnostics replay path, the other file's definition lives in the external registry,
// so this confirms the ancestor-chain walk can traverse both registries read-only
// (regression guard for cross-file deferral).
#[test]
fn diagnostics_flag_resolves_nested_module_constant_across_files() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let formats = dir.path().join("formats.rb");
    let reader = dir.path().join("reader.rb");
    fs::write(
        &formats,
        concat!(
            "module Outer\n",
            "  module Formats\n",
            "    module MyBridge\n",
            "      HEADERS = ['a', 'b'].freeze\n",
            "    end\n",
            "  end\n",
            "end\n",
        ),
    )
    .expect("failed to write formats");
    fs::write(
        &reader,
        concat!(
            "module Outer\n",
            "  class Reader\n",
            "    include Formats\n",
            "    def count\n",
            "      MyBridge::HEADERS.size\n",
            "    end\n",
            "  end\n",
            "end\n",
        ),
    )
    .expect("failed to write reader");

    // --diagnostics: the referencing file must not report unresolved_constant.
    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(formats.to_str().unwrap())
        .arg(reader.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let unresolved: Vec<String> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "unresolved_constant")
        .filter_map(|d| d["message"].as_str().map(str::to_string))
        .collect();
    assert!(
        unresolved.is_empty(),
        "cross-file nested-module constant must resolve, no unresolved_constant: {unresolved:?}"
    );

    // --verbose: re-infers the referencing side with external context, and
    // `MyBridge::HEADERS.size` resolves to Integer (tuple size = literal 2).
    let verbose = tyda_bin()
        .arg("--verbose")
        .arg(formats.to_str().unwrap())
        .arg(reader.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(verbose.status.success());
    let rbs = String::from_utf8_lossy(&verbose.stdout);
    assert!(
        rbs.contains("def count: -> 2"),
        "cross-file constant type must resolve (HEADERS.size -> Integer literal 2): {rbs}"
    );
}

// Using a declared constant whose value type falls to `untyped` (an alias like
// `E = ::Foo::BAR` to an unresolved constant in another file) as a method-call
// receiver does not trigger a false `unresolved_constant`. Being declared makes it
// Unknown rather than "undefined"; a genuinely undefined constant receiver
// (`Bogus.method`) is still reported as before.
#[test]
fn diagnostics_flag_does_not_report_declared_but_untyped_constant_receiver() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    // Declare a constant whose value type becomes untyped via an unresolved cross-file alias.
    let alias = dir.path().join("alias.rb");
    fs::write(
        &alias,
        concat!(
            "class Codes\n",
            "  SIZE_LIST = ::Widgets::Widget::SIZES\n",
            "end\n",
        ),
    )
    .expect("failed to write alias");
    // Use both a declared-untyped constant and a genuinely undefined constant as receivers.
    let user = dir.path().join("user.rb");
    fs::write(
        &user,
        concat!(
            "class Reader\n",
            "  def known(x) = Codes::SIZE_LIST.include?(x)\n",
            "  def unknown(x) = Missing::Absent.include?(x)\n",
            "end\n",
        ),
    )
    .expect("failed to write user");

    let output = tyda_bin()
        .arg("--diagnostics")
        .arg(alias.to_str().unwrap())
        .arg(user.to_str().unwrap())
        .output()
        .expect("failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let unresolved: Vec<String> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "unresolved_constant")
        .filter_map(|d| d["message"].as_str().map(str::to_string))
        .collect();
    // `Codes::SIZE_LIST` is declared -> not reported. `Missing::Absent` is undefined -> reported.
    assert!(
        !unresolved.iter().any(|m| m.contains("SIZE_LIST")),
        "declared-but-untyped constant receiver must not be flagged: {unresolved:?}"
    );
    assert!(
        unresolved.iter().any(|m| m.contains("Missing::Absent")),
        "genuinely undefined constant receiver must still be flagged: {unresolved:?}"
    );
}

// Resolves a nested class under a stdlib module (`OpenSSL`) as a receiver even without
// referencing the module itself directly. `OpenSSL::X509::Store` and friends are
// declared nested inside `module OpenSSL` in openssl.rbs, so they don't show up in the
// stem index, but the head of the unresolved qualified constant (`OpenSSL`) matches a
// stdlib top-level declaration, so the head gets loaded and pulls in the nested
// declarations. Also pins down that a sig return-type reference creating an empty
// stub doesn't block that load.
#[test]
fn diagnostics_flag_resolves_nested_stdlib_constant_via_head_load() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let rb_file = dir.path().join("verifier.rb");
    fs::write(
        &rb_file,
        concat!(
            "# typed: true\n",
            "module AppStoreServer\n",
            "  class Verifier\n",
            "    sig { returns(OpenSSL::X509::Store) }\n",
            "    def store = OpenSSL::X509::Store.new\n",
            "    def cert = OpenSSL::X509::Certificate.new\n",
            "    def pkcs7 = OpenSSL::PKCS7.new\n",
            "  end\n",
            "end\n",
        ),
    )
    .expect("failed to write");

    let stdout = diagnostics_for_target(&rb_file);
    let unresolved: Vec<String> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|d| d["code"] == "unresolved_constant")
        .filter_map(|d| d["message"].as_str().map(str::to_string))
        .collect();
    assert!(
        !unresolved.iter().any(|m| m.contains("OpenSSL")),
        "nested OpenSSL constants must resolve via head lazy-load: {unresolved:?}"
    );
}

#[test]
fn diagnostics_output_is_deterministic() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");

    // Subdirectories and files are created in non-alphabetical order so that
    // raw `fs::read_dir` order (filesystem-dependent) would disagree with the
    // lexicographic traversal order the CLI must produce.
    let sub_dirs = ["zeta", "alpha", "mu"];
    let file_names = ["z_file", "q_file", "a_file", "m_file", "b_file"];
    // The one file that emits two diagnostics whose category-grouped collection
    // order (missing_method, then unresolved_constant) would invert their line
    // order; sorted-by-position output must put them back in line order.
    let special_sub = "mu";
    let special_file = "b_file";

    let mut widget_idx = 0usize;
    let mut total_files = 0usize;
    for sub in sub_dirs {
        let sub_dir = dir.path().join(sub);
        fs::create_dir(&sub_dir).expect("failed to create subdir");
        for fname in file_names {
            let content = if sub == special_sub && fname == special_file {
                concat!(
                    "class OrderSpecial\n",
                    "  def early = Foo.new.something\n\n",
                    "  def identity = object_id\n",
                    "end\n\n",
                    "OrderSpecial.new.missing_method_special\n",
                )
                .to_string()
            } else {
                widget_idx += 1;
                format!(
                    "class Widget{widget_idx}\n  def identity = object_id\nend\n\nWidget{widget_idx}.new.missing_method_{widget_idx}\n"
                )
            };
            fs::write(sub_dir.join(format!("{fname}.rb")), content).expect("failed to write");
            total_files += 1;
        }
    }
    // Root-level files too, also created out of alphabetical order.
    for fname in ["y_root", "b_root"] {
        widget_idx += 1;
        let content = format!(
            "class Widget{widget_idx}\n  def identity = object_id\nend\n\nWidget{widget_idx}.new.missing_method_{widget_idx}\n"
        );
        fs::write(dir.path().join(format!("{fname}.rb")), content).expect("failed to write");
        total_files += 1;
    }

    let run = || {
        let output = tyda_bin()
            .arg("--diagnostics")
            .arg(dir.path().to_str().unwrap())
            .output()
            .expect("failed to run");
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).into_owned()
    };

    let first = run();
    let second = run();

    assert!(!first.is_empty(), "expected non-empty diagnostics output");
    assert_eq!(
        first, second,
        "diagnostics JSONL output must be byte-identical across runs"
    );

    let diagnostics: Vec<serde_json::Value> = first
        .lines()
        .map(|line| serde_json::from_str(line).expect("diagnostic should be JSON"))
        .collect();
    // One diagnostic per plain file, plus two (not one) from the special file.
    assert_eq!(
        diagnostics.len(),
        total_files + 1,
        "expected one diagnostic per plain file plus two from the special file: {diagnostics:?}"
    );

    let paths: Vec<&str> = diagnostics
        .iter()
        .map(|d| d["path"].as_str().expect("path field"))
        .collect();
    assert!(
        paths.windows(2).all(|w| w[0] <= w[1]),
        "file paths in JSONL must appear in lexicographic traversal order: {paths:?}"
    );

    // Within the special file, diagnostics must be in position (line) order,
    // not category-grouped order (missing_method would otherwise precede
    // unresolved_constant despite occurring on a later line).
    // Path::ends_with compares whole components, so the check is separator-agnostic (Windows emits `\`).
    let special_path_suffix = std::path::Path::new(special_sub).join(format!("{special_file}.rb"));
    let special_diags: Vec<&serde_json::Value> = diagnostics
        .iter()
        .filter(|d| {
            d["path"]
                .as_str()
                .is_some_and(|p| std::path::Path::new(p).ends_with(&special_path_suffix))
        })
        .collect();
    assert_eq!(
        special_diags.len(),
        2,
        "special file should emit exactly two diagnostics: {diagnostics:?}"
    );
    assert_eq!(special_diags[0]["code"], "unresolved_constant");
    assert_eq!(special_diags[1]["code"], "missing_method");
    assert!(
        special_diags[0]["line"].as_u64().unwrap() < special_diags[1]["line"].as_u64().unwrap(),
        "special file diagnostics must be in position order: {special_diags:?}"
    );
}
