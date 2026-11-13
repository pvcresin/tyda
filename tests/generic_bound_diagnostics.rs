use std::path::PathBuf;

use tyda::analysis::{
    AnalysisOptions, analyze_cli_diagnostic_target_snapshot_timed, analyze_source_for_display,
    cli_diagnostics_from_snapshot,
};
use tyda::project::{DslActivation, ProjectVersions};
use tyda::rbs::stdlib_loader::LazyRbsLoader;
use tyda::registry::TypeRegistry;

fn argument_mismatch_lines(source: &str, rbs_source: &str, compact: bool) -> Vec<u32> {
    let vendor_rbs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs");
    let lazy_loader =
        LazyRbsLoader::from_rbs_roots(vec![vendor_rbs.join("core"), vendor_rbs.join("stdlib")]);
    let mut workspace_registry = TypeRegistry::new();
    tyda::rbs::import::load_rbs_string(rbs_source, &mut workspace_registry);
    let file_path = "generic_bound.rb";
    let options = AnalysisOptions {
        rails_mode: false,
        dsl_activation: DslActivation::default(),
        project_versions: ProjectVersions::default(),
        project_root: None,
    };
    let snapshot = if compact {
        analyze_cli_diagnostic_target_snapshot_timed(
            source,
            Some(&workspace_registry),
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
            Some(&workspace_registry),
            Some(&lazy_loader),
            None,
            Some(file_path),
            options,
        )
        .0
    };
    cli_diagnostics_from_snapshot(
        &snapshot,
        source,
        file_path,
        &lazy_loader,
        None,
        Some(&workspace_registry),
    )
    .into_iter()
    .filter(|diagnostic| diagnostic.code == "argument_type_mismatch")
    .map(|diagnostic| diagnostic.line)
    .collect()
}

// A bound inferred from callers changes as callers change, so another caller's
// class must not be reported.
#[test]
fn inferred_parameters_do_not_report_other_callers_classes() {
    let source = concat!(
        "class Namer\n",
        "  def touch(x)\n",
        "    x.to_s\n",
        "    x\n",
        "  end\n",
        "end\n",
        "Namer.new.touch(1)\n",
        "Namer.new.touch(:s)\n",
    );
    for compact in [false, true] {
        let lines = argument_mismatch_lines(source, "", compact);
        assert!(lines.is_empty(), "compact={compact}: {lines:?}");
    }
}

#[test]
fn sorbet_type_parameter_binds_to_the_block_return() {
    let source = concat!(
        "class Gen\n",
        "  extend T::Sig\n",
        "  sig { type_parameters(:U).params(blk: T.proc.returns(T.type_parameter(:U))).returns(T.type_parameter(:U)) }\n",
        "  def self.run(&blk) = yield\n",
        "end\n",
        "class Consumer\n",
        "  extend T::Sig\n",
        "  sig { params(s: String).void }\n",
        "  def self.take(s); end\n",
        "  def self.go = take(Gen.run { Object.new })\n",
        "end\n",
    );
    for compact in [false, true] {
        let lines = argument_mismatch_lines(source, "", compact);
        assert_eq!(lines, vec![10], "compact={compact}");
    }
}

// Each call passes an argument outside an explicit type parameter bound. Passing a value
// within the bound instead is not reported.
#[test]
fn explicit_bound_violations_are_reported() {
    let cases = [
        (
            "an RBS method bound",
            "class Bounded\n  def self.keep: [T < Integer] (T x) -> T\nend\n",
            "Bounded.keep(:s)\n",
            1,
        ),
        (
            "an RBS method union bound",
            "class Bounded\n  def self.keep: [T < Integer | String] (T x) -> T\nend\n",
            "Bounded.keep(:s)\n",
            1,
        ),
        (
            "an RBS class bound",
            "class NumBox[T < Numeric]\n  def initialize: (T value) -> void\nend\n",
            "class Maker\n  def make = NumBox.new(\"s\")\nend\n",
            2,
        ),
        (
            "an inline RBS method bound",
            "",
            concat!(
                "class Bounded\n",
                "  #: [T < Integer] (T x) -> T\n",
                "  def keep(x) = x\n",
                "end\n",
                "Bounded.new.keep(:s)\n",
            ),
            5,
        ),
        (
            "an @rbs generic class bound",
            "",
            concat!(
                "# @rbs generic T < Numeric\n",
                "class NumBox\n",
                "  #: (T) -> void\n",
                "  def initialize(value) = @value = value\n",
                "end\n",
                "class Maker\n",
                "  def make = NumBox.new(\"s\")\n",
                "end\n",
            ),
            7,
        ),
        (
            "a Sorbet type_member upper bound",
            "",
            concat!(
                "class NumBox\n",
                "  extend T::Generic\n",
                "  Elem = type_member { { upper: Numeric } }\n",
                "\n",
                "  sig { params(value: Elem).void }\n",
                "  def initialize(value) = @value = value\n",
                "end\n",
                "class Maker\n",
                "  def make = NumBox.new(\"s\")\n",
                "end\n",
            ),
            9,
        ),
    ];

    for (name, rbs_source, source, line) in cases {
        for compact in [false, true] {
            let lines = argument_mismatch_lines(source, rbs_source, compact);
            assert_eq!(lines, vec![line], "{name}, compact={compact}");
            let within_bound = source.replace(":s", "1").replace("\"s\"", "1");
            let lines = argument_mismatch_lines(&within_bound, rbs_source, compact);
            assert!(
                lines.is_empty(),
                "{name} within bound, compact={compact}: {lines:?}"
            );
        }
    }
}
