//! Oj (the `oj` gem), a fast JSON serializer.
//! `Oj` is a C-extension gem with no RBS, so callers see it as an undefined-constant phantom stub. Its module functions have no source definition, so we resolve them synthetically: load methods (JSON string -> arbitrary JSON value) return `untyped`, dump methods (Ruby object -> JSON string) return `String`.

use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;

pub(super) struct Oj;

static MANIFEST: PluginManifest = PluginManifest {
    id: "oj",
    features: &[DslFeature {
        library: DslLibrary::Oj,
        gem_markers: &["oj"],
    }],
    base_classes: &[],
    rails_default: false,
};

const LOAD_METHODS: &[&str] = &["load", "strict_load", "compat_load", "object_load"];
const DUMP_METHODS: &[&str] = &["dump", "generate", "to_json"];

fn oj_module_function_return(method_name: &str) -> Option<Type> {
    if LOAD_METHODS.contains(&method_name) {
        Some(Type::Untyped)
    } else if DUMP_METHODS.contains(&method_name) {
        Some(Type::String)
    } else {
        None
    }
}

fn receiver_is_oj(receiver_type: &Type) -> bool {
    matches!(
        receiver_type,
        Type::Singleton(name) if name.as_str().trim_start_matches("::") == "Oj"
    )
}

impl Plugin for Oj {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn synthetic_method_return(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        if !receiver_is_oj(receiver_type) {
            return None;
        }
        let ret = oj_module_function_return(method_name)?;
        cx.dsl_enabled(DslLibrary::Oj).then_some(ret)
    }

    fn synthetic_method_return_override(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        if !receiver_is_oj(receiver_type) {
            return None;
        }
        let ret = oj_module_function_return(method_name)?;
        if !cx.dsl_enabled(DslLibrary::Oj) {
            return None;
        }
        // Defer to normal resolution if Oj itself has a real definition (project code / external RBS).
        // Only takes priority over the universal fallback (other owners like Kernel).
        if cx
            .registry()
            .lookup_method_def("Oj", method_name, true)
            .is_some()
        {
            return None;
        }
        if cx
            .external_rbs()
            .is_some_and(|rbs| rbs.lookup_method_def("Oj", method_name, true).is_some())
        {
            return None;
        }
        Some(ret)
    }
}
