//! ! GitLab's in-tree runtime utilities with no gem marker: ! ! - The EE extension-loading mechanism (`prepend_mod` family): GitLab FOSS !   monkey-patches `Module` with `prepend_mod`, `prepend_mod_with('EE::Foo')`, !   `include_mod(_with)` and `extend_mod(_with)` — each loads the EE !   counterpart module when present and is a no-op otherwise.
//! The patch !   lives in a Rails initializer outside the analyzed app code, and !   singleton-method lookup does not walk the `Class`/`Module` metaclass !   chain, so every `Foo.prepend_mod` shows up unresolved.

use super::{Plugin, PluginCx, PluginManifest};
use crate::types::Type;

pub(super) struct GitlabEe;

static MANIFEST: PluginManifest = PluginManifest {
    id: "gitlab_ee",
    features: &[],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for GitlabEe {
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

    fn consumes_class_body_call(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        method_name: &str,
    ) -> bool {
        consumes_class_body_call(cx, class_name, method_name)
    }
}

const EE_EXTENSION_METHODS: &[&str] = &[
    "prepend_mod",
    "prepend_mod_with",
    "include_mod",
    "include_mod_with",
    "extend_mod",
    "extend_mod_with",
];

fn is_gitlab_project(engine: &PluginCx<'_, '_>) -> bool {
    engine.registry().class_data_for("Gitlab").is_some()
        || engine
            .external_rbs()
            .is_some_and(|registry| registry.class_data_for("Gitlab").is_some())
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
    if class_name == "Gitlab::CurrentSettings" && is_gitlab_project(engine) {
        if method_name == "current_application_settings" {
            return Some(Type::Class(crate::types::Sym::new("ApplicationSetting")));
        }
        let on_settings = engine.resolve_method_on_type(
            &Type::Class(crate::types::Sym::new("ApplicationSetting")),
            method_name,
        );
        return match on_settings {
            Type::ReceiverMethodRef(..) | Type::MethodReturnRef(..) => Some(Type::Untyped),
            other => Some(other),
        };
    }
    if class_name == "Gitlab::Metrics" && is_gitlab_project(engine) {
        return match method_name {
            "counter" | "gauge" | "histogram" | "summary" | "client" | "registry" => {
                Some(Type::Untyped)
            }
            "prometheus_metrics_enabled?" | "error_detected!" | "error?" => Some(Type::Bool),
            "measure" | "record_duration_for_status?" | "server_error?" => Some(Type::Untyped),
            _ => None,
        };
    }
    let is_utility = EE_EXTENSION_METHODS.contains(&method_name)
        || matches!(
            method_name,
            "strong_memoize" | "strong_memoize_attr" | "strong_memoize_with" | "override"
        );
    if !is_utility {
        return None;
    }
    is_gitlab_project(engine).then_some(Type::Untyped)
}

/// Class-body directives (`strong_memoize_attr :x`, `override :x`,
/// `Foo.prepend_mod` trailers) — diagnostics suppression.
pub(in crate::inference) fn consumes_class_body_call(
    engine: &mut PluginCx<'_, '_>,
    _class_name: &str,
    method_name: &str,
) -> bool {
    (EE_EXTENSION_METHODS.contains(&method_name)
        || matches!(method_name, "strong_memoize_attr" | "override"))
        && is_gitlab_project(engine)
}
