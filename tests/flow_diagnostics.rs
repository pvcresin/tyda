use std::path::PathBuf;

use tyda::analysis::{
    AnalysisOptions, analyze_cli_diagnostic_target_snapshot_timed, analyze_source_for_display,
    cli_diagnostics_from_snapshot,
};
use tyda::project::{DslActivation, ProjectVersions};
use tyda::rbs::stdlib_loader::LazyRbsLoader;

fn diagnostics(source: &str, compact: bool) -> Vec<tyda::diagnostics::TypeDiagnostic> {
    let vendor_rbs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs");
    let lazy_loader =
        LazyRbsLoader::from_rbs_roots(vec![vendor_rbs.join("core"), vendor_rbs.join("stdlib")]);
    let file_path = "optional_nil_guard.rb";
    let options = AnalysisOptions {
        rails_mode: false,
        dsl_activation: DslActivation::default(),
        project_versions: ProjectVersions::default(),
        project_root: None,
    };
    let snapshot = if compact {
        analyze_cli_diagnostic_target_snapshot_timed(
            source,
            None,
            &lazy_loader,
            None,
            file_path,
            options,
            true,
        )
        .0
    } else {
        analyze_source_for_display(
            source,
            None,
            Some(&lazy_loader),
            None,
            Some(file_path),
            options,
        )
        .0
    };
    cli_diagnostics_from_snapshot(&snapshot, source, file_path, &lazy_loader, None, None)
}

#[test]
fn nil_guard_on_optional_parameters_removes_nil_but_preserves_false() {
    let direct = diagnostics("false.chomp\n", false);
    let direct_missing: Vec<_> = direct
        .iter()
        .filter(|diagnostic| diagnostic.code == "missing_method")
        .map(|diagnostic| diagnostic.unresolved_method.as_str())
        .collect();
    assert_eq!(direct_missing, ["FalseClass#chomp"], "{direct:?}");
    let source = concat!(
        "class OptionalNilGuard\n",
        "  def access(path = nil)\n",
        "    return if path.nil?\n",
        "    path.chomp\n",
        "  end\n",
        "  def equality(path = nil)\n",
        "    return if path == nil\n",
        "    path.chomp\n",
        "  end\n",
        "  def keyword(path: nil)\n",
        "    return if path.nil?\n",
        "    path.chomp\n",
        "  end\n",
        "end\n",
        "OptionalNilGuard.new.access(\"path\")\n",
        "OptionalNilGuard.new.equality(\"path\")\n",
        "OptionalNilGuard.new.keyword(path: \"path\")\n",
    );
    let false_source = concat!(
        "class OptionalFalseGuard\n",
        "  #: (String | false | nil) -> untyped\n",
        "  def access(path)\n",
        "    return if path.nil?\n",
        "    path.chomp\n",
        "  end\n",
        "end\n",
    );

    for compact in [false, true] {
        let actual = diagnostics(source, compact);
        let missing_methods: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.code == "missing_method")
            .map(|diagnostic| diagnostic.unresolved_method.as_str())
            .collect();
        assert!(
            !missing_methods.contains(&"NilClass#chomp"),
            "compact={compact}: {missing_methods:?}"
        );

        let actual = diagnostics(false_source, compact);
        let missing_methods: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.code == "missing_method")
            .map(|diagnostic| diagnostic.unresolved_method.as_str())
            .collect();
        assert!(
            missing_methods.contains(&"FalseClass#chomp"),
            "compact={compact}: {missing_methods:?}"
        );
        assert!(
            !missing_methods.contains(&"NilClass#chomp"),
            "compact={compact}: {missing_methods:?}"
        );
    }
}
