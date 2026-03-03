use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct ActiveResource;

static MANIFEST: PluginManifest = PluginManifest {
    id: "active_resource",
    features: &[DslFeature {
        library: DslLibrary::ActiveResource,
        gem_markers: &["activeresource"],
    }],
    base_classes: &[],
    rails_default: true,
};

impl Plugin for ActiveResource {
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
        if method_name == "schema" && cx.dsl_enabled(DslLibrary::ActiveResource) {
            cx.collect_active_resource_schema_dsl(class_name, call_node, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        &["schema"]
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_active_resource_schema_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        if let Some(block_raw) = call_node.block()
            && let Some(block) = block_raw.as_block_node()
            && let Some(body) = block.body()
            && let Node::StatementsNode { .. } = &body
        {
            let statements = body.as_statements_node().expect("must be StatementsNode");
            for stmt in statements.body().iter() {
                if let Node::CallNode { .. } = &stmt {
                    let inner = stmt.as_call_node().expect("must be CallNode");
                    let method_name = String::from_utf8_lossy(inner.name().as_slice()).to_string();
                    if matches!(
                        method_name.as_str(),
                        "string"
                            | "integer"
                            | "float"
                            | "decimal"
                            | "boolean"
                            | "date"
                            | "datetime"
                            | "time"
                    ) {
                        for name in self.symbol_or_string_args(&inner) {
                            self.add_accessor_methods(class_name, &name, Type::Untyped, false, loc);
                        }
                    }
                }
            }
        }
    }
}
