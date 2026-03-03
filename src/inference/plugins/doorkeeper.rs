use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;

pub(super) struct Doorkeeper;

static MANIFEST: PluginManifest = PluginManifest {
    id: "doorkeeper",
    features: &[DslFeature {
        library: DslLibrary::Doorkeeper,
        gem_markers: &["doorkeeper"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Doorkeeper {
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
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let Type::Class(class_name) = receiver_type else {
        return None;
    };
    let result = match method_name {
        "doorkeeper_authorize!" | "doorkeeper_render_error" => Type::Untyped,
        "doorkeeper_token" => Type::Untyped,
        "valid_doorkeeper_token?" => Type::Bool,
        _ => return None,
    };
    if !engine.dsl_enabled(DslLibrary::Doorkeeper) {
        return None;
    }
    engine
        .is_action_controller_class(class_name)
        .then_some(result)
}
