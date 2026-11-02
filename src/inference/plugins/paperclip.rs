//! Paperclip's `has_attached_file` model declaration.

use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::prism_compat::LocationExt;
use crate::project::DslLibrary;
use crate::types::Type;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct Paperclip;

static MANIFEST: PluginManifest = PluginManifest {
    id: "paperclip",
    features: &[DslFeature {
        library: DslLibrary::Paperclip,
        gem_markers: &["paperclip"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Paperclip {
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
        if method_name != "has_attached_file"
            || !cx.dsl_enabled(DslLibrary::Paperclip)
            || !cx.is_active_record_model_class(class_name)
        {
            return false;
        }
        let Some(name) = cx.first_symbol_or_string_arg(call_node) else {
            return false;
        };
        let loc = cx.offset_to_location(parse_result.source(), call_node.location().start_offset());
        cx.add_simple_method_if_missing(class_name, &name, Type::Untyped, false, loc);
        true
    }
}
