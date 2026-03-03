use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct Protobuf;

static MANIFEST: PluginManifest = PluginManifest {
    id: "protobuf",
    features: &[DslFeature {
        library: DslLibrary::Protobuf,
        gem_markers: &["google-protobuf", "protobuf"],
    }],
    base_classes: &[],
    rails_default: false,
};

const FIELD_METHODS: &[&str] = &["optional", "required", "repeated"];

impl Plugin for Protobuf {
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
        if FIELD_METHODS.contains(&method_name) && cx.dsl_enabled(DslLibrary::Protobuf) {
            cx.collect_protobuf_field_dsl(class_name, call_node, method_name, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        FIELD_METHODS
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_protobuf_field_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let Some(name) = self.first_symbol_or_string_arg(call_node) else {
            return;
        };
        let base = Type::Untyped;
        let ty = if method_name == "repeated" {
            Type::Array(Some(Box::new(base)))
        } else {
            base
        };
        self.add_accessor_methods(class_name, &name, ty, false, loc);
    }
}
