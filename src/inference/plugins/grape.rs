//! ! Grape (the `grape` gem): REST-like API DSL.
//! ! ! What Grape defines at runtime: ! - `helpers do … end` mixes the block's `def`s into every endpoint of the !   API class — modeled as instance methods of the class so route-block !   bare calls resolve to them.

use super::super::{ClassBodyCollectionOptions, RbsComment, Scope};
use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;
use ruby_prism::{Node, ParseResult};

pub(super) struct Grape;

static MANIFEST: PluginManifest = PluginManifest {
    id: "grape",
    features: &[DslFeature {
        library: DslLibrary::Grape,
        gem_markers: &["grape"],
    }],
    base_classes: GRAPE_BASES,
    rails_default: false,
};

impl Plugin for Grape {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn synthetic_method_return(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        synthetic_method_return(cx, receiver_type, method_name)
    }

    fn collect_class_body_call(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        method_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[RbsComment],
    ) -> bool {
        collect_class_body_call(
            cx,
            class_name,
            method_name,
            call_node,
            parse_result,
            comments,
        )
    }

    fn consumes_class_body_call(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        method_name: &str,
    ) -> bool {
        consumes_class_body_call(cx, class_name, method_name)
    }
}

const GRAPE_BASES: &[&str] = &["Grape::API", "Grape::API::Instance"];

/// Structural class-body calls whose blocks can contain nested `helpers` /
/// structural calls.
const STRUCTURAL_BLOCKS: &[&str] = &[
    "namespace",
    "resource",
    "resources",
    "route_param",
    "group",
    "segment",
    "version",
    "given",
    "mounted",
];

const ENDPOINT_METHODS: &[&str] = &[
    "params",
    "current_user",
    "status",
    "header",
    "headers",
    "body",
    "declared",
    "present",
    "redirect",
    "route",
    "request",
    "env",
    "version",
    "cookies",
    "content_type",
    "sendfile",
    "stream",
    "return_no_content",
    "configuration",
];

const DESC_BLOCK_METHODS: &[&str] = &[
    "summary",
    "detail",
    "success",
    "failure",
    "named",
    "nickname",
    "is_array",
    "produces",
    "consumes",
    "tags",
    "hidden",
    "deprecated",
    "security",
    "entity",
    "http_codes",
];

const PARAMS_BLOCK_METHODS: &[&str] = &[
    "requires",
    "optional",
    "exactly_one_of",
    "at_least_one_of",
    "all_or_none_of",
    "mutually_exclusive",
    "with",
    "use",
    "declared_params",
];

const ROUTE_DSL_METHODS: &[&str] = &[
    "get",
    "post",
    "put",
    "patch",
    "delete",
    "head",
    "mount",
    "before",
    "after",
    "before_validation",
    "after_validation",
    "rescue_from",
    "format",
    "content_type",
    "default_format",
    "default_error_formatter",
    "default_error_status",
    "route_setting",
    "auth",
    "http_basic",
    "http_digest",
    "desc",
    "params",
];

fn is_grape_api_class(engine: &PluginCx<'_, '_>, class_name: &str) -> bool {
    engine.dsl_enabled(DslLibrary::Grape)
        && engine.class_matches_or_inherits(class_name, GRAPE_BASES)
}

/// Class-body DSL words this plugin recognizes (diagnostics suppression).
pub(in crate::inference) fn consumes_class_body_call(
    engine: &mut PluginCx<'_, '_>,
    class_name: &str,
    method_name: &str,
) -> bool {
    (method_name == "helpers"
        || STRUCTURAL_BLOCKS.contains(&method_name)
        || ROUTE_DSL_METHODS.contains(&method_name))
        && is_grape_api_class(engine, class_name)
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let class_name = match receiver_type {
        Type::Class(name) | Type::Singleton(name) => name.as_str(),
        _ => return None,
    };
    if !is_grape_api_class(engine, class_name) {
        return None;
    }
    if method_name == "error!" {
        return Some(Type::Bot);
    }
    if ENDPOINT_METHODS.contains(&method_name)
        || DESC_BLOCK_METHODS.contains(&method_name)
        || PARAMS_BLOCK_METHODS.contains(&method_name)
        || STRUCTURAL_BLOCKS.contains(&method_name)
    {
        return Some(Type::Untyped);
    }
    None
}

pub(in crate::inference) fn collect_class_body_call(
    engine: &mut PluginCx<'_, '_>,
    class_name: &str,
    method_name: &str,
    call_node: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
    comments: &[RbsComment],
) -> bool {
    if !is_grape_api_class(engine, class_name) {
        return false;
    }
    match method_name {
        "helpers" => {
            collect_helpers_call(engine, class_name, call_node, parse_result, comments);
            true
        }
        name if STRUCTURAL_BLOCKS.contains(&name) => {
            // Only descend looking for nested `helpers` / structural blocks;
            // routes themselves carry no method definitions.
            if let Some(block) = call_node.block().and_then(|raw| raw.as_block_node())
                && let Some(body) = block.body()
            {
                walk_structure_for_helpers(engine, class_name, &body, parse_result, comments);
            }
            true
        }
        _ => false,
    }
}

fn collect_helpers_call(
    engine: &mut PluginCx<'_, '_>,
    class_name: &str,
    call_node: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
    comments: &[RbsComment],
) {
    if let Some(args) = call_node.arguments() {
        for arg in args.arguments().iter() {
            if matches!(
                arg,
                Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. }
            ) {
                let module_name = engine.resolve_constant_path(&arg, parse_result);
                if !module_name.is_empty() {
                    engine.add_dsl_include_mixin(class_name, &module_name);
                }
            }
        }
    }
    if let Some(block) = call_node.block().and_then(|raw| raw.as_block_node())
        && let Some(body) = block.body()
    {
        engine.collect_class_body_inner(
            class_name,
            &body,
            parse_result,
            comments,
            ClassBodyCollectionOptions::new(false, Scope::default()),
        );
    }
}

fn walk_structure_for_helpers(
    engine: &mut PluginCx<'_, '_>,
    class_name: &str,
    body: &Node<'_>,
    parse_result: &ParseResult<'_>,
    comments: &[RbsComment],
) {
    let Some(statements) = body.as_statements_node() else {
        return;
    };
    for statement in statements.body().iter() {
        let Some(call) = statement.as_call_node() else {
            continue;
        };
        if call.receiver().is_some() {
            continue;
        }
        let name = String::from_utf8_lossy(call.name().as_slice()).to_string();
        if name == "helpers" {
            collect_helpers_call(engine, class_name, &call, parse_result, comments);
        } else if STRUCTURAL_BLOCKS.contains(&name.as_str())
            && let Some(block) = call.block().and_then(|raw| raw.as_block_node())
            && let Some(inner) = block.body()
        {
            walk_structure_for_helpers(engine, class_name, &inner, parse_result, comments);
        }
    }
}
