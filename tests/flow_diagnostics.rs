use std::path::PathBuf;

use tyda::analysis::{
    AnalysisOptions, analyze_cli_diagnostic_target_snapshot_timed, analyze_source_for_display,
    cli_diagnostics_from_snapshot,
};
use tyda::project::{DslActivation, ProjectVersions};
use tyda::rbs::stdlib_loader::LazyRbsLoader;
use tyda::registry::TypeRegistry;

fn diagnostics(source: &str, compact: bool) -> Vec<tyda::diagnostics::TypeDiagnostic> {
    diagnostics_with_registry(source, None, compact)
}

fn diagnostics_with_registry(
    source: &str,
    workspace_registry: Option<&TypeRegistry>,
    compact: bool,
) -> Vec<tyda::diagnostics::TypeDiagnostic> {
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
            workspace_registry,
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
            workspace_registry,
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
        workspace_registry,
    )
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
fn singleton_param_refs_use_enclosing_dispatch_for_same_named_methods() {
    let source = concat!(
        "class LoadParamConfig\n",
        "end\n",
        "class LoadParamCollision\n",
        "  def load(path)\n",
        "    LoadParamCollision.consume(path)\n",
        "    path\n",
        "  end\n",
        "  def self.load(conf)\n",
        "    [1].each do\n",
        "      conf.singleton_only_missing\n",
        "    end\n",
        "    LoadParamCollision.consume(conf)\n",
        "    conf\n",
        "  end\n",
        "end\n",
        "class OppositeInstanceSignature\n",
        "  def self.load(value)\n",
        "    value.singleton_only_missing\n",
        "  end\n",
        "end\n",
        "class OppositeSingletonSignature\n",
        "  def load(value)\n",
        "    value.instance_only_missing\n",
        "  end\n",
        "end\n",
        "LoadParamCollision.new.load(\"cart.nes\")\n",
        "LoadParamCollision.load(LoadParamConfig.new)\n",
    );
    let rbs_source = concat!(
        "class LoadParamConfig\n",
        "end\n",
        "class LoadParamCollision\n",
        "  def load: (String) -> Integer\n",
        "  def self.load: (LoadParamConfig) -> Integer\n",
        "  def self.consume: (LoadParamConfig) -> void\n",
        "end\n",
        "class OppositeInstanceSignature\n",
        "  def load: (String) -> String\n",
        "end\n",
        "class OppositeSingletonSignature\n",
        "  def self.load: (LoadParamConfig) -> String\n",
        "end\n",
    );
    let mut workspace_registry = TypeRegistry::new();
    tyda::rbs::import::load_rbs_string(rbs_source, &mut workspace_registry);

    for compact in [false, true] {
        let actual = diagnostics_with_registry(source, Some(&workspace_registry), compact);
        let missing_methods: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.code == "missing_method")
            .map(|diagnostic| (diagnostic.line, diagnostic.unresolved_method.as_str()))
            .collect();

        assert!(
            missing_methods.contains(&(10, "LoadParamConfig#singleton_only_missing")),
            "compact={compact}: singleton param should use the singleton RBS signature: {missing_methods:?}"
        );
        assert!(
            !missing_methods.contains(&(10, "String#singleton_only_missing")),
            "compact={compact}: Kernel#load's String signature must not leak into the singleton method: {missing_methods:?}"
        );
        assert!(
            !missing_methods
                .iter()
                .any(|(line, _)| matches!(line, 18 | 23)),
            "compact={compact}: a signature from the opposite dispatch kind must not leak: {missing_methods:?}"
        );

        let argument_mismatches: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.code == "argument_type_mismatch")
            .map(|diagnostic| (diagnostic.line, diagnostic.message.as_str()))
            .collect();
        assert!(
            argument_mismatches.iter().any(|(line, message)| {
                *line == 5 && message.contains("String") && message.contains("LoadParamConfig")
            }),
            "compact={compact}: instance parameter should resolve to String: {argument_mismatches:?}"
        );
        assert!(
            !argument_mismatches.iter().any(|(line, _)| *line == 12),
            "compact={compact}: singleton parameter should resolve to LoadParamConfig: {argument_mismatches:?}"
        );

        let return_mismatches: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.code == "return_type_mismatch")
            .map(|diagnostic| (diagnostic.line, diagnostic.message.as_str()))
            .collect();
        assert!(
            return_mismatches
                .iter()
                .any(|(_, message)| message.contains("String")),
            "compact={compact}: instance return should resolve to String: {return_mismatches:?}"
        );
        assert!(
            return_mismatches
                .iter()
                .any(|(_, message)| message.contains("LoadParamConfig")),
            "compact={compact}: singleton return should resolve to LoadParamConfig: {return_mismatches:?}"
        );
    }

    let singleton_hover_offset = source.find("conf.singleton_only_missing").unwrap();
    let instance_hover_offset = source.find("LoadParamCollision.consume(path)").unwrap()
        + "LoadParamCollision.consume(".len();
    let vendor_rbs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs");
    let lazy_loader =
        LazyRbsLoader::from_rbs_roots(vec![vendor_rbs.join("core"), vendor_rbs.join("stdlib")]);
    for compact in [false, true] {
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
                "optional_nil_guard.rb",
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
                Some("optional_nil_guard.rb"),
                options,
            )
            .0
        };
        let instance_hover = snapshot
            .hover_at(
                source,
                instance_hover_offset,
                &lazy_loader,
                Some(&workspace_registry),
            )
            .unwrap();
        assert_eq!(
            instance_hover.ty.to_string(),
            "String",
            "compact={compact}: hover should resolve the instance method parameter"
        );
        let singleton_hover = snapshot
            .hover_at(
                source,
                singleton_hover_offset,
                &lazy_loader,
                Some(&workspace_registry),
            )
            .unwrap();
        assert_eq!(
            singleton_hover.ty.to_string(),
            "LoadParamConfig",
            "compact={compact}: hover should resolve the singleton method parameter"
        );
    }
}

#[test]
fn keyword_param_refs_use_enclosing_dispatch_for_same_named_methods() {
    let source = concat!(
        "class KeywordParamConfig\n",
        "end\n",
        "class KeywordParamCollision\n",
        "  def load(value:)\n",
        "    if value\n",
        "      value.instance_keyword_missing\n",
        "    end\n",
        "  end\n",
        "  def self.load(value:)\n",
        "    if value\n",
        "      value.singleton_keyword_missing\n",
        "    end\n",
        "  end\n",
        "end\n",
    );
    let rbs_source = concat!(
        "class KeywordParamConfig\n",
        "end\n",
        "class KeywordParamCollision\n",
        "  def load: (value: String) -> void\n",
        "  def self.load: (value: KeywordParamConfig) -> void\n",
        "end\n",
    );
    let mut workspace_registry = TypeRegistry::new();
    tyda::rbs::import::load_rbs_string(rbs_source, &mut workspace_registry);

    for compact in [false, true] {
        let actual = diagnostics_with_registry(source, Some(&workspace_registry), compact);
        let missing_methods: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.code == "missing_method")
            .map(|diagnostic| (diagnostic.line, diagnostic.unresolved_method.as_str()))
            .collect();
        assert!(
            missing_methods.contains(&(6, "String#instance_keyword_missing")),
            "compact={compact}: instance keyword parameter should use its instance signature: {missing_methods:?}"
        );
        assert!(
            missing_methods.contains(&(11, "KeywordParamConfig#singleton_keyword_missing")),
            "compact={compact}: singleton keyword parameter should use its singleton signature: {missing_methods:?}"
        );
        assert!(
            !missing_methods.iter().any(|(line, method)| {
                (*line == 6 && method.starts_with("KeywordParamConfig#"))
                    || (*line == 11 && method.starts_with("String#"))
            }),
            "compact={compact}: same-named opposite-dispatch signature must not leak: {missing_methods:?}"
        );
    }
}

#[test]
fn local_singleton_param_refs_require_a_proven_signature_owner() {
    let source = concat!(
        "class LocalSingletonSelfReceiver\n",
        "  def self.define\n",
        "    receiver = self\n",
        "    def receiver.load(value)\n",
        "      if value\n",
        "        value.singleton_only_missing\n",
        "      end\n",
        "    end\n",
        "    receiver.load(\"text\")\n",
        "  end\n",
        "end\n",
        "class LocalSingletonExternalOwner\n",
        "end\n",
        "class LocalSingletonOwnerMismatch\n",
        "  def self.define\n",
        "    receiver = LocalSingletonExternalOwner\n",
        "    def receiver.load(value)\n",
        "      if value\n",
        "        value.singleton_only_missing\n",
        "      end\n",
        "    end\n",
        "  end\n",
        "end\n",
        "class LocalSingletonObjectReceiver\n",
        "  def self.define\n",
        "    receiver = new\n",
        "    def receiver.load(value)\n",
        "      if value\n",
        "        value.singleton_only_missing\n",
        "      end\n",
        "    end\n",
        "    receiver.load(new)\n",
        "  end\n",
        "end\n",
        "class LocalSingletonUnknownReceiver\n",
        "  def self.define\n",
        "    receiver = unknown_receiver\n",
        "    def receiver.load(value)\n",
        "      if value\n",
        "        value.singleton_only_missing\n",
        "      end\n",
        "    end\n",
        "    receiver.load(\"text\")\n",
        "  end\n",
        "end\n",
    );
    let rbs_source = concat!(
        "class LocalSingletonSelfReceiver\n",
        "  def load: (Integer) -> void\n",
        "  def self.load: (String) -> void\n",
        "end\n",
        "class LocalSingletonExternalOwner\n",
        "  def self.load: (Symbol) -> void\n",
        "end\n",
        "class LocalSingletonOwnerMismatch\n",
        "  def load: (Integer) -> void\n",
        "  def self.load: (String) -> void\n",
        "end\n",
        "class LocalSingletonObjectReceiver\n",
        "  def load: (Integer) -> void\n",
        "  def self.load: (String) -> void\n",
        "end\n",
        "class LocalSingletonUnknownReceiver\n",
        "  def load: (Integer) -> void\n",
        "  def self.load: (String) -> void\n",
        "end\n",
    );
    let mut workspace_registry = TypeRegistry::new();
    tyda::rbs::import::load_rbs_string(rbs_source, &mut workspace_registry);

    for compact in [false, true] {
        let actual = diagnostics_with_registry(source, Some(&workspace_registry), compact);
        let missing_methods: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.code == "missing_method")
            .map(|diagnostic| (diagnostic.line, diagnostic.unresolved_method.as_str()))
            .collect();
        assert!(
            missing_methods.contains(&(6, "String#singleton_only_missing")),
            "compact={compact}: singleton-method `self` must use its class singleton signature: {missing_methods:?}"
        );
        assert!(
            !missing_methods.contains(&(6, "Integer#singleton_only_missing")),
            "compact={compact}: singleton-method `self` must not bind the instance signature: {missing_methods:?}"
        );
        assert!(
            !missing_methods
                .iter()
                .any(|(line, _)| matches!(line, 19 | 29 | 40)),
            "compact={compact}: object, unknown, or mismatched-owner receivers must not borrow a same-named class signature: {missing_methods:?}"
        );
    }
}

#[test]
fn param_ref_dispatch_follows_same_kind_ancestors_and_rejects_opposite_kind() {
    let source = concat!(
        "class ParamRefSingletonBase\n",
        "  def self.define\n",
        "    receiver = self\n",
        "    def receiver.call(value)\n",
        "      if value\n",
        "        value.singleton_ancestor_missing\n",
        "      end\n",
        "    end\n",
        "    receiver.call(\"text\")\n",
        "  end\n",
        "end\n",
        "class ParamRefSingletonChild < ParamRefSingletonBase\n",
        "end\n",
        "class ParamRefOppositeBase\n",
        "  def call(value)\n",
        "    if value\n",
        "      value.singleton_opposite_missing\n",
        "    end\n",
        "  end\n",
        "end\n",
        "class ParamRefOppositeChild < ParamRefOppositeBase\n",
        "  def self.define\n",
        "    receiver = self\n",
        "    def receiver.call(value)\n",
        "      if value\n",
        "        value.singleton_opposite_missing\n",
        "      end\n",
        "    end\n",
        "  end\n",
        "end\n",
    );
    let rbs_source = concat!(
        "class ParamRefSingletonBase\n",
        "  def call: (Integer) -> void\n",
        "  def self.call: (String) -> void\n",
        "end\n",
        "class ParamRefSingletonChild < ParamRefSingletonBase\n",
        "end\n",
        "class ParamRefOppositeBase\n",
        "  def call: (Integer) -> void\n",
        "end\n",
        "class ParamRefOppositeChild < ParamRefOppositeBase\n",
        "end\n",
    );
    let mut workspace_registry = TypeRegistry::new();
    tyda::rbs::import::load_rbs_string(rbs_source, &mut workspace_registry);

    for compact in [false, true] {
        let actual = diagnostics_with_registry(source, Some(&workspace_registry), compact);
        let missing_methods: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.code == "missing_method")
            .map(|diagnostic| (diagnostic.line, diagnostic.unresolved_method.as_str()))
            .collect();
        assert!(
            missing_methods.contains(&(6, "String#singleton_ancestor_missing")),
            "compact={compact}: singleton dispatch should inherit the parent singleton signature: {missing_methods:?}"
        );
        assert!(
            !missing_methods.contains(&(6, "Integer#singleton_ancestor_missing")),
            "compact={compact}: inherited instance signature must not cross dispatch kinds: {missing_methods:?}"
        );
        assert!(
            !missing_methods.contains(&(26, "Integer#singleton_opposite_missing")),
            "compact={compact}: an instance-only ancestor signature must not bind to a singleton method: {missing_methods:?}"
        );
    }
}

#[test]
fn param_ref_dispatch_preserves_extend_and_prepend_owner_order() {
    let source = concat!(
        "module ParamRefExtendedMethods\n",
        "end\n",
        "class ParamRefExtendedReceiver\n",
        "  extend ParamRefExtendedMethods\n",
        "  def self.define\n",
        "    receiver = self\n",
        "    def receiver.call(value)\n",
        "      if value\n",
        "        value.singleton_extend_missing\n",
        "      end\n",
        "    end\n",
        "    receiver.call(\"text\")\n",
        "  end\n",
        "end\n",
        "module ParamRefPrependedMethods\n",
        "end\n",
        "class ParamRefPrependedReceiver\n",
        "  prepend ParamRefPrependedMethods\n",
        "  def run(value)\n",
        "    if value\n",
        "      value.instance_prepend_missing\n",
        "    end\n",
        "  end\n",
        "end\n",
    );
    let rbs_source = concat!(
        "module ParamRefExtendedMethods\n",
        "  def call: (String) -> void\n",
        "end\n",
        "class ParamRefExtendedReceiver\n",
        "  extend ParamRefExtendedMethods\n",
        "  def call: (Integer) -> void\n",
        "end\n",
        "module ParamRefPrependedMethods\n",
        "  def run: (String) -> void\n",
        "end\n",
        "class ParamRefPrependedReceiver\n",
        "  prepend ParamRefPrependedMethods\n",
        "  def run: (Integer) -> void\n",
        "end\n",
    );
    let mut workspace_registry = TypeRegistry::new();
    tyda::rbs::import::load_rbs_string(rbs_source, &mut workspace_registry);

    for compact in [false, true] {
        let actual = diagnostics_with_registry(source, Some(&workspace_registry), compact);
        let missing_methods: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.code == "missing_method")
            .map(|diagnostic| (diagnostic.line, diagnostic.unresolved_method.as_str()))
            .collect();
        assert!(
            missing_methods.contains(&(9, "String#singleton_extend_missing")),
            "compact={compact}: class-object dispatch should follow `extend` owner order: {missing_methods:?}"
        );
        assert!(
            !missing_methods.contains(&(9, "Integer#singleton_extend_missing")),
            "compact={compact}: the class instance method must not override an extended method: {missing_methods:?}"
        );
        assert!(
            missing_methods.contains(&(21, "String#instance_prepend_missing")),
            "compact={compact}: instance dispatch should preserve Ruby prepend precedence: {missing_methods:?}"
        );
        assert!(
            !missing_methods.contains(&(21, "Integer#instance_prepend_missing")),
            "compact={compact}: the prepended method owner must precede the class method: {missing_methods:?}"
        );
    }
}

#[test]
fn local_singleton_param_context_keeps_explicit_self_call_dispatch() {
    let source = concat!(
        "class LocalExplicitSelfDispatch\n",
        "  def self.define\n",
        "    receiver = self\n",
        "    def receiver.load(value)\n",
        "      self.load(value)\n",
        "    end\n",
        "  end\n",
        "end\n",
    );
    let rbs_source = concat!(
        "class LocalExplicitSelfDispatch\n",
        "  def load: (Integer) -> void\n",
        "  def self.load: (String) -> void\n",
        "end\n",
    );
    let mut workspace_registry = TypeRegistry::new();
    tyda::rbs::import::load_rbs_string(rbs_source, &mut workspace_registry);

    for compact in [false, true] {
        let actual = diagnostics_with_registry(source, Some(&workspace_registry), compact);
        let mismatches: Vec<_> = actual
            .iter()
            .filter(|diagnostic| diagnostic.code == "argument_type_mismatch")
            .map(|diagnostic| (diagnostic.line, diagnostic.message.as_str()))
            .collect();
        assert!(
            !mismatches.iter().any(|(line, _)| *line == 5),
            "compact={compact}: explicit self inside a class-object singleton method should use the singleton signature: {mismatches:?}"
        );
    }
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

#[test]
fn direct_ivar_guarded_root_call_ignores_nil_from_later_writer_only() {
    let source = concat!(
        "class Resource\n",
        "  def close; end\n",
        "end\n",
        "class WriterAfterReader\n",
        "  def initialize\n",
        "    @resource = nil\n",
        "  end\n",
        "  def read\n",
        "    if @resource\n",
        "      @resource.close\n",
        "    end\n",
        "  end\n",
        "  def write\n",
        "    @resource = Resource.new\n",
        "  end\n",
        "end\n",
        "class WriterBeforeReader\n",
        "  def initialize\n",
        "    @resource = nil\n",
        "  end\n",
        "  def write\n",
        "    @resource = Resource.new\n",
        "  end\n",
        "  def read\n",
        "    if @resource\n",
        "      @resource.close\n",
        "    end\n",
        "  end\n",
        "end\n",
        "class OtherReceiver\n",
        "end\n",
        "class NonNilConcreteControl\n",
        "  def initialize\n",
        "    @resource = OtherReceiver.new\n",
        "  end\n",
        "  def read\n",
        "    if @resource\n",
        "      @resource.close\n",
        "    end\n",
        "  end\n",
        "end\n",
    );

    let actual = diagnostics(source, false);
    let nil_close_lines: Vec<_> = actual
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == "missing_method" && diagnostic.unresolved_method == "NilClass#close"
        })
        .map(|diagnostic| diagnostic.line)
        .collect();

    assert!(
        !nil_close_lines.contains(&10),
        "a direct ivar guard proves the writer-after-reader root call is non-nil: {nil_close_lines:?}"
    );
    assert!(
        !nil_close_lines.contains(&26),
        "writer-before-reader remains free of nil diagnostics: {nil_close_lines:?}"
    );
    assert!(
        actual.iter().any(|diagnostic| {
            diagnostic.code == "missing_method"
                && diagnostic.line == 38
                && diagnostic.unresolved_method == "OtherReceiver#close"
        }),
        "the direct call still reports a missing method for a concrete non-nil receiver: {actual:?}"
    );
}

#[test]
fn same_call_argument_mutation_keeps_nil_warning_after_direct_ivar_guard() {
    let source = concat!(
        "class Resource\n",
        "  def close; end\n",
        "end\n",
        "class SameCallArgumentMutation\n",
        "  def initialize\n",
        "    @resource = nil\n",
        "  end\n",
        "  def read\n",
        "    if @resource\n",
        "      consume(clear_resource, @resource.close)\n",
        "    end\n",
        "  end\n",
        "  def clear_resource\n",
        "    @resource = nil\n",
        "    true\n",
        "  end\n",
        "  def write_resource\n",
        "    @resource = Resource.new\n",
        "  end\n",
        "  def consume(first, second); end\n",
        "end\n",
    );

    let actual = diagnostics(source, false);
    let nil_close_lines: Vec<_> = actual
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == "missing_method" && diagnostic.unresolved_method == "NilClass#close"
        })
        .map(|diagnostic| diagnostic.line)
        .collect();

    assert!(
        nil_close_lines.contains(&10),
        "the first argument can clear @resource before the second argument is evaluated: {nil_close_lines:?}"
    );
}

#[test]
fn is_a_guard_refines_instance_variable_call_receiver() {
    let source = concat!(
        "class FlowBase\n",
        "  end\n",
        "class FlowChild < FlowBase\n",
        "  def child_only = \"child\"\n",
        "  end\n",
        "class IvarIsAGuard\n",
        "  def initialize\n",
        "    @value = FlowBase.new\n",
        "  end\n",
        "  def unguarded\n",
        "    @value.child_only\n",
        "  end\n",
        "  def guarded\n",
        "    if @value.is_a?(FlowChild)\n",
        "      value = [@value.child_only, self.is_a?(IvarIsAGuard)]\n",
        "      value\n",
        "    end\n",
        "  end\n",
        "end\n",
    );

    let actual = diagnostics(source, false);
    let child_method_diagnostics: Vec<_> = actual
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == "missing_method"
                && diagnostic.unresolved_method.ends_with("#child_only")
        })
        .collect();
    assert_eq!(
        child_method_diagnostics.len(),
        1,
        "the unguarded call should remain, while the call guarded by is_a? should be narrowed: {actual:?}"
    );
    assert_eq!(child_method_diagnostics[0].line, 11);
}

#[test]
fn lambdas_see_locals_of_the_enclosing_class_body_and_top_level() {
    let source = r#"
class CapturedByLambda
  class << self
    attr_accessor :filter
  end

  pattern = /a/
  self.filter = lambda { |value| pattern.match?(value) }
end

top_level_pattern = /a/
TOP_LEVEL_FILTER = lambda { |value| top_level_pattern.match?(value) }
"#;
    for compact in [true, false] {
        let found = diagnostics(source, compact);
        assert!(found.is_empty(), "compact={compact}: {found:?}");
    }
}

#[test]
fn deferred_repeat_count_does_not_report_missing_join() {
    let source = r#"
class RepeatedTokens
  def joined(tokens) = (["entry"] * tokens.size).join(" ")

  def joined_param(count)
    (["entry"] * count).join(" ")
  end
end
"#;
    for compact in [true, false] {
        let found = diagnostics(source, compact);
        assert!(found.is_empty(), "compact={compact}: {found:?}");
    }
}

#[test]
fn ivar_from_initialize_param_does_not_read_as_the_callers_param() {
    let source = r#"
class LocalAlias
  def initialize(app)
    @app = app
  end

  def request(method)
    app = @app
    app.call(method)
  end

  def run
    request("GET")
  end
end

class DefaultedIvar
  def initialize(root, app = nil)
    @app = app || Object.new
  end

  def list(env, path)
    return @app.call(env) if path
  end

  def run(env)
    list(env, "path")
  end
end
"#;

    for compact in [true, false] {
        let found = diagnostics(source, compact);
        assert!(
            !found
                .iter()
                .any(|diagnostic| diagnostic.message.contains("not found for `String`")),
            "compact={compact}: {found:?}"
        );
    }
}

#[test]
fn respond_to_guard_on_the_left_of_and_covers_the_right() {
    let source = r#"
class GuardedNote
end

class GuardedCaller
  def run(note)
    note.respond_to?(:zork) && note.zork
  end

  def call_run
    run(GuardedNote.new)
  end
end
"#;
    for compact in [true, false] {
        let found = diagnostics(source, compact);
        assert!(found.is_empty(), "compact={compact}: {found:?}");
    }
}

#[test]
fn class_object_answers_methods_declared_by_class() {
    let source = r#"
class Strategy
end

class KeywordDefault
  def initialize(strategy: Strategy)
    strategy.new
  end
end

class PositionalDefault
  def run(strategy = Strategy)
    strategy.new
  end
end
"#;
    for compact in [true, false] {
        let found = diagnostics(source, compact);
        assert!(found.is_empty(), "compact={compact}: {found:?}");
    }
}

#[test]
fn struct_classes_answer_methods_declared_by_struct() {
    let source = r#"
Pair = Struct.new(:left, :right)

class PairBuilder
  def build(flag)
    value = pair_or_hash(flag)
    if value.is_a?(Hash) && value[:left]
      value
    end
  end

  def pair_or_hash(flag)
    return { left: 1 } if flag

    Pair.new(1, 2)
  end

  def call_build
    build(true)
  end
end
"#;
    for compact in [true, false] {
        let found = diagnostics(source, compact);
        assert!(found.is_empty(), "compact={compact}: {found:?}");
    }
}

#[test]
fn template_method_defined_by_a_subclass_is_not_missing_on_self() {
    let source = r#"
class TemplateBase
  def run
    step
  end
end

class TemplateChild < TemplateBase
  def step
    1
  end
end

"#;
    for compact in [true, false] {
        let found = diagnostics(source, compact);
        assert!(
            !found.iter().any(|d| d.message.contains("`step`")),
            "compact={compact}: {found:?}"
        );
    }
}

#[test]
fn method_absent_from_every_subclass_stays_missing() {
    let source = r#"
class ClosedBase
end

class ClosedChild < ClosedBase
  def other
    1
  end
end

class ClosedCaller
  def go
    ClosedBase.new.missing_everywhere
  end
end
"#;
    for compact in [true, false] {
        let found = diagnostics(source, compact);
        assert!(
            found
                .iter()
                .any(|d| d.message.contains("missing_everywhere")),
            "compact={compact}: {found:?}"
        );
    }
}

#[test]
fn nil_guard_on_a_self_call_covers_later_reads() {
    let source = r#"
class GuardedReader
  def name
    rand > 1 ? "a" : nil
  end

  def run
    return false unless name

    name.upcase
  end
end
"#;
    for compact in [true, false] {
        let found = diagnostics(source, compact);
        assert!(found.is_empty(), "compact={compact}: {found:?}");
    }
}
