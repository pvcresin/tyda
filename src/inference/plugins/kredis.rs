use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct Kredis;

static MANIFEST: PluginManifest = PluginManifest {
    id: "kredis",
    features: &[DslFeature {
        library: DslLibrary::Kredis,
        gem_markers: &["kredis"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Kredis {
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
        if method_name.starts_with("kredis_") && cx.dsl_enabled(DslLibrary::Kredis) {
            cx.collect_kredis_dsl(class_name, call_node, method_name, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_prefixes(&self) -> &'static [&'static str] {
        &["kredis_"]
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_kredis_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let type_suffix = method_name.trim_start_matches("kredis_");
        let type_name = format!("Kredis::Types::{}", self.camelize(type_suffix));
        for name in self.symbol_or_string_args(call_node) {
            self.add_accessor_methods(
                class_name,
                &name,
                Type::Class(Sym::new(&type_name)),
                false,
                loc,
            );
        }
    }
}
