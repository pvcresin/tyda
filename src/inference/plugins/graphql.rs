//! ! GraphQL-Ruby (the `graphql` gem): schema / type definition DSL.
//! ! ! Two receiver shapes matter: ! - Schema classes (`< GraphQL::Schema`): class-body singleton DSL such as !   `use`, `query`, `lazy_resolve`, `max_complexity`.

use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;
use ruby_prism::{CallNode, ParseResult};

const ARGUMENT_LIBRARIES: &[DslLibrary] = &[
    DslLibrary::RailsGenerators,
    DslLibrary::GraphqlInputObject,
    DslLibrary::GraphqlMutation,
];

const FIELD_LIBRARIES: &[DslLibrary] =
    &[DslLibrary::GraphqlMutation, DslLibrary::GraphqlInputObject];

pub(super) struct Graphql;

static MANIFEST: PluginManifest = PluginManifest {
    id: "graphql",
    features: &[
        DslFeature {
            library: DslLibrary::GraphqlSchema,
            gem_markers: &["graphql", "graphql-ruby"],
        },
        DslFeature {
            library: DslLibrary::GraphqlInputObject,
            gem_markers: &["graphql", "graphql-ruby"],
        },
        DslFeature {
            library: DslLibrary::GraphqlMutation,
            gem_markers: &["graphql", "graphql-ruby"],
        },
    ],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Graphql {
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
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
        _comments: &[super::RbsComment],
    ) -> bool {
        match method_name {
            "argument" | "class_option" if cx.any_dsl_enabled(ARGUMENT_LIBRARIES) => {
                cx.collect_argument_like_dsl(class_name, call_node, method_name, parse_result);
                true
            }
            "field" if cx.any_dsl_enabled(FIELD_LIBRARIES) => {
                cx.collect_field_like_dsl(class_name, call_node, method_name, parse_result);
                true
            }
            _ => false,
        }
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        &["argument", "class_option", "field"]
    }
}

const SCHEMA_BASES: &[&str] = &["GraphQL::Schema"];

const TYPE_MEMBER_BASES: &[&str] = &[
    "GraphQL::Schema::Object",
    "GraphQL::Schema::Interface",
    "GraphQL::Schema::Enum",
    "GraphQL::Schema::Union",
    "GraphQL::Schema::InputObject",
    "GraphQL::Schema::Scalar",
    "GraphQL::Schema::Mutation",
    "GraphQL::Schema::RelayClassicMutation",
    "GraphQL::Schema::Resolver",
    "GraphQL::Schema::Subscription",
    "GraphQL::Schema::Member",
];

const SCHEMA_DSL_METHODS: &[&str] = &[
    "use",
    "query",
    "mutation",
    "subscription",
    "orphan_types",
    "rescue_from",
    "lazy_resolve",
    "instrument",
    "tracer",
    "trace_with",
    "query_analyzer",
    "multiplex_analyzer",
    "default_max_page_size",
    "default_page_size",
    "max_complexity",
    "max_depth",
    "max_query_string_tokens",
    "validate_timeout",
    "validate_max_errors",
    "disable_introspection_entry_points",
    "disable_schema_introspection_entry_point",
    "disable_type_introspection_entry_point",
    "context_class",
    "cursor_encoder",
    "directive",
    "connections",
    "default_logger",
];

const TYPE_MEMBER_DSL_METHODS: &[&str] = &[
    "connection_type",
    "edge_type",
    "connection_type_class",
    "edge_type_class",
    "graphql_name",
    "global_id_field",
    "implements",
    "authorize",
    "field_class",
    "argument_class",
    "type_expr",
    "default_graphql_name",
    "wrap",
    "scope_items",
];

const TYPE_MEMBER_INSTANCE_METHODS: &[&str] = &["object", "context"];

const GRAPHQL_LIBRARIES: &[DslLibrary] = &[
    DslLibrary::GraphqlSchema,
    DslLibrary::GraphqlInputObject,
    DslLibrary::GraphqlMutation,
];

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let (class_name, is_singleton) = match receiver_type {
        Type::Class(name) => (name.as_str(), false),
        Type::Singleton(name) => (name.as_str(), true),
        _ => return None,
    };
    if !engine.any_dsl_enabled(GRAPHQL_LIBRARIES) {
        return None;
    }
    if is_singleton {
        if SCHEMA_DSL_METHODS.contains(&method_name)
            && engine.class_matches_or_inherits(class_name, SCHEMA_BASES)
        {
            return Some(Type::Untyped);
        }
        if TYPE_MEMBER_DSL_METHODS.contains(&method_name)
            && engine.class_matches_or_inherits(class_name, TYPE_MEMBER_BASES)
        {
            return Some(Type::Untyped);
        }
        return None;
    }
    if TYPE_MEMBER_INSTANCE_METHODS.contains(&method_name)
        && engine.class_matches_or_inherits(class_name, TYPE_MEMBER_BASES)
    {
        return Some(Type::Untyped);
    }
    None
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_argument_like_dsl(
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
        self.add_accessor_methods(class_name, &name, Type::Untyped, false, loc);
    }

    pub(in crate::inference) fn collect_field_like_dsl(
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
        self.add_simple_method_if_missing(class_name, &name, Type::Untyped, false, loc);
    }
}
