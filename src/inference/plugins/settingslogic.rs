//! ! Settingslogic (the `settingslogic` gem) and its derivatives.
//! ! ! A `Settingslogic` subclass loads a YAML file and exposes each top-level key ! as a method (`Config.hosts`, `Config.aws.region`, …) via `method_missing` — ! there is no source definition for those accessors.

use super::{Plugin, PluginCx, PluginManifest};
use crate::types::Type;

pub(super) struct Settingslogic;

const SETTINGSLOGIC_BASES: &[&str] = &["Settingslogic"];

static MANIFEST: PluginManifest = PluginManifest {
    id: "settingslogic",
    features: &[],
    base_classes: SETTINGSLOGIC_BASES,
    rails_default: false,
};

impl Plugin for Settingslogic {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn synthetic_method_return_fallback(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        _method_name: &str,
    ) -> Option<Type> {
        let class_name = match receiver_type {
            Type::Class(name) | Type::Singleton(name) => name,
            _ => return None,
        };
        cx.class_matches_or_inherits(class_name, SETTINGSLOGIC_BASES)
            .then_some(Type::Untyped)
    }
}
