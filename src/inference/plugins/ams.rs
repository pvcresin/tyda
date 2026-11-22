use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;

pub(super) struct Ams;

static MANIFEST: PluginManifest = PluginManifest {
    id: "ams",
    features: &[DslFeature {
        library: DslLibrary::ActiveModelSerializers,
        gem_markers: &["active_model_serializers"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Ams {
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

const SERIALIZER_INSTANCE_METHODS: &[&str] = &[
    "object",
    "scope",
    "current_user",
    "instance_options",
    "serialization_options",
    "root",
    "read_attribute_for_serialization",
];

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let Type::Class(class_name) = receiver_type else {
        return None;
    };
    if !engine.dsl_enabled(DslLibrary::ActiveModelSerializers) {
        return None;
    }
    if !SERIALIZER_INSTANCE_METHODS.contains(&method_name) {
        return None;
    }
    engine
        .is_active_model_serializer_class(class_name)
        .then_some(Type::Untyped)
}
