//! String extensions provided by Rake.

use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;

pub(super) struct Rake;

static MANIFEST: PluginManifest = PluginManifest {
    id: "rake",
    features: &[DslFeature {
        library: DslLibrary::Rake,
        gem_markers: &["rake"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Rake {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn synthetic_method_return(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        if !cx.dsl_enabled(DslLibrary::Rake) || !is_string_receiver(receiver_type) {
            return None;
        }

        matches!(method_name, "ext" | "pathmap").then_some(Type::String)
    }
}

fn is_string_receiver(receiver_type: &Type) -> bool {
    match receiver_type {
        Type::String | Type::LiteralString(_) => true,
        Type::Class(name) => name.as_str() == "String",
        _ => false,
    }
}
