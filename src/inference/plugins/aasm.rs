use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct Aasm;

static MANIFEST: PluginManifest = PluginManifest {
    id: "aasm",
    features: &[DslFeature {
        library: DslLibrary::Aasm,
        gem_markers: &["aasm"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Aasm {
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
        if method_name == "aasm" && cx.dsl_enabled(DslLibrary::Aasm) {
            cx.collect_aasm_dsl(class_name, call_node, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        &["aasm"]
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_aasm_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.collect_state_machine_block_like(class_name, call_node, parse_result, loc);
    }
}
