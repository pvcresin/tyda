use std::collections::BTreeSet;
use std::path::PathBuf;

use tempfile::tempdir;
use tyda::analysis::{AnalysisOptions, analyze_source_for_display, cli_diagnostics_from_snapshot};
use tyda::project::{DslActivation, DslLibrary, ProjectVersions, detect_dsl_libraries_from_gems};
use tyda::rbs::stdlib_loader::LazyRbsLoader;

const SOURCE: &str = r#"
"lib/rake_probe.rb".ext(".txt")
String.new("lib/rake_probe.rb").pathmap("%n")
String.new("lib/rake_probe.rb").not_a_rake_extension
"#;

fn missing_methods(source: &str, dsl_activation: DslActivation) -> BTreeSet<String> {
    let lazy_loader =
        LazyRbsLoader::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core"));
    let file_path = "lib/rake_probe.rb";
    let options = AnalysisOptions {
        rails_mode: false,
        dsl_activation,
        project_versions: ProjectVersions::default(),
        project_root: None,
    };
    let (analysis, _, _) = analyze_source_for_display(
        source,
        None,
        Some(&lazy_loader),
        None,
        Some(file_path),
        options,
    );
    cli_diagnostics_from_snapshot(&analysis, source, file_path, &lazy_loader, None, None)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == "missing_method")
        .map(|diagnostic| diagnostic.unresolved_method)
        .collect()
}

#[test]
fn rake_string_extensions_are_scoped_to_rake_projects() {
    let rake_project = tempdir().expect("tempdir");
    std::fs::write(
        rake_project.path().join("Gemfile"),
        "source 'https://rubygems.org'\ngem 'rake'\n",
    )
    .expect("write Rake Gemfile");
    let detected = detect_dsl_libraries_from_gems(rake_project.path());
    assert!(detected.contains(&DslLibrary::Rake));

    let missing = missing_methods(SOURCE, DslActivation::with_auto_detected(detected));
    assert!(!missing.contains("String#ext"), "{missing:?}");
    assert!(!missing.contains("String#pathmap"), "{missing:?}");
    assert!(
        missing.contains("String#not_a_rake_extension"),
        "{missing:?}"
    );

    let plain_project = tempdir().expect("tempdir");
    std::fs::write(
        plain_project.path().join("Gemfile"),
        "source 'https://rubygems.org'\ngem 'minitest'\n",
    )
    .expect("write non-Rake Gemfile");
    let detected = detect_dsl_libraries_from_gems(plain_project.path());
    assert!(!detected.contains(&DslLibrary::Rake));

    let missing = missing_methods(SOURCE, DslActivation::with_auto_detected(detected));
    assert!(missing.contains("String#ext"), "{missing:?}");
    assert!(missing.contains("String#pathmap"), "{missing:?}");
}
