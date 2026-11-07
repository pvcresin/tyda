//! Rack::Attack request integration.

use super::{Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::{SourceLocation, Type};

pub(super) struct RackAttack;

static MANIFEST: PluginManifest = PluginManifest {
    id: "rack_attack",
    features: &[super::DslFeature {
        library: DslLibrary::RackAttack,
        gem_markers: &["rack-attack"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for RackAttack {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn register_on_class(&self, cx: &mut PluginCx<'_, '_>, class_name: &str, loc: SourceLocation) {
        if !cx.dsl_enabled(DslLibrary::RackAttack) || class_name != "Rack::Attack::Request" {
            return;
        }

        let inherits_from_rack_request = match cx.registry().get_superclass(class_name) {
            None => {
                cx.registry_mut()
                    .set_superclass(class_name, "Rack::Request");
                true
            }
            Some(superclass) => superclass == "Rack::Request",
        };
        if inherits_from_rack_request {
            cx.add_simple_method_if_missing("Rack::Request", "params", Type::Untyped, false, loc);
        }
    }
}
