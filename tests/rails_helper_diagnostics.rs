#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "integration test assertions"
)]

use std::collections::BTreeSet;
use std::process::Command;

use tempfile::tempdir;

fn cli_missing_methods(source: &str) -> BTreeSet<String> {
    let project = tempdir().expect("temporary project");
    std::fs::write(
        project.path().join("Gemfile"),
        "source 'https://rubygems.org'\ngem 'rails', '~> 8.1.0'\n",
    )
    .expect("write Gemfile");
    let helpers = project.path().join("app/helpers");
    std::fs::create_dir_all(&helpers).expect("create helpers directory");
    std::fs::write(helpers.join("nav_helper.rb"), source).expect("write Ruby source");

    let output = Command::new(env!("CARGO_BIN_EXE_tyda"))
        .args(["--diagnostics", "app/helpers/nav_helper.rb"])
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

// GitLab defines `can?` in the app, so a helper without CanCanCan must not report it.
#[test]
fn authorization_predicates_in_helpers_are_not_reported_missing_without_cancancan() {
    let missing = cli_missing_methods(concat!(
        "module NavHelper\n",
        "  extend self\n",
        "  def show_nav? = can?(:read, :nav)\n",
        "  def hide_nav? = cannot?(:read, :nav)\n",
        "end\n",
    ));
    assert!(!missing.contains("NavHelper#can?"), "{missing:?}");
    assert!(!missing.contains("NavHelper#cannot?"), "{missing:?}");
}
