use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;

pub(super) struct Gettext;

static MANIFEST: PluginManifest = PluginManifest {
    id: "gettext",
    features: &[DslFeature {
        library: DslLibrary::Gettext,
        gem_markers: &["fast_gettext", "gettext_i18n_rails", "gettext"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Gettext {
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

const TRANSLATION_METHODS: &[&str] = &[
    "_", "s_", "n_", "N_", "Nn_", "ns_", "np_", "np", "p_", "pgettext", "sgettext", "ngettext",
    "gettext",
];

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    if !TRANSLATION_METHODS.contains(&method_name) {
        return None;
    }
    if !matches!(
        receiver_type,
        Type::Class(_) | Type::Generic { .. } | Type::Singleton(_)
    ) {
        return None;
    }
    engine
        .dsl_enabled(DslLibrary::Gettext)
        .then_some(Type::String)
}
