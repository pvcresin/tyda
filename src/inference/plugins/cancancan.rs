//! CanCanCan: `can?` / `cannot?` in controllers and view helpers.

use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;

pub(super) struct Cancancan;

static MANIFEST: PluginManifest = PluginManifest {
    id: "cancancan",
    features: &[DslFeature {
        library: DslLibrary::Cancancan,
        gem_markers: &["cancancan", "cancan"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Cancancan {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn synthetic_method_return(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let Type::Class(class_name) = receiver_type else {
            return None;
        };
        (matches!(method_name, "can?" | "cannot?")
            && cx.dsl_enabled(DslLibrary::Cancancan)
            && super::action_controller::is_controller_context(cx, class_name))
        .then_some(Type::Bool)
    }
}
