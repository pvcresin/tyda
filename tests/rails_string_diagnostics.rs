#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "integration test assertions"
)]

use std::collections::BTreeSet;
use std::process::Command;

use tempfile::tempdir;
use tyda::analysis::{AnalysisOptions, hover_at_with_analysis_options};
use tyda::project::{DslActivation, ProjectVersions};
use tyda::rbs::stdlib_loader::LazyRbsLoader;

const SOURCE: &str = concat!(
    "class Probe\n",
    "  def normalize(value)\n",
    "    case value\n",
    "    when String\n",
    "      value.underscore.camelize(:lower).not_a_rails_string_chain_method\n",
    "    else\n",
    "      \"other\".not_a_rails_string_method\n",
    "    end\n",
    "  end\n",
    "end\n",
);

const HOVER_SOURCE: &str = concat!(
    "class Probe\n",
    "  #: (untyped) -> String\n",
    "  def normalize(value)\n",
    "    case value\n",
    "    when String\n",
    "      value.underscore\n",
    "    else\n",
    "      \"other\"\n",
    "    end\n",
    "  end\n",
    "end\n",
);

fn cli_missing_methods(gemfile: &str, source: &str, rbs: Option<&str>) -> BTreeSet<String> {
    let project = tempdir().expect("temporary project");
    std::fs::write(project.path().join("Gemfile"), gemfile).expect("write Gemfile");
    let app = project.path().join("app");
    std::fs::create_dir_all(&app).expect("create app directory");
    std::fs::write(app.join("probe.rb"), source).expect("write Ruby source");
    if let Some(rbs) = rbs {
        let sig = project.path().join("sig");
        std::fs::create_dir_all(&sig).expect("create sig directory");
        std::fs::write(sig.join("active_support.rbs"), rbs).expect("write RBS");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_tyda"))
        .args(["--diagnostics", "app/probe.rb"])
        .current_dir(project.path())
        .output()
        .expect("run Tyda diagnostics");
    assert!(
        output.status.success(),
        "Tyda failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|diagnostic| diagnostic["code"] == "missing_method")
        .filter_map(|diagnostic| diagnostic["unresolved_method"].as_str().map(str::to_owned))
        .collect()
}

fn rails_gemfile() -> &'static str {
    "source 'https://rubygems.org'\ngem 'rails', '~> 8.1.0'\n"
}

#[test]
fn active_support_string_extensions_are_not_reported_missing() {
    let missing = cli_missing_methods(rails_gemfile(), SOURCE, None);
    assert!(!missing.contains("String#underscore"), "{missing:?}");
    assert!(!missing.contains("String#camelize"), "{missing:?}");
    assert!(
        missing.contains("String#not_a_rails_string_method"),
        "{missing:?}"
    );
    assert!(
        missing.contains("String#not_a_rails_string_chain_method"),
        "{missing:?}"
    );

    let lazy_loader = LazyRbsLoader::new(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core"),
    );
    let hover = hover_at_with_analysis_options(
        HOVER_SOURCE,
        None,
        &lazy_loader,
        "app/probe.rb",
        6,
        15,
        AnalysisOptions {
            rails_mode: true,
            dsl_activation: DslActivation::with_rails_mode(true),
            project_versions: ProjectVersions::default(),
            project_root: None,
        },
    )
    .expect("hover on the Active Support call");
    assert_eq!(hover.name, "underscore");
    assert_eq!(hover.unresolved_method, None);
    assert_eq!(hover.ty.to_string(), "String");
}

#[test]
fn active_support_string_fallback_does_not_hide_source_or_rbs_overrides() {
    let source_override = concat!(
        "class String\n",
        "  def underscore = 42\n",
        "end\n",
        "class Probe\n",
        "  def normalize(value)\n",
        "    case value\n",
        "    when String\n",
        "      value.underscore.camelize(:lower)\n",
        "    end\n",
        "  end\n",
        "end\n",
    );
    let source_missing = cli_missing_methods(rails_gemfile(), source_override, None);
    assert!(
        !source_missing.contains("String#underscore"),
        "{source_missing:?}"
    );
    assert!(
        source_missing.contains("Integer#camelize"),
        "{source_missing:?}"
    );

    let rbs_override = "class String\n  def underscore: -> Integer\nend\n";
    let rbs_missing = cli_missing_methods(rails_gemfile(), SOURCE, Some(rbs_override));
    assert!(
        !rbs_missing.contains("String#underscore"),
        "{rbs_missing:?}"
    );
    assert!(rbs_missing.contains("Integer#camelize"), "{rbs_missing:?}");
}

#[test]
fn active_support_string_fallback_is_scoped_to_rails() {
    let gemfile = "source 'https://rubygems.org'\ngem 'minitest'\n";
    let missing = cli_missing_methods(gemfile, SOURCE, None);
    assert!(missing.contains("String#underscore"), "{missing:?}");
    assert!(
        missing.contains("String#not_a_rails_string_method"),
        "{missing:?}"
    );
}

#[test]
fn active_support_string_fallback_handles_only_the_string_union_member() {
    let source = concat!(
        "class Provider\n",
        "end\n",
        "class Probe\n",
        "  def normalize(use_provider)\n",
        "    value = use_provider ? Provider.new : \"text\"\n",
        "    value.underscore\n",
        "  end\n",
        "  def integer_value\n",
        "    1.underscore\n",
        "  end\n",
        "end\n",
    );
    let missing = cli_missing_methods(rails_gemfile(), source, None);
    assert!(!missing.contains("String#underscore"), "{missing:?}");
    assert!(missing.contains("Provider#underscore"), "{missing:?}");
    assert!(missing.contains("Integer#underscore"), "{missing:?}");
}

#[test]
fn active_support_string_fallback_does_not_hide_singleton_union_members() {
    let singleton_source = concat!(
        "class Probe\n",
        "  def normalize\n",
        "    String.underscore\n",
        "  end\n",
        "end\n",
    );
    let singleton_missing = cli_missing_methods(rails_gemfile(), singleton_source, None);
    assert!(
        singleton_missing.contains("String#underscore"),
        "singleton receiver alone must remain missing: {singleton_missing:?}"
    );

    let source = concat!(
        "class Probe\n",
        "  def normalize(use_class)\n",
        "    value = use_class ? String : \"text\"\n",
        "    value.underscore\n",
        "  end\n",
        "end\n",
    );
    let missing = cli_missing_methods(rails_gemfile(), source, None);
    assert!(
        missing.contains("String#underscore"),
        "singleton(String) union member must remain missing: {missing:?}"
    );
}
