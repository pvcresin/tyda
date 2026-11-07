use super::{Plugin, PluginCx, PluginManifest};
use crate::types::Type;

pub(super) struct Shellwords;

static MANIFEST: PluginManifest = PluginManifest {
    id: "shellwords",
    features: &[],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Shellwords {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn synthetic_method_return(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        if !cx.stdlib_feature_loaded("shellwords") || !is_string_receiver(receiver_type) {
            return None;
        }

        match method_name {
            "shellescape" => Some(Type::String),
            "shellsplit" => Some(Type::Array(Some(Box::new(Type::String)))),
            _ => None,
        }
    }
}

fn is_string_receiver(receiver_type: &Type) -> bool {
    match receiver_type {
        Type::String | Type::LiteralString(_) => true,
        Type::Class(name) => name.as_str() == "String",
        _ => false,
    }
}
