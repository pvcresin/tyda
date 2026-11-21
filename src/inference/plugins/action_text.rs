use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::prism_compat::LocationExt;
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

const RICH_TEXT_METHODS: &[&str] = &["has_rich_text"];

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

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_action_text_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        _method_name: &str,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let Some(name) = self.first_symbol_or_string_arg(call_node) else {
            return;
        };
        // The reader builds the record when it is missing, so it is never nil.
        let rich_text_class = if self.hash_option_bool(call_node, "encrypted", parse_result) {
            "ActionText::EncryptedRichText"
        } else {
            "ActionText::RichText"
        };
        self.add_simple_method_if_missing(
            class_name,
            &name,
            Type::Class(Sym::new(rich_text_class)),
            false,
            loc,
        );
        self.add_simple_method_if_missing(class_name, &format!("{name}?"), Type::Bool, false, loc);
        let body = Type::Union(vec![
            Type::Class(Sym::new("ActionText::Content")),
            Type::Class(Sym::new("ActionText::RichText")),
            Type::String,
            Type::Nil,
        ]);
        self.add_method_with_param_if_missing(
            class_name,
            &format!("{name}="),
            &name,
            body.clone(),
            body,
            false,
            loc,
        );
    }
}
