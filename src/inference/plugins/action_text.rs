use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct ActionText;

static MANIFEST: PluginManifest = PluginManifest {
    id: "action_text",
    features: &[DslFeature {
        library: DslLibrary::ActionText,
        gem_markers: &[],
    }],
    base_classes: &[],
    rails_default: true,
};

const RICH_TEXT_METHODS: &[&str] = &["has_rich_text", "has_many_rich_texts"];

impl Plugin for ActionText {
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
        if RICH_TEXT_METHODS.contains(&method_name) && cx.dsl_enabled(DslLibrary::ActionText) {
            cx.collect_action_text_dsl(class_name, call_node, method_name, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        RICH_TEXT_METHODS
    }
}

use super::super::*;

const ACTION_TEXT_RICH_TEXT_CLASS: &str = "ActionText::RichText";

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_action_text_dsl(
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
        let ty = if method_name == "has_many_rich_texts" {
            Type::Array(Some(Box::new(Type::Class(Sym::new(
                ACTION_TEXT_RICH_TEXT_CLASS,
            )))))
        } else {
            Type::Union(vec![
                Type::Class(Sym::new(ACTION_TEXT_RICH_TEXT_CLASS)),
                Type::Nil,
            ])
        };
        self.add_accessor_methods(class_name, &name, ty, false, loc);
    }
}
