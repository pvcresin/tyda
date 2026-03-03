use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct Properties;

static MANIFEST: PluginManifest = PluginManifest {
    id: "properties",
    features: &[
        DslFeature {
            library: DslLibrary::Config,
            gem_markers: &["config"],
        },
        DslFeature {
            library: DslLibrary::SmartProperties,
            gem_markers: &["smart_properties"],
        },
        DslFeature {
            library: DslLibrary::JsonApiClientResource,
            gem_markers: &["json_api_client", "jsonapi-client"],
        },
    ],
    base_classes: &[],
    rails_default: false,
};

const PROPERTY_LIBRARIES: &[DslLibrary] = &[
    DslLibrary::Config,
    DslLibrary::SmartProperties,
    DslLibrary::JsonApiClientResource,
];

impl Plugin for Properties {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn collect_class_body_call(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        method_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
        _comments: &[super::RbsComment],
    ) -> bool {
        if matches!(method_name, "setting" | "property") && cx.any_dsl_enabled(PROPERTY_LIBRARIES) {
            cx.collect_property_dsl(class_name, call_node, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        &["setting", "property"]
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_property_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let ty = self
            .hash_option_type(call_node, "default", parse_result)
            .unwrap_or(Type::Untyped);
        for name in self.symbol_or_string_args(call_node) {
            self.add_accessor_methods(class_name, &name, ty.clone(), false, loc);
        }
    }
}
