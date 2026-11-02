use std::collections::BTreeSet;
use std::path::PathBuf;

use tyda::analysis::{AnalysisOptions, analyze_source_for_display, cli_diagnostics_from_snapshot};
use tyda::project::{DslActivation, DslLibrary, ProjectVersions, detect_dsl_from_source_text};
use tyda::rbs::import::load_rbs_string;
use tyda::rbs::stdlib_loader::LazyRbsLoader;
use tyda::registry::TypeRegistry;

const SOURCE: &str = r#"
class ActiveRecord::Base
end

class User < ActiveRecord::Base
  devise :database_authenticatable, :confirmable,
    :two_factor_authenticatable, :two_factor_backupable
end

class PasswordUser < ActiveRecord::Base
  devise :database_authenticatable
end

class Configuration < ActiveRecord::Base
end

class Owner < ActiveRecord::Base
  has_one :configuration, class_name: "Configuration"
end

class Backup < ActiveRecord::Base
  has_attached_file :file
end

class Plain
  has_attached_file :file
end

class Rack::Attack::Request
end

Plain.build.file
Owner.new.create_configuration!
Rack::Attack::Request.new.params
Rack::Attack::Request.new.not_a_rack_request_method
"#;

fn diagnostics_for(
    source: &str,
    dsl_activation: DslActivation,
) -> Vec<tyda::diagnostics::TypeDiagnostic> {
    let lazy_loader =
        LazyRbsLoader::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core"));
    let file_path = "app/models/dynamic_library_dsl_probe.rb";
    let options = AnalysisOptions {
        rails_mode: false,
        dsl_activation,
        project_versions: ProjectVersions::default(),
        project_root: None,
    };
    let mut rbs_registry = TypeRegistry::new();
    load_rbs_string(
        "class Plain\n  def self.build: -> Plain\nend\n",
        &mut rbs_registry,
    );
    let (analysis, _, _) = analyze_source_for_display(
        source,
        Some(&rbs_registry),
        Some(&lazy_loader),
        None,
        Some(file_path),
        options,
    );
    cli_diagnostics_from_snapshot(&analysis, source, file_path, &lazy_loader, None, None)
}

fn missing_methods(diagnostics: &[tyda::diagnostics::TypeDiagnostic]) -> BTreeSet<&str> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "missing_method")
        .map(|diagnostic| diagnostic.unresolved_method.as_str())
        .collect()
}

#[test]
fn generated_dynamic_library_methods_are_scoped_and_unknowns_stay_missing() {
    let mut detected = BTreeSet::new();
    detect_dsl_from_source_text(SOURCE, &mut detected);
    assert!(detected.contains(&DslLibrary::Devise));
    assert!(detected.contains(&DslLibrary::Paperclip));
    assert!(detected.contains(&DslLibrary::RackAttack));

    let diagnostics = diagnostics_for(SOURCE, DslActivation::with_auto_detected(detected));
    let missing = missing_methods(&diagnostics);

    assert!(missing.contains("Plain#has_attached_file"), "{missing:?}");
    assert!(missing.contains("Plain#file"), "{missing:?}");
    assert!(
        missing.contains("Rack::Attack::Request#not_a_rack_request_method"),
        "{missing:?}"
    );
    for method in [
        "Backup#has_attached_file",
        "Owner#create_configuration!",
        "Rack::Attack::Request#params",
    ] {
        assert!(
            !missing.contains(method),
            "unexpected missing method: {method}; {missing:?}"
        );
    }
}

#[test]
fn rack_attack_request_integration_is_disabled_when_the_feature_is_forced_off() {
    let mut detected = BTreeSet::new();
    detect_dsl_from_source_text(SOURCE, &mut detected);
    let mut activation = DslActivation::with_auto_detected(detected);
    activation.forced_off.insert(DslLibrary::RackAttack);

    let diagnostics = diagnostics_for(SOURCE, activation);
    let missing = missing_methods(&diagnostics);
    assert!(
        missing.contains("Rack::Attack::Request#params"),
        "{missing:?}"
    );
}
