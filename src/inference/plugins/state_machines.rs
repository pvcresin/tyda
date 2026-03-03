use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct StateMachines;

static MANIFEST: PluginManifest = PluginManifest {
    id: "state_machines",
    features: &[DslFeature {
        library: DslLibrary::StateMachines,
        gem_markers: &[
            "state_machines",
            "state_machines-activerecord",
            "state_machines-activemodel",
        ],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for StateMachines {
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
        if method_name == "state_machine" && cx.dsl_enabled(DslLibrary::StateMachines) {
            cx.collect_state_machine_dsl(class_name, call_node, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        &["state_machine"]
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_state_machine_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.collect_state_machine_block_like(class_name, call_node, parse_result, loc);
    }

    pub(in crate::inference) fn collect_state_machine_block_like(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        _parse_result: &ParseResult<'_>,
        loc: SourceLocation,
    ) {
        if let Some(block_raw) = call_node.block()
            && let Some(block) = block_raw.as_block_node()
            && let Some(body) = block.body()
            && let Node::StatementsNode { .. } = &body
        {
            let statements = body.as_statements_node().expect("must be StatementsNode");
            for stmt in statements.body().iter() {
                if let Node::CallNode { .. } = &stmt {
                    let inner = stmt.as_call_node().expect("must be CallNode");
                    let inner_name = String::from_utf8_lossy(inner.name().as_slice()).to_string();
                    match inner_name.as_str() {
                        "state" => {
                            for name in self.symbol_or_string_args(&inner) {
                                self.add_simple_method_if_missing(
                                    class_name,
                                    &format!("{name}?"),
                                    Type::Bool,
                                    false,
                                    loc,
                                );
                            }
                        }
                        "event" => {
                            if let Some(name) = self.first_symbol_or_string_arg(&inner) {
                                self.add_simple_method_if_missing(
                                    class_name,
                                    &format!("may_{name}?"),
                                    Type::Bool,
                                    false,
                                    loc,
                                );
                                self.add_simple_method_if_missing(
                                    class_name,
                                    &name,
                                    Type::Bool,
                                    false,
                                    loc,
                                );
                                self.add_simple_method_if_missing(
                                    class_name,
                                    &format!("{name}!"),
                                    Type::Bool,
                                    false,
                                    loc,
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
