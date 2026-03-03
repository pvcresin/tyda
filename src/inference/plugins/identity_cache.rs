use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct IdentityCache;

static MANIFEST: PluginManifest = PluginManifest {
    id: "identity_cache",
    features: &[DslFeature {
        library: DslLibrary::IdentityCache,
        gem_markers: &["identity_cache"],
    }],
    base_classes: &[],
    rails_default: false,
};

const CACHE_METHODS: &[&str] = &["cache_index", "cache_has_many", "cache_belongs_to"];

impl Plugin for IdentityCache {
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
        if CACHE_METHODS.contains(&method_name) && cx.dsl_enabled(DslLibrary::IdentityCache) {
            cx.collect_identity_cache_dsl(class_name, call_node, method_name, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        CACHE_METHODS
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_identity_cache_dsl(
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
        self.add_method_with_param_if_missing(
            class_name,
            &format!("fetch_by_{name}"),
            &name,
            Type::Untyped,
            Type::Union(vec![Type::SelfType, Type::Nil]),
            true,
            loc,
        );
        self.add_method_with_param_if_missing(
            class_name,
            &format!("fetch_multi_by_{name}"),
            &name,
            Type::Array(Some(Box::new(Type::Untyped))),
            Type::Array(Some(Box::new(Type::SelfType))),
            true,
            loc,
        );
    }
}
