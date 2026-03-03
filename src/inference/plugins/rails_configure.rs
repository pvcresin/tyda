//! ! Rails boot-time configuration surfaces: ! - `Rails.application.configure do … end` blocks expose `config` as a bare !   call (`config/environments/*.rb`, `config/application.rb`).
//! The block !   runs with the application as `self`, which static analysis sees as !   `Object`, so `config` is provided as a path-gated synthetic.

use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::{Sym, Type};

pub(super) struct RailsConfigure;

static MANIFEST: PluginManifest = PluginManifest {
    id: "rails_configure",
    features: &[
        DslFeature {
            library: DslLibrary::RailsConfigure,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveJob,
            gem_markers: &[],
        },
    ],
    base_classes: APPLICATION_BASES,
    rails_default: true,
};

impl Plugin for RailsConfigure {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn synthetic_method_return(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        synthetic_method_return(cx, receiver_type, method_name)
    }

    fn synthetic_method_return_fallback(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        configuration_fallback_method_return(cx, receiver_type, method_name)
    }
}

const APPLICATION_BASES: &[&str] = &["Rails::Application"];

const ROUTES_DSL_METHODS: &[&str] = &[
    "resources",
    "resource",
    "member",
    "collection",
    "namespace",
    "scope",
    "root",
    "get",
    "post",
    "put",
    "patch",
    "delete",
    "match",
    "mount",
    "draw",
    "concern",
    "concerns",
    "constraints",
    "defaults",
    "direct",
    "resolve",
    "shallow",
    "devise_for",
    "devise_scope",
    "authenticate",
    "authenticated",
    "unauthenticated",
];

fn in_rails_config_file(engine: &PluginCx<'_, '_>) -> bool {
    engine.file_path().is_some_and(|path| {
        path.contains("/config/environments/")
            || path.contains("/config/initializers/")
            || path.ends_with("config/application.rb")
            || path.ends_with("config/environment.rb")
    })
}

fn in_routes_file(engine: &PluginCx<'_, '_>) -> bool {
    engine
        .file_path()
        .is_some_and(|path| path.ends_with("config/routes.rb") || path.contains("/config/routes/"))
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let class_name = match receiver_type {
        Type::Class(name) | Type::Singleton(name) => name.as_str(),
        _ => return None,
    };
    if !engine.dsl_enabled(DslLibrary::RailsConfigure) {
        return None;
    }
    if class_name == "Rails::Application"
        || engine.class_matches_or_inherits(class_name, APPLICATION_BASES)
    {
        return match method_name {
            "configure" => Some(Type::Untyped),
            "config" => Some(Type::Class(Sym::new("Rails::Application::Configuration"))),
            "credentials" | "secrets" | "config_for" => Some(Type::Untyped),
            "routes" | "reload_routes!" | "eager_load!" | "initialize!" => Some(Type::Untyped),
            _ => None,
        };
    }
    if class_name != "Object" {
        return None;
    }
    if method_name == "config" && in_rails_config_file(engine) {
        return Some(Type::Class(Sym::new("Rails::Application::Configuration")));
    }
    if ROUTES_DSL_METHODS.contains(&method_name) && in_routes_file(engine) {
        return Some(Type::Untyped);
    }
    None
}

/// `Rails::Application::Configuration` is a settings bag — any attribute (`config.assets`, `config.active_record`, custom keys) is readable and writable at runtime.
/// Catch-all: must run at the **end** of the plugin chain so named plugins keep precise types.
pub(in crate::inference) fn configuration_fallback_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    _method_name: &str,
) -> Option<Type> {
    let class_name = match receiver_type {
        Type::Class(name) | Type::Singleton(name) => name.as_str(),
        _ => return None,
    };
    if class_name != "Rails::Application::Configuration" {
        return None;
    }
    engine
        .dsl_enabled(DslLibrary::RailsConfigure)
        .then_some(Type::Untyped)
}
