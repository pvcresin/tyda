use super::super::InferenceEngine;
use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;
use ruby_prism::{CallNode, ParseResult};

const CLASS_BODY_METHODS: &[&str] = &[
    "belongs_to",
    "has_many",
    "has_one",
    "has_and_belongs_to_many",
    "composed_of",
    "scope",
    "enum",
    "store_accessor",
    "store",
    "typed_store",
    "delegated_type",
    "connects_to",
    "encrypts",
    "has_encrypted",
    "normalizes",
    "normalize",
    "generates_token_for",
    "has_secure_token",
    "alias_attribute",
];

const STORE_LIBRARIES: &[DslLibrary] = &[
    DslLibrary::ActiveRecordStore,
    DslLibrary::ActiveRecordTypedStore,
];

pub(super) struct ActiveRecord;

static MANIFEST: PluginManifest = PluginManifest {
    id: "active_record",
    features: &[
        DslFeature {
            library: DslLibrary::ActiveRecordAssociations,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveRecordColumns,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveRecordDelegatedTypes,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveRecordEnum,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveRecordFixtures,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveRecordPersistence,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveRecordRelations,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveRecordScope,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveRecordSecureToken,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveRecordStore,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveRecordTypedStore,
            gem_markers: &[],
        },
    ],
    base_classes: &[],
    rails_default: true,
};

impl Plugin for ActiveRecord {
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
            "belongs_to" if cx.dsl_enabled(DslLibrary::ActiveRecordAssociations) => {
                cx.collect_belongs_to(class_name, call_node, parse_result);
                cx.collect_active_model_serializer_belongs_to(class_name, call_node, parse_result);
                true
            }
            "has_many" if cx.dsl_enabled(DslLibrary::ActiveRecordAssociations) => {
                cx.collect_has_many(class_name, call_node, parse_result);
                cx.collect_active_model_serializer_has_many(class_name, call_node, parse_result);
                true
            }
            "has_one" if cx.dsl_enabled(DslLibrary::ActiveRecordAssociations) => {
                cx.collect_has_one(class_name, call_node, parse_result);
                cx.collect_active_model_serializer_belongs_to(class_name, call_node, parse_result);
                true
            }
            "has_and_belongs_to_many" if cx.dsl_enabled(DslLibrary::ActiveRecordAssociations) => {
                cx.collect_has_and_belongs_to_many(class_name, call_node, parse_result);
                true
            }
            "composed_of" if cx.dsl_enabled(DslLibrary::ActiveRecordAssociations) => {
                cx.collect_composed_of(class_name, call_node, parse_result);
                true
            }
            "scope"
                if cx.dsl_enabled(DslLibrary::ActiveRecordScope)
                    || cx.dsl_enabled(DslLibrary::ActiveHash) =>
            {
                if cx.dsl_enabled(DslLibrary::ActiveHash)
                    && cx.is_active_hash_model_class(class_name)
                {
                    cx.collect_active_hash_scope_dsl(class_name, call_node, parse_result);
                } else {
                    cx.collect_scope_dsl(class_name, call_node, parse_result);
                }
                true
            }
            "enum" if cx.dsl_enabled(DslLibrary::ActiveRecordEnum) => {
                cx.collect_enum_dsl(class_name, call_node, parse_result);
                true
            }
            "store_accessor" if cx.any_dsl_enabled(STORE_LIBRARIES) => {
                cx.collect_store_accessor_dsl(class_name, call_node, parse_result);
                true
            }
            "store" if cx.any_dsl_enabled(STORE_LIBRARIES) => {
                cx.collect_store_dsl(class_name, call_node, parse_result);
                true
            }
            "typed_store" if cx.dsl_enabled(DslLibrary::ActiveRecordTypedStore) => {
                cx.collect_typed_store_dsl(class_name, call_node, parse_result);
                true
            }
            "delegated_type" if cx.dsl_enabled(DslLibrary::ActiveRecordDelegatedTypes) => {
                cx.collect_delegated_type_dsl(class_name, call_node, parse_result);
                true
            }
            "connects_to" if cx.rails_feature_enabled() => {
                cx.collect_connects_to(class_name, call_node, parse_result);
                true
            }
            "encrypts" | "has_encrypted" if cx.rails_feature_enabled() => {
                cx.collect_encrypted_attributes(class_name, call_node, parse_result);
                true
            }
            "normalizes" | "normalize" if cx.rails_feature_enabled() => {
                cx.collect_normalized_attributes(class_name, call_node, parse_result);
                true
            }
            "generates_token_for" if cx.rails_feature_enabled() => {
                cx.collect_generates_token_for(class_name, call_node, parse_result);
                true
            }
            "has_secure_token" if cx.dsl_enabled(DslLibrary::ActiveRecordSecureToken) => {
                cx.collect_secure_token_dsl(class_name, call_node, parse_result);
                true
            }
            "alias_attribute" if cx.rails_feature_enabled() => {
                cx.collect_alias_attribute(class_name, call_node, parse_result);
                true
            }
            _ => false,
        }
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        CLASS_BODY_METHODS
    }
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    if !engine.dsl_enabled(DslLibrary::ActiveRecordPersistence) {
        return None;
    }
    if let Some(elem_type) = InferenceEngine::active_record_relation_element_type(receiver_type) {
        return relation_method_return(receiver_type, &elem_type, method_name);
    }
    match receiver_type {
        Type::Class(name) if name.as_str() == "ActiveRecord::Batches::BatchEnumerator" => {
            batch_enumerator_method_return(method_name)
        }
        Type::Class(name) if engine.is_active_record_model_class(name) => {
            instance_method_return(receiver_type, method_name)
        }
        Type::Singleton(name) if engine.is_active_record_model_class(name) => {
            dynamic_finder_return(name.as_str(), method_name)
        }
        _ => None,
    }
}

fn batch_enumerator_method_return(method_name: &str) -> Option<Type> {
    match method_name {
        "update_all" | "delete_all" | "touch_all" => Some(Type::Integer),
        "destroy_all" | "each" | "each_record" | "relation" => Some(Type::Untyped),
        _ => None,
    }
}

fn instance_method_return(receiver_type: &Type, method_name: &str) -> Option<Type> {
    let changes_hash = || Type::Hash(Some(Box::new(Type::String)), Some(Box::new(Type::Untyped)));
    match method_name {
        "save" | "save!" | "update" | "update!" | "update_attribute" | "update_column"
        | "update_columns" | "touch" | "increment!" | "decrement!" | "toggle!" => Some(Type::Bool),
        "persisted?"
        | "new_record?"
        | "destroyed?"
        | "previously_new_record?"
        | "changed?"
        | "marked_for_destruction?"
        | "readonly?"
        | "has_changes_to_save?"
        | "saved_changes?" => Some(Type::Bool),
        "destroy" | "destroy!" | "delete" | "reload" | "lock!" | "increment" | "decrement"
        | "toggle" | "tap_reload" => Some(receiver_type.clone()),
        "assign_attributes" | "attributes=" => Some(Type::Nil),
        "attributes" | "saved_changes" | "previous_changes" | "changes" | "changes_to_save" => {
            Some(changes_hash())
        }
        "becomes"
        | "becomes!"
        | "with_lock"
        | "mark_for_destruction"
        | "restore_attributes"
        | "attribute_before_last_save"
        | "run_callbacks"
        | "connection"
        | "transaction"
        | "read_attribute"
        | "write_attribute"
        | "logger"
        | "attribute_was" => Some(Type::Untyped),
        _ => None,
    }
}

fn dynamic_finder_return(model_name: &str, method_name: &str) -> Option<Type> {
    let model = || Type::Class(crate::types::Sym::new(model_name));
    if matches!(
        method_name,
        "connection"
            | "with_connection"
            | "lease_connection"
            | "release_connection"
            | "transaction"
            | "connected_to"
            | "connected_to_many"
            | "connecting_to"
            | "connection_pool"
            | "connection_handler"
            | "connection_db_config"
            | "connection_specification_name"
            | "establish_connection"
            | "remove_connection"
            | "logger"
            | "benchmark"
    ) {
        return Some(Type::Untyped);
    }
    if matches!(
        method_name,
        "sanitize_sql"
            | "sanitize_sql_like"
            | "sanitize_sql_for_conditions"
            | "sanitize_sql_for_assignment"
            | "sanitize_sql_array"
    ) {
        return Some(Type::String);
    }
    if matches!(
        method_name,
        "table_name" | "primary_key" | "quoted_table_name"
    ) {
        return Some(Type::String);
    }
    if matches!(
        method_name,
        "from_union" | "union" | "safe_find_or_create_by"
    ) {
        return Some(InferenceEngine::active_record_relation_type(model_name));
    }
    if method_name == "find_by_sql" {
        return Some(Type::Array(Some(Box::new(model()))));
    }
    if let Some(rest) = method_name.strip_prefix("find_all_by_")
        && !rest.is_empty()
    {
        return Some(Type::Array(Some(Box::new(model()))));
    }
    if let Some(rest) = method_name
        .strip_prefix("find_or_create_by_")
        .or_else(|| method_name.strip_prefix("find_or_initialize_by_"))
        && !rest.is_empty()
    {
        return Some(model());
    }
    if let Some(rest) = method_name.strip_prefix("find_by_") {
        if rest.is_empty() {
            return None;
        }
        if rest.ends_with('!') {
            return Some(model());
        }
        return Some(Type::Union(vec![model(), Type::Nil]));
    }
    None
}

fn relation_method_return(
    receiver_type: &Type,
    elem_type: &Type,
    method_name: &str,
) -> Option<Type> {
    match method_name {
        "update_all" | "delete_all" | "touch_all" | "delete" | "destroy" | "in_order_of_count" => {
            Some(Type::Integer)
        }
        "destroy_all" => Some(Type::Array(Some(Box::new(elem_type.clone())))),
        "merge!" | "<<" | "push" | "append" | "concat" | "load" | "reset" | "prepend" => {
            Some(receiver_type.clone())
        }
        "insert" | "insert!" | "insert_all" | "insert_all!" | "upsert" | "upsert_all"
        | "replace" | "scoping" | "explain" | "calculate" | "each_batch" => Some(Type::Untyped),
        _ => None,
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_secure_token_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.rails_at_least(5, 0) {
            return;
        }
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let attr_name = self
            .first_symbol_or_string_arg(call_node)
            .unwrap_or_else(|| "token".to_string());
        self.add_simple_method_if_missing(
            class_name,
            &format!("regenerate_{attr_name}"),
            Type::Bool,
            false,
            loc,
        );
    }

    pub(in crate::inference) fn collect_store_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        for name in self.hash_option_names(call_node, "accessors", parse_result) {
            self.add_accessor_methods(class_name, &name, Type::Untyped, false, loc);
        }
    }

    pub(in crate::inference) fn collect_typed_store_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        for name in self.hash_option_names(call_node, "accessors", parse_result) {
            self.add_accessor_methods(class_name, &name, Type::Untyped, false, loc);
        }

        if let Some(block_raw) = call_node.block()
            && let Some(block) = block_raw.as_block_node()
            && let Some(body) = block.body()
            && let Node::StatementsNode { .. } = &body
        {
            let statements = body.as_statements_node().expect("must be StatementsNode");
            for stmt in statements.body().iter() {
                if let Node::CallNode { .. } = &stmt {
                    let inner = stmt.as_call_node().expect("must be CallNode");
                    for name in self.symbol_or_string_args(&inner) {
                        self.add_accessor_methods(class_name, &name, Type::Untyped, false, loc);
                    }
                }
            }
        }
    }
}

impl<'a> InferenceEngine<'a> {
    pub(super) fn collect_alias_attribute(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let args = Self::extract_symbol_args(call_node);
        if args.len() < 2 {
            return;
        }
        let new_name = &args[0];
        let old_name = &args[1];
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let alias_type = self
            .registry
            .lookup_method_sig(class_name, old_name)
            .map(|sig| sig.return_type)
            .or_else(|| self.infer_alias_attribute_type(class_name, old_name));
        let getter_type = alias_type
            .clone()
            .unwrap_or_else(|| Type::MethodReturnRef(class_name.into(), old_name.clone().into()));
        let setter_type = alias_type.clone().unwrap_or_else(|| {
            Type::MethodReturnRef(Sym::new(class_name), format!("{old_name}=").into())
        });

        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(new_name),
                param_infos: Vec::new(),
                raw_return_type: getter_type.clone(),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(format!("{new_name}=")),
                param_infos: vec![ParamInfo {
                    name: new_name.clone(),
                    kind: ParamKind::Required,
                    default_type: Some(getter_type),
                }],
                raw_return_type: setter_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
    }
    pub(super) fn collect_belongs_to(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if self.is_active_model_serializer_class(class_name) {
            return;
        }
        let names = Self::extract_symbol_args(call_node);
        let Some(assoc_name) = names.first() else {
            return;
        };
        let mut options = Self::extract_association_options(call_node, parse_result);
        self.apply_with_options_fallback(&mut options);
        let target_class =
            self.infer_association_target_class(class_name, assoc_name, &options, false);
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.ensure_active_record_query_methods(loc);

        if options.polymorphic {
            let ret_type = if options.optional {
                Type::Union(vec![Type::Untyped, Type::Nil])
            } else {
                Type::Untyped
            };
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(assoc_name),
                    param_infos: Vec::new(),
                    raw_return_type: ret_type,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(format!("{assoc_name}=")),
                    param_infos: vec![ParamInfo {
                        name: assoc_name.clone(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::Untyped),
                    }],
                    raw_return_type: Type::Untyped,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
            for suffix_name in [format!("{assoc_name}_type"), format!("{assoc_name}_id")] {
                let ret = if suffix_name.ends_with("_type") {
                    Type::String
                } else {
                    Type::Integer
                };
                self.registry.add_method_def(
                    class_name,
                    MethodDef {
                        name: Sym::new(suffix_name),
                        param_infos: Vec::new(),
                        raw_return_type: ret,
                        sorbet_modifier_comments: Vec::new(),
                        rbs_annotated: true,
                        rbs_inline_annotated: false,
                        sig_annotated: false,
                        attr_ivar: None,
                        is_singleton: false,
                        rbs_file_source: false,
                        synthetic_dsl_source: false,
                        rbs_method_types: Default::default(),
                        extra_overloads: Vec::new(),
                        loc: Some(loc),
                    },
                );
            }
            return;
        }

        let ret_type = if options.optional {
            Type::Union(vec![Type::Class(Sym::new(&target_class)), Type::Nil])
        } else {
            Type::Class(Sym::new(&target_class))
        };
        self.record_reference(&target_class);

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(assoc_name),
                param_infos: Vec::new(),
                raw_return_type: ret_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(format!("{assoc_name}=")),
                param_infos: vec![ParamInfo {
                    name: assoc_name.clone(),
                    kind: ParamKind::Required,
                    default_type: Some(Type::Class(Sym::new(&target_class))),
                }],
                raw_return_type: Type::Class(Sym::new(&target_class)),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        for prefix in ["build_", "create_"] {
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(format!("{prefix}{assoc_name}")),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Class(Sym::new(&target_class)),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }
    }

    pub(super) fn collect_has_many(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if self.is_active_model_serializer_class(class_name) {
            return;
        }
        let names = Self::extract_symbol_args(call_node);
        let Some(assoc_name) = names.first() else {
            return;
        };
        let mut options = Self::extract_association_options(call_node, parse_result);
        self.apply_with_options_fallback(&mut options);
        let target_class =
            self.infer_association_target_class(class_name, assoc_name, &options, true);
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.ensure_active_record_query_methods(loc);
        self.record_reference(&target_class);
        let singular = self.singularize_association_name(assoc_name);
        let collection_type = Self::active_record_collection_proxy_type(&target_class);

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(assoc_name),
                param_infos: Vec::new(),
                raw_return_type: collection_type.clone(),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(format!("{singular}_ids")),
                param_infos: Vec::new(),
                raw_return_type: Type::Array(Some(Box::new(Type::Integer))),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(format!("{singular}_ids=")),
                param_infos: vec![ParamInfo {
                    name: format!("{singular}_ids"),
                    kind: ParamKind::Required,
                    default_type: Some(Type::Array(Some(Box::new(Type::Integer)))),
                }],
                raw_return_type: Type::Array(Some(Box::new(Type::Integer))),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(format!("{assoc_name}=")),
                param_infos: vec![ParamInfo {
                    name: assoc_name.clone(),
                    kind: ParamKind::Required,
                    default_type: Some(Type::Array(Some(Box::new(Type::Class(Sym::new(
                        target_class.clone(),
                    )))))),
                }],
                raw_return_type: collection_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
    }

    pub(super) fn collect_has_and_belongs_to_many(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        self.collect_has_many(class_name, call_node, parse_result);
    }

    pub(super) fn collect_has_one(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if self.is_active_model_serializer_class(class_name) {
            return;
        }
        let names = Self::extract_symbol_args(call_node);
        let Some(assoc_name) = names.first() else {
            return;
        };
        let mut options = Self::extract_association_options(call_node, parse_result);
        self.apply_with_options_fallback(&mut options);
        let target_class =
            self.infer_association_target_class(class_name, assoc_name, &options, false);
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.ensure_active_record_query_methods(loc);
        self.record_reference(&target_class);

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(assoc_name),
                param_infos: Vec::new(),
                raw_return_type: Type::Union(vec![Type::Class(Sym::new(&target_class)), Type::Nil]),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(format!("{assoc_name}=")),
                param_infos: vec![ParamInfo {
                    name: assoc_name.clone(),
                    kind: ParamKind::Required,
                    default_type: Some(Type::Class(Sym::new(&target_class))),
                }],
                raw_return_type: Type::Class(Sym::new(&target_class)),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        for prefix in ["build_", "create_"] {
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(format!("{prefix}{assoc_name}")),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Class(Sym::new(&target_class)),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }
    }

    pub(super) fn collect_scope_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let names = Self::extract_symbol_args(call_node);
        let Some(scope_name) = names.first() else {
            return;
        };
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.ensure_active_record_query_methods(loc);
        let relation_type = self.owner_relation_type_for_dsl(class_name);
        let param_infos = call_node
            .arguments()
            .and_then(|args| {
                args.arguments()
                    .iter()
                    .find(|arg| matches!(arg, Node::LambdaNode { .. }))
            })
            .map(|node| self.extract_scope_param_infos_from_node(&node))
            .or_else(|| {
                call_node
                    .block()
                    .map(|block| self.extract_scope_param_infos_from_node(&block))
            })
            .unwrap_or_default();
        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(scope_name),
                param_infos,
                raw_return_type: relation_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: true,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
    }

    pub(super) fn collect_composed_of(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let names = Self::extract_symbol_args(call_node);
        let Some(attr_name) = names.first() else {
            return;
        };
        let target_class = Self::extract_hash_option_str(call_node, "class_name", parse_result)
            .unwrap_or_else(|| Self::camelize_attr_name(attr_name));
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let value_type = Type::Class(Sym::new(&target_class));
        self.record_reference(&target_class);

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(attr_name),
                param_infos: Vec::new(),
                raw_return_type: value_type.clone(),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(format!("{attr_name}=")),
                param_infos: vec![ParamInfo {
                    name: attr_name.clone(),
                    kind: ParamKind::Required,
                    default_type: Some(value_type.clone()),
                }],
                raw_return_type: value_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
    }

    fn camelize_attr_name(name: &str) -> String {
        name.split('_')
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(c) => {
                        let upper: String = c.to_uppercase().collect();
                        format!("{upper}{}", chars.as_str())
                    }
                    None => String::new(),
                }
            })
            .collect()
    }

    pub(super) fn collect_enum_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let value_names = Self::extract_enum_value_names(call_node, parse_result);
        let enum_attr_name = Self::extract_enum_attribute_name(call_node, parse_result);
        let prefix = enum_attr_name
            .as_deref()
            .and_then(|name| Self::enum_method_affix(call_node, parse_result, "prefix", name));
        let suffix = enum_attr_name
            .as_deref()
            .and_then(|name| Self::enum_method_affix(call_node, parse_result, "suffix", name));
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());

        for val in &value_names {
            let method_base =
                Self::decorate_enum_method_name(val, prefix.as_deref(), suffix.as_deref());
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(format!("{method_base}?")),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Bool,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(format!("{method_base}!")),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Bool,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
            let scope_return = self.owner_relation_type_for_dsl(class_name);
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(method_base),
                    param_infos: Vec::new(),
                    raw_return_type: scope_return,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }
    }

    pub(super) fn collect_store_accessor_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.rails_at_least(5, 0) || !self.is_active_record_dsl_target(class_name) {
            return;
        }
        let names = Self::extract_symbol_args(call_node);
        if names.len() <= 1 {
            return;
        }
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.with_concern_included_synthetic_marking(class_name, |engine| {
            for attr_name in names.iter().skip(1) {
                engine.register_untyped_attribute_accessors(class_name, attr_name, loc);
            }
        });
    }

    pub(super) fn collect_delegated_type_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.rails_at_least(6, 1) || !self.is_active_record_dsl_target(class_name) {
            return;
        }
        let names = Self::extract_symbol_args(call_node);
        let Some(assoc_name) = names.first() else {
            return;
        };
        let target_types = Self::extract_hash_option_names(call_node, "types", parse_result);
        if target_types.is_empty() {
            return;
        }

        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.ensure_active_record_query_methods(loc);

        let assoc_union = Type::from_type_vec(
            target_types
                .iter()
                .map(|name| Type::Class(Sym::new(name)))
                .collect(),
        );
        let owner_relation = self.owner_relation_type_for_dsl(class_name);
        let synthetic_start = self
            .is_collecting_concern_included()
            .then(|| self.registry.method_defs_len(class_name));

        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(assoc_name),
                param_infos: Vec::new(),
                raw_return_type: assoc_union.clone(),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(format!("{assoc_name}=")),
                param_infos: vec![ParamInfo {
                    name: assoc_name.clone(),
                    kind: ParamKind::Required,
                    default_type: Some(assoc_union.clone()),
                }],
                raw_return_type: assoc_union.clone(),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(format!("{assoc_name}_class")),
                param_infos: Vec::new(),
                raw_return_type: Type::Class(Sym::new("Class")),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
        self.registry.add_method_def(
            class_name,
            MethodDef {
                name: Sym::new(format!("{assoc_name}_name")),
                param_infos: Vec::new(),
                raw_return_type: Type::String,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        for type_name in &target_types {
            self.record_reference(type_name);
            let method_base = Self::snake_case_type_name(type_name);
            let scope_name = Self::pluralize_name(&method_base);
            let specific_optional = Type::Union(vec![Type::Class(Sym::new(type_name)), Type::Nil]);

            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(format!("{method_base}?")),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Bool,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(&method_base),
                    param_infos: Vec::new(),
                    raw_return_type: specific_optional,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(format!("{method_base}_id")),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Union(vec![Type::Integer, Type::Nil]),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(scope_name),
                    param_infos: Vec::new(),
                    raw_return_type: owner_relation.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        if let Some(start) = synthetic_start {
            self.registry
                .mark_methods_synthetic_dsl_from(class_name, start);
        }
    }

    pub(super) fn collect_connects_to(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.rails_at_least(6, 0) || !self.is_active_record_dsl_target(class_name) {
            return;
        }
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let synthetic_start = self
            .is_collecting_concern_included()
            .then(|| self.registry.method_defs_len(class_name));
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("connects_to"),
                param_infos: vec![ParamInfo {
                    name: "database".to_string(),
                    kind: ParamKind::Required,
                    default_type: Some(Type::Untyped),
                }],
                raw_return_type: Type::Void,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: true,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
        if let Some(start) = synthetic_start {
            self.registry
                .mark_methods_synthetic_dsl_from(class_name, start);
        }
    }

    pub(super) fn collect_encrypted_attributes(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.rails_at_least(7, 0) || !self.is_active_record_dsl_target(class_name) {
            return;
        }
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        self.with_concern_included_synthetic_marking(class_name, |engine| {
            for attr_name in Self::extract_symbol_args(call_node) {
                engine.register_untyped_attribute_accessors(class_name, &attr_name, loc);
            }
        });
    }

    pub(super) fn collect_normalized_attributes(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.rails_at_least(7, 1) || !self.is_active_record_dsl_target(class_name) {
            return;
        }
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let synthetic_start = self
            .is_collecting_concern_included()
            .then(|| self.registry.method_defs_len(class_name));
        for attr_name in Self::extract_symbol_args(call_node) {
            self.register_untyped_attribute_accessors(class_name, &attr_name, loc);
        }
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("normalize_value_for"),
                param_infos: vec![
                    ParamInfo {
                        name: "name".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::Symbol),
                    },
                    ParamInfo {
                        name: "value".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::Untyped),
                    },
                ],
                raw_return_type: Type::ParamRef(1),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: true,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
        if let Some(start) = synthetic_start {
            self.registry
                .mark_methods_synthetic_dsl_from(class_name, start);
        }
    }

    pub(super) fn collect_generates_token_for(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.rails_at_least(7, 1) || !self.is_active_record_dsl_target(class_name) {
            return;
        }
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let synthetic_start = self
            .is_collecting_concern_included()
            .then(|| self.registry.method_defs_len(class_name));
        let owner_instance = if self.is_collecting_concern_included() {
            Type::InstanceType
        } else {
            Type::Class(Sym::new(class_name))
        };
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("generate_token_for"),
                param_infos: vec![ParamInfo {
                    name: "purpose".to_string(),
                    kind: ParamKind::Required,
                    default_type: Some(Type::Symbol),
                }],
                raw_return_type: Type::String,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("find_by_token_for"),
                param_infos: vec![
                    ParamInfo {
                        name: "purpose".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::Symbol),
                    },
                    ParamInfo {
                        name: "token".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::String),
                    },
                ],
                raw_return_type: Type::Union(vec![owner_instance.clone(), Type::Nil]),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: true,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("find_by_token_for!"),
                param_infos: vec![
                    ParamInfo {
                        name: "purpose".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::Symbol),
                    },
                    ParamInfo {
                        name: "token".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::String),
                    },
                ],
                raw_return_type: owner_instance,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: true,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
        if let Some(start) = synthetic_start {
            self.registry
                .mark_methods_synthetic_dsl_from(class_name, start);
        }
    }
}
