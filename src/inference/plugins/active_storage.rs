use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct ActiveStorage;

static MANIFEST: PluginManifest = PluginManifest {
    id: "active_storage",
    features: &[DslFeature {
        library: DslLibrary::ActiveStorage,
        gem_markers: &[],
    }],
    base_classes: &[],
    rails_default: true,
};

const ATTACH_METHODS: &[&str] = &["has_one_attached", "has_many_attached"];

impl Plugin for ActiveStorage {
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
        if ATTACH_METHODS.contains(&method_name) && cx.dsl_enabled(DslLibrary::ActiveStorage) {
            cx.collect_active_storage_dsl(class_name, call_node, method_name, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        ATTACH_METHODS
    }
}

use super::super::*;

const ACTIVE_STORAGE_ONE_CLASS: &str = "ActiveStorage::Attached::One";
const ACTIVE_STORAGE_MANY_CLASS: &str = "ActiveStorage::Attached::Many";

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_active_storage_dsl(
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
        let ty = if method_name == "has_many_attached" {
            Type::Class(Sym::new(ACTIVE_STORAGE_MANY_CLASS))
        } else {
            Type::Class(Sym::new(ACTIVE_STORAGE_ONE_CLASS))
        };
        self.add_accessor_methods(class_name, &name, ty, false, loc);
    }
}
