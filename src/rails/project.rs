use std::path::Path;

use crate::project::{DslActivation, DslLibrary};
use crate::registry::{MethodDef, TypeRegistry};
use crate::types::{Sym, Type};

pub fn detect_rails(root: &Path) -> bool {
    if root.join("config").join("application.rb").exists() {
        return true;
    }
    let gemfile = root.join("Gemfile");
    if gemfile.exists()
        && let Ok(content) = std::fs::read_to_string(&gemfile)
    {
        return content.lines().any(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with('#')
                && (trimmed.contains("gem 'rails'")
                    || trimmed.contains("gem \"rails\"")
                    || trimmed.contains("gem 'railties'")
                    || trimmed.contains("gem \"railties\""))
        });
    }
    false
}

pub fn load_project_types(root: &Path, registry: &mut TypeRegistry) -> bool {
    load_project_types_with_activation(root, registry, &DslActivation::with_rails_mode(true))
}

pub fn load_project_types_with_activation(
    root: &Path,
    registry: &mut TypeRegistry,
    activation: &DslActivation,
) -> bool {
    let rails_mode = detect_rails(root);
    if !rails_mode || !activation.rails_mode_compat() {
        return false;
    }
    let preload_timing = std::env::var_os("TYDA_PRELOAD_TIMING").is_some();
    // Registers declared-gem / framework constants as known, so they don't trigger `unresolved_constant`.
    let t = std::time::Instant::now();
    registry
        .add_known_constant_namespaces(crate::project::known_external_constant_namespaces(root));
    let known_constants_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = std::time::Instant::now();
    if activation.is_enabled(DslLibrary::ActiveRecordColumns)
        || activation.is_enabled(DslLibrary::ActiveRecordAssociations)
        || activation.is_enabled(DslLibrary::ActiveRecordStore)
        || activation.is_enabled(DslLibrary::ActiveRecordTypedStore)
        || activation.is_enabled(DslLibrary::ActionText)
        || activation.is_enabled(DslLibrary::ActiveStorage)
    {
        super::schema::load_schema(root, registry);
    }
    let schema_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = std::time::Instant::now();
    if activation.is_enabled(DslLibrary::UrlHelpers) {
        super::routes::load_routes(root, registry);
    }
    let routes_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = std::time::Instant::now();
    if activation.is_enabled(DslLibrary::ActiveSupportEnvironmentInquirer) {
        load_environment_predicates(root, registry);
    }
    let env_predicates_ms = t.elapsed().as_secs_f64() * 1000.0;
    if preload_timing {
        eprintln!(
            "TIMING rails_project known_constants_ms={known_constants_ms:.3} schema_ms={schema_ms:.3} routes_ms={routes_ms:.3} env_predicates_ms={env_predicates_ms:.3}",
        );
    }
    true
}

fn load_environment_predicates(root: &Path, registry: &mut TypeRegistry) {
    let env_dir = root.join("config").join("environments");
    let Ok(entries) = std::fs::read_dir(env_dir) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "rb") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let predicate = format!("{stem}?");
        for class_name in [
            "ActiveSupport::StringInquirer",
            "ActiveSupport::EnvironmentInquirer",
        ] {
            registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(&predicate),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Bool,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: None,
                },
            );
        }
    }
}
