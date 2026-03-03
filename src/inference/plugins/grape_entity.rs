use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;

pub(super) struct GrapeEntity;

static MANIFEST: PluginManifest = PluginManifest {
    id: "grape_entity",
    features: &[DslFeature {
        library: DslLibrary::GrapeEntity,
        gem_markers: &["grape-entity", "grape_entity"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for GrapeEntity {
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

const ENTITY_BASES: &[&str] = &["Grape::Entity", "API::Entities::Base"];

const ENTITY_CLASS_DSL: &[&str] = &[
    "expose",
    "unexpose",
    "with_options",
    "format_with",
    "root",
    "present_collection",
    "represent",
    "entity_name",
];

fn is_entity_class(engine: &PluginCx<'_, '_>, class_name: &str) -> bool {
    engine.dsl_enabled(DslLibrary::GrapeEntity)
        && engine.class_matches_or_inherits(class_name, ENTITY_BASES)
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let (class_name, is_singleton) = match receiver_type {
        Type::Class(name) => (name.as_str(), false),
        Type::Singleton(name) => (name.as_str(), true),
        _ => return None,
    };
    let recognized = if is_singleton {
        ENTITY_CLASS_DSL.contains(&method_name)
    } else {
        matches!(method_name, "object" | "options" | "represent")
    };
    if !recognized {
        return None;
    }
    is_entity_class(engine, class_name).then_some(Type::Untyped)
}

/// Class-body DSL words (diagnostics suppression for `expose :x` etc.).
pub(in crate::inference) fn consumes_class_body_call(
    engine: &mut PluginCx<'_, '_>,
    class_name: &str,
    method_name: &str,
) -> bool {
    ENTITY_CLASS_DSL.contains(&method_name) && is_entity_class(engine, class_name)
}
