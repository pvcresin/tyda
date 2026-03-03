use std::path::Path;

use ruby_prism::{Node, ParseResult};

use super::inflector::singularize;
use crate::registry::{MethodDef, MixinKind, ParamInfo, TypeRegistry};
use crate::types::ParamKind;
use crate::types::{Sym, Type};

const ROUTE_HELPERS_MODULE: &str = "Rails::GeneratedUrlHelpers";

#[derive(Clone, Debug, Eq, PartialEq)]
struct RouteHelper {
    name: String,
    required_args: usize,
}

#[derive(Clone, Debug, Default)]
struct RouteContext {
    prefixes: Vec<String>,
    required_args: usize,
}

#[derive(Clone, Debug)]
struct ResourceContext {
    outer_prefixes: Vec<String>,
    collection_segment: String,
    member_segment: String,
    collection_args: usize,
    member_args: usize,
}

pub fn load_routes(root: &Path, registry: &mut TypeRegistry) {
    let routes_path = root.join("config").join("routes.rb");
    if !routes_path.exists() {
        return;
    }
    let Ok(source) = std::fs::read_to_string(&routes_path) else {
        return;
    };
    for helper in parse_route_helpers(&source) {
        register_route_helper(registry, &helper);
    }
}

fn parse_route_helpers(source: &str) -> Vec<RouteHelper> {
    let parse_result = ruby_prism::parse(source.as_bytes());
    let root = parse_result.node();
    let mut helpers = std::collections::BTreeMap::new();

    if let Node::ProgramNode { .. } = &root {
        let program = root.as_program_node().expect("root must be ProgramNode");
        for node in program.statements().body().iter() {
            collect_route_helpers(&node, &parse_result, &RouteContext::default(), &mut helpers);
        }
    }

    helpers
        .into_iter()
        .map(|(name, required_args)| RouteHelper {
            name,
            required_args,
        })
        .collect()
}

fn collect_route_helpers(
    node: &Node<'_>,
    parse_result: &ParseResult<'_>,
    context: &RouteContext,
    helpers: &mut std::collections::BTreeMap<String, usize>,
) {
    let Node::CallNode { .. } = node else {
        return;
    };
    let call = node.as_call_node().expect("must be CallNode");
    let method_name = String::from_utf8_lossy(call.name().as_slice()).to_string();

    match method_name.as_str() {
        "namespace" => {
            let mut nested = context.clone();
            if let Some(name) = first_route_name(&call, parse_result) {
                nested.prefixes.push(name);
            }
            collect_call_block(&call, parse_result, &nested, helpers);
        }
        "scope" => {
            let mut nested = context.clone();
            if let Some(as_name) = extract_hash_option(&call, "as", parse_result) {
                nested.prefixes.push(as_name);
            }
            collect_call_block(&call, parse_result, &nested, helpers);
        }
        "resources" => collect_resource_routes(&call, parse_result, context, false, helpers),
        "resource" => collect_resource_routes(&call, parse_result, context, true, helpers),
        "get" | "post" | "put" | "patch" | "delete" => {
            register_simple_route(&call, parse_result, context, helpers);
        }
        "root" => {
            insert_helper(
                helpers,
                join_route_name(&context.prefixes, "root"),
                context.required_args,
            );
        }
        _ => collect_call_block(&call, parse_result, context, helpers),
    }
}

fn first_route_name(
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
) -> Option<String> {
    let args = call.arguments()?;
    let first = args.arguments().iter().next()?;
    extract_string_or_symbol(&first, parse_result)
}

fn extract_hash_option(
    call: &ruby_prism::CallNode<'_>,
    key: &str,
    parse_result: &ParseResult<'_>,
) -> Option<String> {
    let args = call.arguments()?;
    for arg in args.arguments().iter() {
        match &arg {
            Node::KeywordHashNode { .. } => {
                let hash = arg.as_keyword_hash_node().expect("must be KeywordHashNode");
                for elem in hash.elements().iter() {
                    if let Node::AssocNode { .. } = &elem {
                        let assoc = elem.as_assoc_node().expect("must be AssocNode");
                        let assoc_key = extract_label_or_symbol(&assoc.key(), parse_result);
                        if assoc_key.as_deref() == Some(key) {
                            return extract_string_or_symbol(&assoc.value(), parse_result);
                        }
                    }
                }
            }
            Node::HashNode { .. } => {
                let hash = arg.as_hash_node().expect("must be HashNode");
                for elem in hash.elements().iter() {
                    if let Node::AssocNode { .. } = &elem {
                        let assoc = elem.as_assoc_node().expect("must be AssocNode");
                        let assoc_key = extract_label_or_symbol(&assoc.key(), parse_result);
                        if assoc_key.as_deref() == Some(key) {
                            return extract_string_or_symbol(&assoc.value(), parse_result);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_label_or_symbol(node: &Node<'_>, parse_result: &ParseResult<'_>) -> Option<String> {
    if let Node::SymbolNode { .. } = node {
        let sym = node.as_symbol_node().expect("must be SymbolNode");
        return Some(String::from_utf8_lossy(sym.unescaped()).to_string());
    }
    let raw = &parse_result.source()[node.location().start_offset()..node.location().end_offset()];
    let s = String::from_utf8_lossy(raw).to_string();
    let label = s.trim_end_matches(':');
    if label != s && !label.is_empty() {
        Some(label.to_string())
    } else {
        None
    }
}

fn extract_string_or_symbol(node: &Node<'_>, parse_result: &ParseResult<'_>) -> Option<String> {
    match node {
        Node::StringNode { .. } => {
            let string = node.as_string_node().expect("must be StringNode");
            Some(String::from_utf8_lossy(string.unescaped()).to_string())
        }
        Node::SymbolNode { .. } => {
            let sym = node.as_symbol_node().expect("must be SymbolNode");
            Some(String::from_utf8_lossy(sym.unescaped()).to_string())
        }
        Node::InterpolatedStringNode { .. } => {
            let raw = &parse_result.source()
                [node.location().start_offset()..node.location().end_offset()];
            Some(
                String::from_utf8_lossy(raw)
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string(),
            )
        }
        _ => None,
    }
}

fn join_route_name(prefixes: &[String], leaf: &str) -> String {
    if prefixes.is_empty() {
        leaf.to_string()
    } else {
        format!("{}_{}", prefixes.join("_"), leaf)
    }
}

fn collect_resource_routes(
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
    context: &RouteContext,
    singular_resource: bool,
    helpers: &mut std::collections::BTreeMap<String, usize>,
) {
    let Some(name) = first_route_name(call, parse_result) else {
        return;
    };
    let collection_segment = name.clone();
    let member_segment = singularize(&name);
    let collection_name = join_route_name(&context.prefixes, &collection_segment);
    let member_name = join_route_name(&context.prefixes, &member_segment);
    let member_args = context.required_args + usize::from(!singular_resource);

    if !singular_resource {
        insert_helper(helpers, collection_name, context.required_args);
    }
    insert_helper(helpers, member_name.clone(), member_args);
    insert_helper(helpers, format!("new_{member_name}"), context.required_args);
    insert_helper(helpers, format!("edit_{member_name}"), member_args);

    let resource = ResourceContext {
        outer_prefixes: context.prefixes.clone(),
        collection_segment,
        member_segment,
        collection_args: context.required_args,
        member_args,
    };
    collect_resource_block(call, parse_result, &resource, helpers);
}

fn collect_resource_block(
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
    resource: &ResourceContext,
    helpers: &mut std::collections::BTreeMap<String, usize>,
) {
    let Some(block_raw) = call.block() else {
        return;
    };
    let Some(block) = block_raw.as_block_node() else {
        return;
    };
    let Some(body) = block.body() else {
        return;
    };
    let Node::StatementsNode { .. } = &body else {
        return;
    };
    let statements = body.as_statements_node().expect("must be StatementsNode");
    for stmt in statements.body().iter() {
        collect_resource_stmt(&stmt, parse_result, resource, helpers);
    }
}

fn collect_resource_stmt(
    node: &Node<'_>,
    parse_result: &ParseResult<'_>,
    resource: &ResourceContext,
    helpers: &mut std::collections::BTreeMap<String, usize>,
) {
    let Node::CallNode { .. } = node else {
        return;
    };
    let call = node.as_call_node().expect("must be CallNode");
    let method_name = String::from_utf8_lossy(call.name().as_slice()).to_string();

    match method_name.as_str() {
        "member" => collect_resource_scoped_block(&call, parse_result, resource, true, helpers),
        "collection" => {
            collect_resource_scoped_block(&call, parse_result, resource, false, helpers)
        }
        "get" | "post" | "put" | "patch" | "delete" => {
            if let Some(scope) = extract_hash_option(&call, "on", parse_result) {
                let is_member = scope == "member";
                register_custom_resource_route(&call, parse_result, resource, is_member, helpers);
            } else {
                register_custom_resource_route(&call, parse_result, resource, true, helpers);
            }
        }
        _ => {
            let nested = RouteContext {
                prefixes: resource_nested_prefixes(resource),
                required_args: resource.member_args,
            };
            collect_route_helpers(node, parse_result, &nested, helpers);
        }
    }
}

fn collect_resource_scoped_block(
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
    resource: &ResourceContext,
    member_scope: bool,
    helpers: &mut std::collections::BTreeMap<String, usize>,
) {
    let Some(block_raw) = call.block() else {
        return;
    };
    let Some(block) = block_raw.as_block_node() else {
        return;
    };
    let Some(body) = block.body() else {
        return;
    };
    let Node::StatementsNode { .. } = &body else {
        return;
    };
    let statements = body.as_statements_node().expect("must be StatementsNode");
    for stmt in statements.body().iter() {
        if let Node::CallNode { .. } = &stmt {
            let stmt_call = stmt.as_call_node().expect("must be CallNode");
            let stmt_name = String::from_utf8_lossy(stmt_call.name().as_slice()).to_string();
            if matches!(
                stmt_name.as_str(),
                "get" | "post" | "put" | "patch" | "delete"
            ) {
                register_custom_resource_route(
                    &stmt_call,
                    parse_result,
                    resource,
                    member_scope,
                    helpers,
                );
            } else {
                let nested = if member_scope {
                    RouteContext {
                        prefixes: resource_nested_prefixes(resource),
                        required_args: resource.member_args,
                    }
                } else {
                    RouteContext {
                        prefixes: resource_collection_prefixes(resource),
                        required_args: resource.collection_args,
                    }
                };
                collect_route_helpers(&stmt, parse_result, &nested, helpers);
            }
        }
    }
}

fn resource_nested_prefixes(resource: &ResourceContext) -> Vec<String> {
    let mut prefixes = resource.outer_prefixes.clone();
    prefixes.push(resource.member_segment.clone());
    prefixes
}

fn resource_collection_prefixes(resource: &ResourceContext) -> Vec<String> {
    let mut prefixes = resource.outer_prefixes.clone();
    prefixes.push(resource.collection_segment.clone());
    prefixes
}

fn register_custom_resource_route(
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
    resource: &ResourceContext,
    member_scope: bool,
    helpers: &mut std::collections::BTreeMap<String, usize>,
) {
    let Some(helper_leaf) = extract_hash_option(call, "as", parse_result)
        .or_else(|| first_route_name(call, parse_result))
    else {
        return;
    };
    let mut parts = vec![helper_leaf];
    parts.extend(resource.outer_prefixes.iter().cloned());
    parts.push(if member_scope {
        resource.member_segment.clone()
    } else {
        resource.collection_segment.clone()
    });
    insert_helper(
        helpers,
        parts.join("_"),
        if member_scope {
            resource.member_args
        } else {
            resource.collection_args
        },
    );
}

fn register_simple_route(
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
    context: &RouteContext,
    helpers: &mut std::collections::BTreeMap<String, usize>,
) {
    if let Some(helper) = extract_hash_option(call, "as", parse_result)
        .or_else(|| first_route_name(call, parse_result))
    {
        let arg_count = route_segment_count(call, parse_result);
        insert_helper(
            helpers,
            join_route_name(&context.prefixes, &helper),
            context.required_args + arg_count,
        );
    }
}

fn route_segment_count(call: &ruby_prism::CallNode<'_>, parse_result: &ParseResult<'_>) -> usize {
    let Some(args) = call.arguments() else {
        return 0;
    };
    let Some(first) = args.arguments().iter().next() else {
        return 0;
    };
    let Some(path) = extract_string_or_symbol(&first, parse_result) else {
        return 0;
    };
    path.split('/')
        .filter(|segment| segment.starts_with(':') || segment.starts_with('*'))
        .count()
}

fn insert_helper(
    helpers: &mut std::collections::BTreeMap<String, usize>,
    name: String,
    required_args: usize,
) {
    helpers
        .entry(name)
        .and_modify(|current| *current = (*current).max(required_args))
        .or_insert(required_args);
}

fn collect_call_block(
    call: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
    context: &RouteContext,
    helpers: &mut std::collections::BTreeMap<String, usize>,
) {
    if let Some(block_raw) = call.block()
        && let Some(block) = block_raw.as_block_node()
        && let Some(body) = block.body()
        && let Node::StatementsNode { .. } = &body
    {
        let statements = body.as_statements_node().expect("must be StatementsNode");
        for stmt in statements.body().iter() {
            collect_route_helpers(&stmt, parse_result, context, helpers);
        }
    }
}

fn register_route_helper(registry: &mut TypeRegistry, helper: &RouteHelper) {
    registry.add_mixin("Object", ROUTE_HELPERS_MODULE, MixinKind::Include);
    for suffix in ["path", "url"] {
        registry.add_method_def_if_missing(
            ROUTE_HELPERS_MODULE,
            build_route_helper_method(&helper.name, suffix, helper.required_args),
        );
    }
}

fn build_route_helper_method(name: &str, suffix: &str, required_args: usize) -> MethodDef {
    MethodDef {
        name: Sym::new(format!("{name}_{suffix}")),
        param_infos: (0..required_args)
            .map(|idx| ParamInfo {
                name: format!("arg{idx}"),
                kind: ParamKind::Required,
                default_type: Some(Type::Untyped),
            })
            .collect(),
        raw_return_type: Type::String,
        sorbet_modifier_comments: Vec::new(),
        rbs_annotated: true,
        rbs_inline_annotated: false,
        sig_annotated: false,
        attr_ivar: None,
        is_singleton: false,
        rbs_file_source: true,
        synthetic_dsl_source: true,
        rbs_method_types: Default::default(),
        extra_overloads: Vec::new(),
        loc: None,
    }
}
