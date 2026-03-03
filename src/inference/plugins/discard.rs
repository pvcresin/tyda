use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::SourceLocation;

pub(super) struct Discard;

static MANIFEST: PluginManifest = PluginManifest {
    id: "discard",
    features: &[DslFeature {
        library: DslLibrary::Discard,
        gem_markers: &["discard"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Discard {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn register_on_mixin(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        cx.register_discard_mixin_methods(class_name, module_name, loc);
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn register_discard_mixin_methods(
        &mut self,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        if self.discard_dsl_enabled() && module_name == "Discard::Model" {
            for name in ["discard", "discard!", "undiscard", "undiscard!"] {
                self.add_simple_method_if_missing(class_name, name, Type::Bool, false, loc);
            }
            for name in ["discarded?", "kept?", "undiscarded?"] {
                self.add_simple_method_if_missing(class_name, name, Type::Bool, false, loc);
            }
            let relation_type = Self::active_record_relation_type(class_name);
            for name in ["kept", "undiscarded", "discarded", "with_discarded"] {
                self.add_simple_method_if_missing(
                    class_name,
                    name,
                    relation_type.clone(),
                    true,
                    loc,
                );
            }
        }
    }
}
