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

#[test]
fn singleton_method_returns_resolve_with_singleton_ivar_context() {
    let source = concat!(
        "class SingletonIvarDiagnostics\n",
        "  @value = \"singleton\"\n",
        "  def initialize\n",
        "    @value = nil\n",
        "  end\n",
        "  def value = @value\n",
        "  def self.value = @value\n",
        "  def self.call\n",
        "    value.missing_method\n",
        "  end\n",
        "  def self.call_instance_method\n",
        "    new.value.missing_method\n",
        "  end\n",
        "end\n",
    );

    let actual = diagnostics(source, false);
    let missing_methods: Vec<_> = actual
        .iter()
        .filter(|diagnostic| diagnostic.code == "missing_method")
        .map(|diagnostic| (diagnostic.line, diagnostic.unresolved_method.as_str()))
        .collect();

    assert!(
        missing_methods.contains(&(9, "String#missing_method")),
        "the singleton method returns String: {missing_methods:?}"
    );
    assert!(
        !missing_methods.contains(&(9, "NilClass#missing_method")),
        "the instance-side nil ivar must not leak into singleton diagnostics: {missing_methods:?}"
    );
    assert!(
        missing_methods.contains(&(12, "NilClass#missing_method")),
        "an explicit instance receiver must keep instance ivar types in singleton scope: {missing_methods:?}"
    );
    assert!(
        !missing_methods.contains(&(12, "String#missing_method")),
        "singleton ivars must not leak into explicit instance calls: {missing_methods:?}"
    );
}

#[test]
fn singleton_method_discoverability_uses_singleton_dispatch() {
    let source = concat!(
        "class SingletonMethodMissing\n",
        "  def value = \"instance\"\n",
        "  def self.call\n",
        "    value\n",
        "  end\n",
        "end\n",
    );

    let actual = diagnostics(source, false);
    let missing_methods: Vec<_> = actual
        .iter()
        .filter(|diagnostic| diagnostic.code == "missing_method")
        .map(|diagnostic| (diagnostic.line, diagnostic.unresolved_method.as_str()))
        .collect();

    assert!(
        missing_methods.contains(&(4, "SingletonMethodMissing#value")),
        "an instance method must not make a missing singleton call discoverable: {missing_methods:?}"
    );
}

#[test]
fn singleton_ivar_write_from_dup_keeps_the_receiver_type_in_local_flow() {
    let source = concat!(
        "class SingletonConfigFlow\n",
        "  @defaults = {}\n",
        "  @config = nil\n",
        "  def self.load\n",
        "    @config = @defaults.dup\n",
        "    @config.merge!({})\n",
        "  end\n",
        "end\n",
    );

    let actual = diagnostics(source, false);
    let false_positive_nil_receivers: Vec<_> = actual
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == "missing_method" && diagnostic.unresolved_method == "NilClass#merge!"
        })
        .collect();

    assert!(
        false_positive_nil_receivers.is_empty(),
        "the local dup result is a Hash: {false_positive_nil_receivers:?}"
    );
}

#[test]
fn untracked_ivar_nil_and_known_union_members_remain_diagnosable() {
    let source = concat!(
        "class IvarDiagnosticTarget\n",
        "end\n",
        "class IvarDiagnosticFlow\n",
        "  def initialize\n",
        "    @receiver = nil\n",
        "  end\n",
        "  def assign\n",
        "    @receiver = IvarDiagnosticTarget.new\n",
        "  end\n",
        "  def read\n",
        "    @receiver.missing_method\n",
        "  end\n",
        "  def local_nil\n",
        "    @value = nil\n",
        "    @value.missing_method\n",
        "  end\n",
        "  def nil_factory = nil\n",
        "  def call_nil_factory\n",
        "    @value = nil_factory\n",
        "    @value.missing_method\n",
        "  end\n",
        "  def direct_nil\n",
        "    nil.missing_method\n",
        "  end\n",
        "end\n",
    );

    let actual = diagnostics(source, false);
    let missing_methods: Vec<_> = actual
        .iter()
        .filter(|diagnostic| diagnostic.code == "missing_method")
        .map(|diagnostic| (diagnostic.line, diagnostic.unresolved_method.as_str()))
        .collect();

    assert!(
        missing_methods.contains(&(11, "IvarDiagnosticTarget#missing_method")),
        "the known non-nil union member remains diagnosable: {missing_methods:?}"
    );
    assert!(
        missing_methods.contains(&(11, "NilClass#missing_method")),
        "the ivar can be read before its later assignment: {missing_methods:?}"
    );
    assert_eq!(
        missing_methods
            .iter()
            .filter(|(_, method)| *method == "NilClass#missing_method")
            .count(),
        4,
        "local nil facts, the untracked ivar, a nil-returning call, and a literal nil remain diagnosable: {missing_methods:?}"
    );
}

#[test]
fn a_truthy_guard_narrows_an_untracked_ivar_union_without_making_it_constant() {
    let source = concat!(
        "class GuardedIvarValue\n",
        "end\n",
        "class GuardedIvarFlow\n",
        "  def initialize\n",
        "    @value = nil\n",
        "  end\n",
        "  def assign\n",
        "    @value = GuardedIvarValue.new\n",
        "  end\n",
        "  def read\n",
        "    if @value\n",
        "      @value.only_in_truthy_branch\n",
        "    end\n",
        "  end\n",
        "end\n",
    );

    let actual = diagnostics(source, false);
    let missing_methods: Vec<_> = actual
        .iter()
        .filter(|diagnostic| diagnostic.code == "missing_method")
        .map(|diagnostic| (diagnostic.line, diagnostic.unresolved_method.as_str()))
        .collect();

    assert!(
        missing_methods.contains(&(12, "GuardedIvarValue#only_in_truthy_branch")),
        "the truthy branch keeps the non-nil ivar type: {missing_methods:?}"
    );
    assert!(
        !missing_methods.contains(&(12, "NilClass#only_in_truthy_branch")),
        "the nil member is removed by the same-method guard: {missing_methods:?}"
    );
}

#[test]
fn later_writes_in_a_case_do_not_invalidate_earlier_ivar_reads() {
    let source = concat!(
        "class CaseIvarEvaluationOrder\n",
        "  def update(event)\n",
        "    @comma = \"(\"\n",
        "    if @comma\n",
        "      case event\n",
        "      when :append\n",
        "        @comma << \")\"\n",
        "      end\n",
        "      @comma = nil\n",
        "      @comma.upcase\n",
        "    end\n",
        "  end\n",
        "end\n",
    );

    let actual = diagnostics(source, false);
    let missing_methods: Vec<_> = actual
        .iter()
        .filter(|diagnostic| diagnostic.code == "missing_method")
        .map(|diagnostic| (diagnostic.line, diagnostic.unresolved_method.as_str()))
        .collect();

    assert!(
        !missing_methods.contains(&(7, "NilClass#<<")),
        "the String fact is valid before the later case branch write: {missing_methods:?}"
    );
    assert!(
        missing_methods.contains(&(10, "NilClass#upcase")),
        "the explicit nil write invalidates the fact for later reads: {missing_methods:?}"
    );
}

#[test]
fn hash_seeded_inject_keeps_its_receiver_in_nested_blocks() {
    let source = concat!(
        "class InjectMergeFlow\n",
        "  def self.call(params)\n",
        "    if params.is_a?(Hash) && params.key?(:keywords)\n",
        "      attributes = params.keys\n",
        "      params[:keywords].each_with_index do |keywords, index|\n",
        "        next if keywords.empty?\n",
        "        attributes.inject({}) do |hash, key|\n",
        "          hash[key.to_s] = keywords[index].to_s\n",
        "          hash\n",
        "        end.merge('keywords' => keywords)\n",
        "      end\n",
        "    end\n",
        "  end\n",
        "end\n",
    );

    let actual = diagnostics(source, false);
    let nil_merge_calls: Vec<_> = actual
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == "missing_method" && diagnostic.unresolved_method == "NilClass#merge"
        })
        .collect();

    assert!(
        nil_merge_calls.is_empty(),
        "Array#inject with a Hash seed returns the accumulator: {nil_merge_calls:?}"
    );
}

#[test]
fn only_nonempty_all_class_guards_narrow_the_first_array_element() {
    let source = concat!(
        "class ArrayFirstGuardDiagnostics\n",
        "  #: (Hash[Symbol, (Hash[Symbol, Integer] | nil)]) -> untyped\n",
        "  def checked_values(value)\n",
        "    if value.values.any? && value.values.all?(Hash)\n",
        "      value = value.values.first\n",
        "      value.fetch(:id)\n",
        "    end\n",
        "  end\n",
        "  #: (Hash[Symbol, (Hash[Symbol, Integer] | nil)]) -> untyped\n",
        "  def any_only(value)\n",
        "    if value.values.any?\n",
        "      value = value.values.first\n",
        "      value.fetch(:id)\n",
        "    end\n",
        "  end\n",
        "  #: (Hash[Symbol, (Hash[Symbol, Integer] | nil)]) -> untyped\n",
        "  def all_only(value)\n",
        "    if value.values.all?(Hash)\n",
        "      value = value.values.first\n",
        "      value.fetch(:id)\n",
        "    end\n",
        "  end\n",
        "  #: (Hash[Symbol, (Hash[Symbol, Integer] | nil)]) -> untyped\n",
        "  def cleared_after_guard(value)\n",
        "    if value.values.any? && value.values.all?(Hash)\n",
        "      value.clear\n",
        "      value = value.values.first\n",
        "      value.fetch(:id)\n",
        "    end\n",
        "  end\n",
        "  #: (Hash[Symbol, (Hash[Symbol, Integer] | nil)]) -> untyped\n",
        "  def any_block_only(value)\n",
        "    if value.values.any? { |item| item.is_a?(Hash) }\n",
        "      value = value.values.first\n",
        "      value.fetch(:id)\n",
        "    end\n",
        "  end\n",
        "end\n",
    );

    for compact in [false, true] {
        let actual = diagnostics(source, compact);
        let nil_key_lines: Vec<_> = actual
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == "missing_method"
                    && diagnostic.unresolved_method == "NilClass#fetch"
            })
            .map(|diagnostic| diagnostic.line)
            .collect();

        assert_eq!(
            nil_key_lines,
            [13, 20, 28, 35],
            "compact={compact}: only the paired, side-effect-free guard narrows the first element: {actual:?}"
        );
    }
}

#[test]
fn shellwords_extension_resolves_after_method_local_require() {
    let source = concat!(
        "def escaped\n",
        "  require \"shellwords\"\n",
        "  \"tool name\".shellescape\n",
        "end\n",
        "escaped\n",
    );
    let actual = diagnostics(source, false);
    assert!(
        !actual.iter().any(|diagnostic| {
            diagnostic.code == "missing_method"
                && diagnostic.unresolved_method == "String#shellescape"
        }),
        "method-local require should load Shellwords before the call: {actual:?}"
    );
}
