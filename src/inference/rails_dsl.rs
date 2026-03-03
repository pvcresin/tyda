use super::*;
use crate::types::{SharedName, Sym};

const ACTIVE_RECORD_RELATION_CLASS: &str = "ActiveRecord::Relation";
const ACTIVE_RECORD_COLLECTION_PROXY_CLASS: &str = "ActiveRecord::Associations::CollectionProxy";
const ACTIVE_RECORD_RELATION_METHODS: &[&str] = &[
    "all",
    "none",
    "where",
    "eager_load",
    "preload",
    "references",
    "reselect",
    "reorder",
    "order",
    "joins",
    "includes",
    "left_outer_joins",
    "group",
    "unscope",
    "rewhere",
    "and",
    "or",
    "having",
    "limit",
    "offset",
    "lock",
    "readonly",
    "create_with",
    "from",
    "distinct",
    "extending",
    "optimizer_hints",
    "reverse_order",
    "annotate",
    "select",
    "merge",
];
const ACTIVE_RECORD_RELATION_METHODS_RAILS_6_1: &[&str] = &["strict_loading"];
const ACTIVE_RECORD_RELATION_METHODS_RAILS_7_0: &[&str] =
    &["with", "in_order_of", "invert_where", "excluding"];
const ACTIVE_RECORD_RELATION_METHODS_RAILS_7_1: &[&str] = &["with_recursive", "regroup"];
const ACTIVE_RECORD_OPTIONAL_FINDER_METHODS: &[&str] = &["find_by", "first", "last", "take"];
const ACTIVE_RECORD_REQUIRED_FINDER_METHODS: &[&str] =
    &["find_by!", "first!", "last!", "take!", "sole"];
const ACTIVE_RECORD_POSITIONAL_OPTIONAL_FINDER_METHODS: &[&str] = &[
    "second",
    "third",
    "fourth",
    "fifth",
    "forty_two",
    "third_to_last",
    "second_to_last",
];
const ACTIVE_RECORD_POSITIONAL_REQUIRED_FINDER_METHODS: &[&str] = &[
    "second!",
    "third!",
    "fourth!",
    "fifth!",
    "forty_two!",
    "third_to_last!",
    "second_to_last!",
];
const ACTIVE_RECORD_FIND_BY_ATTRS_METHODS: &[&str] = &["find_sole_by"];
const ACTIVE_RECORD_FIND_OR_CREATE_METHODS: &[&str] = &[
    "find_or_create_by",
    "find_or_create_by!",
    "find_or_initialize_by",
    "create_or_find_by",
    "create_or_find_by!",
];
const ACTIVE_RECORD_BUILDER_METHODS: &[&str] = &["new", "build", "create", "create!"];
const ACTIVE_RECORD_ENUMERABLE_QUERY_METHODS: &[&str] = &["any?", "many?", "none?", "one?"];
const ACTIVE_RECORD_CALCULATION_METHODS: &[&str] = &[
    "count", "ids", "pick", "pluck", "sum", "average", "minimum", "maximum",
];
const ACTIVE_RECORD_BOOL_RELATION_METHODS: &[&str] = &["exists?", "include?"];
const ACTIVE_RECORD_WHERE_CHAIN_METHODS: &[&str] = &["not", "associated", "missing"];
const ACTIVE_RECORD_BATCH_METHODS: &[&str] = &["find_each", "find_in_batches", "in_batches"];

#[derive(Clone, Default)]
pub(super) struct AssociationOptions {
    pub(super) class_name: Option<String>,
    pub(super) optional: bool,
    pub(super) polymorphic: bool,
    pub(super) through: Option<String>,
    pub(super) source: Option<String>,
    pub(super) inverse_of: Option<String>,
    pub(super) delegate_to: Option<String>,
    pub(super) delegate_allow_nil: bool,
    pub(super) delegate_prefix: Option<String>,
    pub(super) delegate_prefix_target: bool,
}

impl<'a> InferenceEngine<'a> {
    fn active_record_model_type(class_name: &str) -> Type {
        Type::Class(Sym::new(class_name))
    }

    fn active_record_optional_model_type(class_name: &str) -> Type {
        Type::Union(vec![Self::active_record_model_type(class_name), Type::Nil])
    }

    fn active_record_batch_method_return(class_name: &str, method_name: &str) -> Option<Type> {
        match method_name {
            "find_each" => Some(Type::Generic {
                base: Sym::new("Enumerator"),
                args: vec![Type::Class(Sym::new(class_name))].into(),
            }),
            "find_in_batches" => Some(Type::Generic {
                base: Sym::new("Enumerator"),
                args: vec![Type::Array(Some(Box::new(Type::Class(Sym::new(
                    class_name,
                )))))]
                .into(),
            }),
            "in_batches" => Some(Type::Class(Sym::new(
                "ActiveRecord::Batches::BatchEnumerator",
            ))),
            _ => None,
        }
    }

    fn active_record_common_method_return(class_name: &str, method_name: &str) -> Option<Type> {
        match method_name {
            "any?" | "many?" | "none?" | "one?" => Some(Type::Bool),
            "count" => Some(Type::Integer),
            "ids" => Some(Type::Array(Some(Box::new(Type::Integer)))),
            "pluck" => Some(Type::Array(Some(Box::new(Type::Untyped)))),
            "pick" | "sum" | "average" | "minimum" | "maximum" => Some(Type::Untyped),
            "find_by" | "first" | "last" | "take" => {
                Some(Self::active_record_optional_model_type(class_name))
            }
            "second" | "third" | "fourth" | "fifth" | "forty_two" | "third_to_last"
            | "second_to_last" => Some(Self::active_record_optional_model_type(class_name)),
            "find_by!"
            | "first!"
            | "last!"
            | "take!"
            | "sole"
            | "second!"
            | "third!"
            | "fourth!"
            | "fifth!"
            | "forty_two!"
            | "third_to_last!"
            | "second_to_last!"
            | "find_sole_by"
            | "find_or_create_by"
            | "find_or_create_by!"
            | "find_or_initialize_by"
            | "create_or_find_by"
            | "create_or_find_by!"
            | "new"
            | "build"
            | "create"
            | "create!"
            | "find" => Some(Self::active_record_model_type(class_name)),
            "exists?" | "include?" => Some(Type::Bool),
            "find_each" | "find_in_batches" | "in_batches" => {
                Self::active_record_batch_method_return(class_name, method_name)
            }
            _ => None,
        }
    }

    pub(super) fn try_resolve_controller_respond_to(
        &self,
        class_name: &str,
        method_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
    ) -> Option<Type> {
        // `respond_to do |format| ... end` is a hot path in large Rails controllers.
        // Keep this fast path intentionally narrow so we do not change behavior for non-Rails code, controller-like names, or apps that override `respond_to`.
        if self.action_controller_helpers_enabled()
            && method_name == "respond_to"
            && call_node.receiver().is_none()
            && call_node.block().is_some()
            && self.should_fast_path_controller_respond_to(class_name)
        {
            return Some(Type::Void);
        }
        None
    }

    fn should_fast_path_controller_respond_to(&self, class_name: &str) -> bool {
        self.class_matches_or_inherits(
            class_name,
            &[
                "ActionController::Base",
                "ActionController::API",
                "ApplicationController",
            ],
        ) && !self
            .registry
            .get_methods(class_name)
            .iter()
            .any(|method| method.name == "respond_to" && !method.rbs_file_source)
    }

    pub(super) fn register_rails_framework_methods(
        &mut self,
        class_name: &str,
        loc: SourceLocation,
    ) {
        if !self.rails_feature_enabled() {
            return;
        }

        if self.is_action_controller_class(class_name) {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new("params"),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Class(Sym::new("ActionController::Parameters")),
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
                    loc: Some(loc),
                },
            );
            for void_method in super::method_tables::VOID_TERMINAL_METHODS {
                self.registry.add_method_def_if_missing(
                    class_name,
                    MethodDef {
                        name: Sym::new(void_method),
                        param_infos: Vec::new(),
                        raw_return_type: Type::Void,
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
                        loc: Some(loc),
                    },
                );
            }

            if self.rails_at_least(7, 0) {
                for name in ["async", "load_async"] {
                    self.registry.add_method_def_if_missing(
                        class_name,
                        MethodDef {
                            name: Sym::new(name),
                            param_infos: Vec::new(),
                            raw_return_type: Type::SelfType,
                            sorbet_modifier_comments: Vec::new(),
                            rbs_annotated: true,
                            rbs_inline_annotated: false,
                            sig_annotated: false,
                            attr_ivar: None,
                            is_singleton: false,
                            rbs_file_source: true,
                            synthetic_dsl_source: false,
                            rbs_method_types: Default::default(),
                            extra_overloads: Vec::new(),
                            loc: Some(loc),
                        },
                    );
                }
            }

            if self.rails_at_least(7, 1) {
                for (name, _param_names, param_infos, return_type) in [
                    (
                        "async_count",
                        Vec::new(),
                        Vec::new(),
                        Type::Generic {
                            base: Sym::new("ActiveRecord::Promise"),
                            args: vec![Type::Integer].into(),
                        },
                    ),
                    (
                        "async_ids",
                        Vec::new(),
                        Vec::new(),
                        Type::Generic {
                            base: Sym::new("ActiveRecord::Promise"),
                            args: vec![Type::Array(Some(Box::new(Type::Integer)))].into(),
                        },
                    ),
                    (
                        "async_pick",
                        vec!["columns".to_string()],
                        vec![ParamInfo {
                            name: "columns".to_string(),
                            kind: ParamKind::Rest,
                            default_type: None,
                        }],
                        Type::Generic {
                            base: Sym::new("ActiveRecord::Promise"),
                            args: vec![Type::Untyped].into(),
                        },
                    ),
                    (
                        "async_pluck",
                        vec!["columns".to_string()],
                        vec![ParamInfo {
                            name: "columns".to_string(),
                            kind: ParamKind::Rest,
                            default_type: None,
                        }],
                        Type::Generic {
                            base: Sym::new("ActiveRecord::Promise"),
                            args: vec![Type::Untyped].into(),
                        },
                    ),
                    (
                        "async_sum",
                        vec!["column".to_string()],
                        vec![ParamInfo {
                            name: "column".to_string(),
                            kind: ParamKind::Required,
                            default_type: Some(Type::Untyped),
                        }],
                        Type::Generic {
                            base: Sym::new("ActiveRecord::Promise"),
                            args: vec![Type::Untyped].into(),
                        },
                    ),
                    (
                        "async_average",
                        vec!["column".to_string()],
                        vec![ParamInfo {
                            name: "column".to_string(),
                            kind: ParamKind::Required,
                            default_type: Some(Type::Untyped),
                        }],
                        Type::Generic {
                            base: Sym::new("ActiveRecord::Promise"),
                            args: vec![Type::Untyped].into(),
                        },
                    ),
                    (
                        "async_minimum",
                        vec!["column".to_string()],
                        vec![ParamInfo {
                            name: "column".to_string(),
                            kind: ParamKind::Required,
                            default_type: Some(Type::Untyped),
                        }],
                        Type::Generic {
                            base: Sym::new("ActiveRecord::Promise"),
                            args: vec![Type::Untyped].into(),
                        },
                    ),
                    (
                        "async_maximum",
                        vec!["column".to_string()],
                        vec![ParamInfo {
                            name: "column".to_string(),
                            kind: ParamKind::Required,
                            default_type: Some(Type::Untyped),
                        }],
                        Type::Generic {
                            base: Sym::new("ActiveRecord::Promise"),
                            args: vec![Type::Untyped].into(),
                        },
                    ),
                ] {
                    self.registry.add_method_def_if_missing(
                        class_name,
                        MethodDef {
                            name: Sym::new(name),
                            param_infos,
                            raw_return_type: return_type,
                            sorbet_modifier_comments: Vec::new(),
                            rbs_annotated: true,
                            rbs_inline_annotated: false,
                            sig_annotated: false,
                            attr_ivar: None,
                            is_singleton: false,
                            rbs_file_source: true,
                            synthetic_dsl_source: false,
                            rbs_method_types: Default::default(),
                            extra_overloads: Vec::new(),
                            loc: Some(loc),
                        },
                    );
                }
            }
        }

        if self.is_active_model_serializer_class(class_name) {
            self.register_active_model_serializer_framework_methods(class_name, loc);
        }

        if self.is_active_job_class(class_name) {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new("perform_later"),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Class(Sym::new(class_name)),
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
                    name: Sym::new("perform_now"),
                    param_infos: Vec::new(),
                    raw_return_type: Type::MethodReturnRef(class_name.into(), "perform".into()),
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

        if self.is_action_mailer_class(class_name) {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new("mail"),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Class(Sym::new("Mail::Message")),
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
                    loc: Some(loc),
                },
            );
            self.ensure_message_delivery_methods(loc);
        }

        if matches!(class_name, "ActiveRecord::Base" | "ApplicationRecord")
            && self.rails_at_least(6, 0)
        {
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
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );

            for name in ["insert_all", "upsert_all"] {
                self.registry.add_method_def_if_missing(
                    class_name,
                    MethodDef {
                        name: Sym::new(name),
                        param_infos: vec![ParamInfo {
                            name: "attributes".to_string(),
                            kind: ParamKind::Required,
                            default_type: Some(Type::Array(Some(Box::new(Type::Hash(
                                Some(Box::new(Type::Untyped)),
                                Some(Box::new(Type::Untyped)),
                            ))))),
                        }],
                        raw_return_type: Type::Class(Sym::new("ActiveRecord::Result")),
                        sorbet_modifier_comments: Vec::new(),
                        rbs_annotated: true,
                        rbs_inline_annotated: false,
                        sig_annotated: false,
                        attr_ivar: None,
                        is_singleton: true,
                        rbs_file_source: true,
                        synthetic_dsl_source: true,
                        rbs_method_types: Default::default(),
                        extra_overloads: Vec::new(),
                        loc: Some(loc),
                    },
                );
            }
        }

        if matches!(class_name, "ActiveRecord::Base" | "ApplicationRecord") {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new("arel_table"),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Class(Sym::new("Arel::Table")),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
            for (name, return_type) in [("id", Type::Integer), ("id_value", Type::Integer)] {
                self.registry.add_method_def_if_missing(
                    class_name,
                    MethodDef {
                        name: Sym::new(name),
                        param_infos: Vec::new(),
                        raw_return_type: return_type,
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
                        loc: Some(loc),
                    },
                );
            }
            for (name, return_type) in [
                ("update", Type::Bool),
                ("update!", Type::Bool),
                ("assign_attributes", Type::Void),
            ] {
                self.registry.add_method_def_if_missing(
                    class_name,
                    MethodDef {
                        name: Sym::new(name),
                        param_infos: vec![ParamInfo {
                            name: "attributes".to_string(),
                            kind: ParamKind::Required,
                            default_type: Some(Type::Hash(
                                Some(Box::new(Type::Untyped)),
                                Some(Box::new(Type::Untyped)),
                            )),
                        }],
                        raw_return_type: return_type,
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
                        loc: Some(loc),
                    },
                );
            }

            if !self.rails_at_least(6, 0) {
                for (name, return_type) in [
                    ("update_attributes", Type::Bool),
                    ("update_attributes!", Type::Bool),
                ] {
                    self.registry.add_method_def_if_missing(
                        class_name,
                        MethodDef {
                            name: Sym::new(name),
                            param_infos: vec![ParamInfo {
                                name: "attributes".to_string(),
                                kind: ParamKind::Required,
                                default_type: Some(Type::Hash(
                                    Some(Box::new(Type::Untyped)),
                                    Some(Box::new(Type::Untyped)),
                                )),
                            }],
                            raw_return_type: return_type,
                            sorbet_modifier_comments: Vec::new(),
                            rbs_annotated: true,
                            rbs_inline_annotated: false,
                            sig_annotated: false,
                            attr_ivar: None,
                            is_singleton: false,
                            rbs_file_source: true,
                            synthetic_dsl_source: false,
                            rbs_method_types: Default::default(),
                            extra_overloads: Vec::new(),
                            loc: Some(loc),
                        },
                    );
                }
            }
        }
    }

    pub(super) fn register_mailer_action_proxy(
        &mut self,
        class_name: &str,
        method_name: &str,
        param_infos: &[ParamInfo],
        loc: SourceLocation,
    ) {
        if !self.action_mailer_dsl_enabled()
            || !self.is_action_mailer_class(class_name)
            || matches!(method_name, "mail" | "initialize")
        {
            return;
        }

        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(method_name),
                param_infos: param_infos.to_vec(),
                raw_return_type: Type::Class(Sym::new("ActionMailer::MessageDelivery")),
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
        // Copy the instance action's annotated param types to the singleton proxy. annotated_params is keyed per (name, is_singleton), so without copying, the proxy's params would degrade to untyped (they were originally shared with the instance side).
        for i in 0..param_infos.len() {
            if self
                .registry
                .get_annotated_param_type(class_name, method_name, true, i)
                .is_none()
                && let Some(ty) =
                    self.registry
                        .get_annotated_param_type(class_name, method_name, false, i)
            {
                self.registry
                    .set_annotated_param_type(class_name, method_name, true, i, ty);
            }
        }
    }

    fn ensure_message_delivery_methods(&mut self, loc: SourceLocation) {
        let class_name = "ActionMailer::MessageDelivery";
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("deliver_now"),
                param_infos: Vec::new(),
                raw_return_type: Type::Class(Sym::new("Mail::Message")),
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
                loc: Some(loc),
            },
        );
        for name in ["deliver_later", "deliver_later!", "deliver_now!"] {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(name),
                    param_infos: Vec::new(),
                    raw_return_type: if name == "deliver_later" || name == "deliver_later!" {
                        Type::Class(Sym::new(class_name))
                    } else {
                        Type::Class(Sym::new("Mail::Message"))
                    },
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
                    loc: Some(loc),
                },
            );
        }
    }

    pub(crate) fn is_action_controller_class(&self, class_name: &str) -> bool {
        class_name.ends_with("Controller")
            || self.class_matches_or_inherits(
                class_name,
                &[
                    "ActionController::Base",
                    "ActionController::API",
                    "ApplicationController",
                ],
            )
    }

    pub(crate) fn has_own_method(&self, class_name: &str, method_name: &str) -> bool {
        self.registry
            .get_methods(class_name)
            .iter()
            .any(|m| m.name == method_name && !m.synthetic_dsl_source && !m.rbs_file_source)
    }

    pub(crate) fn is_active_job_class(&self, class_name: &str) -> bool {
        class_name.ends_with("Job")
            || self.class_matches_or_inherits(class_name, &["ActiveJob::Base", "ApplicationJob"])
    }

    pub(super) fn propagate_perform_params_to_job_helpers(&mut self, class_name: &str) {
        let Some(perform_def) = self
            .registry
            .lookup_method_def(class_name, "perform", false)
            .cloned()
        else {
            return;
        };
        let perform_param_names = perform_def.effective_param_names();
        if perform_param_names.is_empty() {
            return;
        }
        // Copy the `perform` entry of annotated_params for use by the helper
        let annotated: Vec<(usize, crate::types::Type)> = perform_param_names
            .iter()
            .enumerate()
            .filter_map(|(i, _)| {
                self.registry
                    .get_annotated_param_type(class_name, "perform", false, i)
                    .map(|ty| (i, ty))
            })
            .collect();
        for helper_name in ["perform_later", "perform_now"] {
            self.registry.update_method_params(
                class_name,
                helper_name,
                true,
                perform_def.param_infos.clone(),
            );
            for (i, ty) in &annotated {
                self.registry.set_annotated_param_type(
                    class_name,
                    helper_name,
                    true,
                    *i,
                    ty.clone(),
                );
            }
        }
    }

    pub(super) fn synthetic_action_controller_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let Type::Class(class_name) = receiver_type else {
            return None;
        };
        if class_name != "ActionController::Parameters" {
            return None;
        }
        let ac_params = || Type::Class(Sym::new("ActionController::Parameters"));
        match method_name {
            "require"
            | "permit"
            | "expect"
            | "merge"
            | "merge!"
            | "reverse_merge"
            | "reverse_merge!"
            | "with_defaults"
            | "with_defaults!"
            | "slice"
            | "except"
            | "transform_values"
            | "transform_values!"
            | "transform_keys"
            | "transform_keys!"
            | "select"
            | "reject"
            | "compact"
            | "deep_transform_values"
            | "deep_transform_values!" => Some(ac_params()),
            "to_h" | "to_unsafe_h" | "to_hash" => Some(Type::Hash(
                Some(Box::new(Type::String)),
                Some(Box::new(Type::Untyped)),
            )),
            "keys" => Some(Type::Array(Some(Box::new(Type::String)))),
            "values" | "values_at" => Some(Type::Array(Some(Box::new(Type::Untyped)))),
            "any?" | "all?" | "empty?" | "none?" | "has_key?" | "include?" | "key?" | "member?"
            | "present?" | "blank?" => Some(Type::Bool),
            "[]" | "fetch" | "dig" | "each_value" => Some(Type::Untyped),
            _ => None,
        }
    }

    pub(super) fn synthetic_active_support_hash_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let Type::Class(class_name) = receiver_type else {
            return None;
        };
        if !matches!(
            class_name.as_str(),
            "HashWithIndifferentAccess" | "ActiveSupport::HashWithIndifferentAccess"
        ) {
            return None;
        }

        let same_hash = || Type::Class(*class_name);
        match method_name {
            "[]" | "fetch" | "dig" | "delete" | "each" | "each_pair" | "each_key"
            | "each_value" => Some(Type::Untyped),
            "keys" | "values" | "values_at" => Some(Type::Array(Some(Box::new(Type::Untyped)))),
            "to_h" | "to_hash" | "to_options" => Some(Type::Hash(
                Some(Box::new(Type::Untyped)),
                Some(Box::new(Type::Untyped)),
            )),
            "any?" | "all?" | "empty?" | "none?" | "has_key?" | "include?" | "key?" | "member?"
            | "present?" | "blank?" => Some(Type::Bool),
            "merge"
            | "merge!"
            | "reverse_merge"
            | "reverse_merge!"
            | "with_defaults"
            | "with_defaults!"
            | "slice"
            | "except"
            | "symbolize_keys"
            | "stringify_keys"
            | "deep_symbolize_keys"
            | "deep_stringify_keys"
            | "with_indifferent_access" => Some(same_hash()),
            _ => None,
        }
    }

    pub(super) fn synthetic_active_support_duration_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let Type::Class(class_name) = receiver_type else {
            return None;
        };
        if class_name != "ActiveSupport::Duration" {
            return None;
        }

        match method_name {
            "in_seconds" | "seconds" | "to_i" => Some(Type::Integer),
            "ago" | "until" | "from_now" | "since" => Some(Type::Class(Sym::new("Time"))),
            "parts" => Some(Type::Hash(
                Some(Box::new(Type::Symbol)),
                Some(Box::new(Type::Untyped)),
            )),
            _ => None,
        }
    }

    pub(super) fn synthetic_i18n_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let Type::Singleton(class_name) = receiver_type else {
            return None;
        };
        if class_name != "I18n" {
            return None;
        }

        match method_name {
            "t" | "translate" | "l" | "localize" => Some(Type::String),
            "locale" | "default_locale" => Some(Type::Symbol),
            "available_locales" => Some(Type::Array(Some(Box::new(Type::Symbol)))),
            "exists?" | "exists" => Some(Type::Bool),
            "with_locale" => Some(Type::Untyped),
            _ => None,
        }
    }

    pub(super) fn synthetic_rails_singleton_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let Type::Singleton(class_name) = receiver_type else {
            return None;
        };
        if class_name != "Rails" {
            return None;
        }

        match method_name {
            "application" => Some(Type::Class(Sym::new("Rails::Application"))),
            "configuration" => Some(Type::Class(Sym::new("Rails::Application::Configuration"))),
            "cache" => Some(Type::Class(Sym::new("ActiveSupport::Cache::Store"))),
            "logger" => {
                let broadcast = "ActiveSupport::BroadcastLogger";
                if self.class_declared_in_any_source(broadcast) {
                    Some(Type::Class(Sym::new(broadcast)))
                } else {
                    Some(Type::Class(Sym::new("Logger")))
                }
            }
            "root" | "public_path" => Some(Type::Class(Sym::new("Pathname"))),
            "env" => Some(Type::Class(Sym::new("ActiveSupport::StringInquirer"))),
            _ => None,
        }
    }

    pub(super) fn synthetic_rails_configuration_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let Type::Class(class_name) = receiver_type else {
            return None;
        };
        if class_name != "Rails::Application::Configuration" {
            return None;
        }

        match method_name {
            "x" => Some(Type::Untyped),
            _ => None,
        }
    }

    pub(super) fn synthetic_active_support_cache_store_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let Type::Class(class_name) = receiver_type else {
            return None;
        };
        if class_name != "ActiveSupport::Cache::Store" {
            return None;
        }

        match method_name {
            "fetch" | "read" | "read_multi" => Some(Type::Untyped),
            "write" | "delete" | "delete_matched" | "exist?" | "clear" => Some(Type::Bool),
            _ => None,
        }
    }

    pub(super) fn synthetic_action_controller_helper_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let Type::Class(class_name) = receiver_type else {
            return None;
        };
        if !self.action_controller_helpers_enabled()
            || !self.class_matches_or_inherits(
                class_name,
                &[
                    "ActionController::Base",
                    "ActionController::API",
                    "ApplicationController",
                ],
            )
        {
            return None;
        }

        let hash = || Type::Hash(Some(Box::new(Type::Untyped)), Some(Box::new(Type::Untyped)));
        match method_name {
            "request" => Some(Type::Class(Sym::new("ActionDispatch::Request"))),
            "response" => Some(Type::Class(Sym::new("ActionDispatch::Response"))),
            "flash" | "session" | "cookies" => Some(hash()),
            "headers" => Some(Type::Hash(
                Some(Box::new(Type::String)),
                Some(Box::new(Type::String)),
            )),
            "helpers" => Some(Type::Untyped),
            "respond_to" if self.should_fast_path_controller_respond_to(class_name) => {
                Some(Type::Void)
            }
            _ => None,
        }
    }

    pub(super) fn synthetic_action_dispatch_request_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let Type::Class(class_name) = receiver_type else {
            return None;
        };
        if class_name != "ActionDispatch::Request" {
            return None;
        }

        match method_name {
            "host" | "host_with_port" | "remote_ip" | "referer" | "referrer" | "path"
            | "fullpath" | "original_url" | "url" | "user_agent" | "method" | "request_method"
            | "uuid" => Some(Type::String),
            "get?" | "post?" | "put?" | "patch?" | "delete?" | "xhr?" | "ssl?" | "local?" => {
                Some(Type::Bool)
            }
            "headers" => Some(Type::Hash(
                Some(Box::new(Type::String)),
                Some(Box::new(Type::String)),
            )),
            "env" | "params" | "query_parameters" | "request_parameters" | "path_parameters" => {
                Some(Type::Hash(
                    Some(Box::new(Type::Untyped)),
                    Some(Box::new(Type::Untyped)),
                ))
            }
            "format" => Some(Type::Untyped),
            _ => None,
        }
    }

    fn is_action_mailer_class(&self, class_name: &str) -> bool {
        class_name.ends_with("Mailer")
            || self
                .class_matches_or_inherits(class_name, &["ActionMailer::Base", "ApplicationMailer"])
    }

    pub(super) fn is_active_record_model_class(&self, class_name: &str) -> bool {
        self.class_matches_or_inherits(class_name, &["ActiveRecord::Base", "ApplicationRecord"])
            || class_name == "ApplicationRecord"
    }

    pub(super) fn is_active_record_dsl_target(&self, class_name: &str) -> bool {
        self.is_active_record_model_class(class_name) || self.is_collecting_concern_included()
    }

    pub(super) fn owner_relation_type_for_dsl(&self, class_name: &str) -> Type {
        if self.is_collecting_concern_included() {
            Type::Generic {
                base: Sym::new(ACTIVE_RECORD_RELATION_CLASS),
                args: vec![Type::SelfType].into(),
            }
        } else {
            Self::active_record_relation_type(class_name)
        }
    }

    pub(super) fn is_active_model_serializers_model_class(&self, class_name: &str) -> bool {
        self.class_matches_or_inherits(class_name, &["ActiveModelSerializers::Model"])
            || class_name == "ActiveModelSerializers::Model"
    }

    pub(super) fn is_active_model_serializer_class(&self, class_name: &str) -> bool {
        self.class_matches_or_inherits(class_name, &["ActiveModel::Serializer"])
            || class_name == "ActiveModel::Serializer"
    }

    pub(crate) fn class_matches_or_inherits(&self, class_name: &str, bases: &[&str]) -> bool {
        if bases.contains(&class_name) {
            return true;
        }

        let mut current = self.resolved_superclass_name(class_name);
        let mut seen = HashSet::new();
        while let Some(name) = current {
            if !seen.insert(name.clone()) {
                break;
            }
            if bases.contains(&name.as_str()) {
                return true;
            }
            current = self.resolved_superclass_name(&name);
        }
        false
    }

    fn resolved_superclass_name(&self, class_name: &str) -> Option<String> {
        let raw = self
            .registry
            .class_data_for(class_name)?
            .superclass
            .as_ref()?
            .to_string();
        if let Some(stripped) = raw.strip_prefix("::") {
            return Some(stripped.to_string());
        }
        if !raw.contains("::") && self.registry.class_data_for(&raw).is_none() {
            let mut ns = class_name;
            while let Some(idx) = ns.rfind_scope_sep() {
                ns = &ns[..idx];
                let candidate = crate::sym::join_scope(ns, &raw);
                if candidate != class_name && self.registry.class_data_for(&candidate).is_some() {
                    return Some(candidate);
                }
            }
        }
        Some(raw)
    }

    pub(super) fn class_or_ancestors_include_module(
        &self,
        class_name: &str,
        module_name: &str,
    ) -> bool {
        // `SharedName` throughout: the DSL plugins call this per method
        // resolution, and every name here is already an `Arc<str>` on the
        // registry side, so the walk needs no per-node string copy.
        let mut class_queue: Vec<SharedName> = vec![SharedName::from(class_name)];
        let mut seen_classes: FxHashSet<SharedName> = FxHashSet::default();
        let mut seen_modules: FxHashSet<SharedName> = FxHashSet::default();

        while let Some(current) = class_queue.pop() {
            if !seen_classes.insert(current.clone()) {
                continue;
            }
            let Some(data) = self.registry.class_data_for(&current) else {
                continue;
            };
            if let Some(superclass) = &data.superclass {
                class_queue.push(superclass.clone());
            }
            for mixin in &data.mixins {
                if mixin.module_name.as_ref() == module_name {
                    return true;
                }
                if seen_modules.insert(mixin.module_name.clone())
                    && self.module_includes_module(
                        &mixin.module_name,
                        module_name,
                        &mut seen_modules,
                    )
                {
                    return true;
                }
            }
        }
        false
    }

    fn module_includes_module(
        &self,
        module_name: &str,
        target_module_name: &str,
        seen_modules: &mut FxHashSet<SharedName>,
    ) -> bool {
        if module_name == target_module_name {
            return true;
        }
        let Some(data) = self.registry.class_data_for(module_name) else {
            return false;
        };
        for mixin in &data.mixins {
            if mixin.module_name.as_ref() == target_module_name {
                return true;
            }
            if seen_modules.insert(mixin.module_name.clone())
                && self.module_includes_module(&mixin.module_name, target_module_name, seen_modules)
            {
                return true;
            }
        }
        false
    }

    fn scope_chain_return_is_relation_like(ret: &Type) -> bool {
        match ret {
            Type::Union(parts) => parts
                .first()
                .is_some_and(Self::scope_chain_return_is_relation_like),
            _ => matches!(
                Self::nominal_base_name(ret),
                Some(ACTIVE_RECORD_RELATION_CLASS | ACTIVE_RECORD_COLLECTION_PROXY_CLASS)
            ),
        }
    }

    pub(super) fn active_record_relation_element_type(ty: &Type) -> Option<Type> {
        let base = Self::nominal_base_name(ty)?;
        if !matches!(
            base,
            ACTIVE_RECORD_RELATION_CLASS | ACTIVE_RECORD_COLLECTION_PROXY_CLASS
        ) {
            return None;
        }
        Self::extract_type_args(ty).into_iter().next()
    }

    pub(super) fn active_record_relation_type(target_class: &str) -> Type {
        Type::Generic {
            base: Sym::new(ACTIVE_RECORD_RELATION_CLASS),
            args: vec![Type::Class(Sym::new(target_class))].into(),
        }
    }

    fn register_active_model_serializer_framework_methods(
        &mut self,
        class_name: &str,
        loc: SourceLocation,
    ) {
        let object_type = self
            .infer_active_model_serializer_object_class(class_name)
            .map(|name| Type::Class(Sym::new(name)))
            .unwrap_or(Type::Untyped);

        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("object"),
                param_infos: Vec::new(),
                raw_return_type: object_type,
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
                loc: Some(loc),
            },
        );

        for name in ["scope", "instance_options"] {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(name),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Untyped,
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
                    loc: Some(loc),
                },
            );
        }

        if self.is_active_record_model_class(class_name) {
            self.register_active_record_relation_model_methods(class_name, loc);
        }
    }

    fn infer_active_model_serializer_object_class(&self, class_name: &str) -> Option<String> {
        let mut parts: Vec<&str> = class_name.split("::").collect();
        let last = parts.last_mut()?;
        let base = last.strip_suffix("Serializer")?;
        if base.is_empty() || base == "Application" {
            return None;
        }

        *last = base;
        let full_name = parts.join("::");
        if self.registry.has_class(&full_name) {
            return Some(full_name);
        }

        if self.registry.has_class(base) {
            return Some(base.to_string());
        }

        for start in 1..parts.len() {
            let candidate = parts[start..].join("::");
            if self.registry.has_class(&candidate) {
                return Some(candidate);
            }
        }

        Some(base.to_string())
    }

    pub(in crate::inference) fn active_record_collection_proxy_type(target_class: &str) -> Type {
        Type::Generic {
            base: Sym::new(ACTIVE_RECORD_COLLECTION_PROXY_CLASS),
            args: vec![Type::Class(Sym::new(target_class))].into(),
        }
    }

    fn active_record_relation_type_for_elem(elem: &Type) -> Type {
        Type::Generic {
            base: Sym::new(ACTIVE_RECORD_RELATION_CLASS),
            args: vec![elem.clone()].into(),
        }
    }

    pub(super) fn register_active_record_relation_model_methods(
        &mut self,
        class_name: &str,
        loc: SourceLocation,
    ) {
        let relation_type = Self::active_record_relation_type(class_name);
        let model_type = Self::active_record_model_type(class_name);
        let optional_model_type = Self::active_record_optional_model_type(class_name);

        for name in ACTIVE_RECORD_RELATION_METHODS {
            let (_param_names, param_infos) = match *name {
                "all" | "none" => (Vec::new(), Vec::new()),
                "merge" => (
                    vec!["relation".to_string()],
                    vec![ParamInfo {
                        name: "relation".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(relation_type.clone()),
                    }],
                ),
                "limit" | "offset" => (
                    vec!["value".to_string()],
                    vec![ParamInfo {
                        name: "value".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::Integer),
                    }],
                ),
                _ => (
                    vec!["args".to_string()],
                    vec![ParamInfo {
                        name: "args".to_string(),
                        kind: ParamKind::Rest,
                        default_type: None,
                    }],
                ),
            };
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos,
                    raw_return_type: relation_type.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        let versioned_methods: &[(&[&str], u16, u16)] = &[
            (ACTIVE_RECORD_RELATION_METHODS_RAILS_6_1, 6, 1),
            (ACTIVE_RECORD_RELATION_METHODS_RAILS_7_0, 7, 0),
            (ACTIVE_RECORD_RELATION_METHODS_RAILS_7_1, 7, 1),
        ];
        for (methods, major, minor) in versioned_methods {
            if !self.rails_at_least(*major, *minor) {
                continue;
            }
            for name in *methods {
                let (_param_names, param_infos) = match *name {
                    "in_order_of" => (
                        vec!["column".to_string(), "values".to_string()],
                        vec![
                            ParamInfo {
                                name: "column".to_string(),
                                kind: ParamKind::Required,
                                default_type: Some(Type::Untyped),
                            },
                            ParamInfo {
                                name: "values".to_string(),
                                kind: ParamKind::Required,
                                default_type: Some(Type::Untyped),
                            },
                        ],
                    ),
                    "strict_loading" => (
                        vec!["value".to_string()],
                        vec![ParamInfo {
                            name: "value".to_string(),
                            kind: ParamKind::Optional,
                            default_type: Some(Type::Untyped),
                        }],
                    ),
                    _ => (
                        vec!["args".to_string()],
                        vec![ParamInfo {
                            name: "args".to_string(),
                            kind: ParamKind::Rest,
                            default_type: None,
                        }],
                    ),
                };
                self.registry.add_method_def_if_missing(
                    class_name,
                    MethodDef {
                        name: Sym::new(*name),
                        param_infos,
                        raw_return_type: relation_type.clone(),
                        sorbet_modifier_comments: Vec::new(),
                        rbs_annotated: true,
                        rbs_inline_annotated: false,
                        sig_annotated: false,
                        attr_ivar: None,
                        is_singleton: true,
                        rbs_file_source: true,
                        synthetic_dsl_source: true,
                        rbs_method_types: Default::default(),
                        extra_overloads: Vec::new(),
                        loc: Some(loc),
                    },
                );
            }
        }

        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("find"),
                param_infos: vec![ParamInfo {
                    name: "id_or_ids".to_string(),
                    kind: ParamKind::Required,
                    default_type: Some(Type::Untyped),
                }],
                raw_return_type: Self::active_record_model_type(class_name),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: true,
                rbs_file_source: true,
                synthetic_dsl_source: true,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );

        for name in ACTIVE_RECORD_OPTIONAL_FINDER_METHODS {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos: if matches!(*name, "find_by") {
                        vec![ParamInfo {
                            name: "attributes".to_string(),
                            kind: ParamKind::Required,
                            default_type: Some(Type::Untyped),
                        }]
                    } else {
                        Vec::new()
                    },
                    raw_return_type: optional_model_type.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        for name in ACTIVE_RECORD_POSITIONAL_OPTIONAL_FINDER_METHODS {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos: Vec::new(),
                    raw_return_type: optional_model_type.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        for name in ACTIVE_RECORD_REQUIRED_FINDER_METHODS {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos: Vec::new(),
                    raw_return_type: model_type.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        for name in ACTIVE_RECORD_POSITIONAL_REQUIRED_FINDER_METHODS {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos: Vec::new(),
                    raw_return_type: model_type.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        for name in ACTIVE_RECORD_FIND_BY_ATTRS_METHODS {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos: vec![ParamInfo {
                        name: "attributes".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::Untyped),
                    }],
                    raw_return_type: model_type.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        for name in ACTIVE_RECORD_FIND_OR_CREATE_METHODS {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos: vec![ParamInfo {
                        name: "attributes".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::Untyped),
                    }],
                    raw_return_type: model_type.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        for name in ACTIVE_RECORD_BUILDER_METHODS {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos: vec![ParamInfo {
                        name: "attributes".to_string(),
                        kind: ParamKind::Optional,
                        default_type: Some(Type::Untyped),
                    }],
                    raw_return_type: model_type.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        for name in ACTIVE_RECORD_ENUMERABLE_QUERY_METHODS {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Bool,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        for name in ACTIVE_RECORD_CALCULATION_METHODS {
            let (_param_names, param_infos, return_type) = match *name {
                "count" | "ids" => (
                    Vec::new(),
                    Vec::new(),
                    Self::active_record_common_method_return(class_name, name).unwrap(),
                ),
                "pick" | "pluck" => (
                    vec!["columns".to_string()],
                    vec![ParamInfo {
                        name: "columns".to_string(),
                        kind: ParamKind::Rest,
                        default_type: None,
                    }],
                    Self::active_record_common_method_return(class_name, name).unwrap(),
                ),
                _ => (
                    vec!["column".to_string()],
                    vec![ParamInfo {
                        name: "column".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(Type::Untyped),
                    }],
                    Self::active_record_common_method_return(class_name, name).unwrap(),
                ),
            };
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos,
                    raw_return_type: return_type,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        for name in ACTIVE_RECORD_BOOL_RELATION_METHODS {
            let (_param_names, param_infos) = match *name {
                "exists?" => (
                    vec!["conditions".to_string()],
                    vec![ParamInfo {
                        name: "conditions".to_string(),
                        kind: ParamKind::Optional,
                        default_type: Some(Type::Untyped),
                    }],
                ),
                "include?" => (
                    vec!["record".to_string()],
                    vec![ParamInfo {
                        name: "record".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(model_type.clone()),
                    }],
                ),
                _ => (Vec::new(), Vec::new()),
            };
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos,
                    raw_return_type: Type::Bool,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        for name in ACTIVE_RECORD_BATCH_METHODS {
            let (_param_names, param_infos) = match *name {
                "find_each" | "find_in_batches" => (
                    vec![
                        "start".to_string(),
                        "finish".to_string(),
                        "batch_size".to_string(),
                        "error_on_ignore".to_string(),
                        "order".to_string(),
                    ],
                    vec![
                        ParamInfo {
                            name: "start".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Untyped),
                        },
                        ParamInfo {
                            name: "finish".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Untyped),
                        },
                        ParamInfo {
                            name: "batch_size".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Integer),
                        },
                        ParamInfo {
                            name: "error_on_ignore".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Untyped),
                        },
                        ParamInfo {
                            name: "order".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Untyped),
                        },
                    ],
                ),
                "in_batches" => (
                    vec![
                        "of".to_string(),
                        "start".to_string(),
                        "finish".to_string(),
                        "load".to_string(),
                        "error_on_ignore".to_string(),
                        "order".to_string(),
                        "use_ranges".to_string(),
                    ],
                    vec![
                        ParamInfo {
                            name: "of".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Integer),
                        },
                        ParamInfo {
                            name: "start".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Untyped),
                        },
                        ParamInfo {
                            name: "finish".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Untyped),
                        },
                        ParamInfo {
                            name: "load".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Bool),
                        },
                        ParamInfo {
                            name: "error_on_ignore".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Untyped),
                        },
                        ParamInfo {
                            name: "order".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Untyped),
                        },
                        ParamInfo {
                            name: "use_ranges".to_string(),
                            kind: ParamKind::KeywordOptional,
                            default_type: Some(Type::Untyped),
                        },
                    ],
                ),
                _ => (Vec::new(), Vec::new()),
            };
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(*name),
                    param_infos,
                    raw_return_type: Self::active_record_batch_method_return(class_name, name)
                        .unwrap_or(Type::Untyped),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: None,
                    is_singleton: true,
                    rbs_file_source: true,
                    synthetic_dsl_source: true,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }
    }

    pub(super) fn synthetic_active_record_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        let (elem_type, singleton_model) =
            if let Some(et) = Self::active_record_relation_element_type(receiver_type) {
                (et, false)
            } else if let Type::Singleton(name) = receiver_type {
                if self.is_active_record_model_class(name) {
                    (Type::Class(*name), true)
                } else {
                    return None;
                }
            } else {
                return None;
            };

        let relation_type_for_singleton =
            || Self::active_record_relation_type(&elem_type.to_string());

        let relation_return = || {
            if singleton_model {
                relation_type_for_singleton()
            } else {
                receiver_type.clone()
            }
        };

        match method_name {
            "async" | "load_async" if self.rails_at_least(7, 0) => Some(relation_return()),
            name if ACTIVE_RECORD_RELATION_METHODS.contains(&name) => Some(relation_return()),
            name if ACTIVE_RECORD_RELATION_METHODS_RAILS_6_1.contains(&name)
                && self.rails_at_least(6, 1) =>
            {
                Some(relation_return())
            }
            name if ACTIVE_RECORD_RELATION_METHODS_RAILS_7_0.contains(&name)
                && self.rails_at_least(7, 0) =>
            {
                Some(relation_return())
            }
            name if ACTIVE_RECORD_RELATION_METHODS_RAILS_7_1.contains(&name)
                && self.rails_at_least(7, 1) =>
            {
                Some(relation_return())
            }
            name if ACTIVE_RECORD_WHERE_CHAIN_METHODS.contains(&name) => Some(relation_return()),
            "any?" | "many?" | "none?" | "one?" => Some(Type::Bool),
            "exists?" | "include?" => Some(Type::Bool),
            "count" => Some(Type::Integer),
            "ids" => Some(Type::Array(Some(Box::new(Type::Integer)))),
            "pick" | "sum" | "average" | "minimum" | "maximum" => Some(Type::Untyped),
            "pluck" => Some(Type::Array(Some(Box::new(Type::Untyped)))),
            "kept" | "undiscarded" | "discarded" | "with_discarded"
                if self.discard_dsl_enabled()
                    && self
                        .registry
                        .lookup_method_sig(&elem_type.to_string(), method_name)
                        .is_some() =>
            {
                Some(relation_return())
            }
            "find" => Some(elem_type.clone()),
            "find_by" | "first" | "last" | "take" => {
                Some(Type::Union(vec![elem_type.clone(), Type::Nil]))
            }
            "second" | "third" | "fourth" | "fifth" | "forty_two" | "third_to_last"
            | "second_to_last" => Some(Type::Union(vec![elem_type.clone(), Type::Nil])),
            "find_by!" | "first!" | "last!" | "take!" | "sole" | "find_sole_by" | "second!"
            | "third!" | "fourth!" | "fifth!" | "forty_two!" | "third_to_last!"
            | "second_to_last!" => Some(elem_type.clone()),
            name if ACTIVE_RECORD_FIND_OR_CREATE_METHODS.contains(&name) => Some(elem_type.clone()),
            "to_a" => Some(Type::Array(Some(Box::new(elem_type)))),
            "async_count" if self.rails_at_least(7, 1) => Some(Type::Generic {
                base: Sym::new("ActiveRecord::Promise"),
                args: vec![Type::Integer].into(),
            }),
            "async_ids" if self.rails_at_least(7, 1) => Some(Type::Generic {
                base: Sym::new("ActiveRecord::Promise"),
                args: vec![Type::Array(Some(Box::new(Type::Integer)))].into(),
            }),
            "async_pick" | "async_pluck" | "async_sum" | "async_average" | "async_minimum"
            | "async_maximum"
                if self.rails_at_least(7, 1) =>
            {
                Some(Type::Generic {
                    base: Sym::new("ActiveRecord::Promise"),
                    args: vec![Type::Untyped].into(),
                })
            }
            name if ACTIVE_RECORD_BUILDER_METHODS.contains(&name) => Some(elem_type.clone()),
            "find_each" => Some(Type::Generic {
                base: Sym::new("Enumerator"),
                args: vec![elem_type].into(),
            }),
            "find_in_batches" => Some(Type::Generic {
                base: Sym::new("Enumerator"),
                args: vec![Type::Array(Some(Box::new(elem_type)))].into(),
            }),
            "in_batches" => Some(Type::Class(Sym::new(
                "ActiveRecord::Batches::BatchEnumerator",
            ))),
            _ => {
                if !singleton_model {
                    let model_class = elem_type.to_string();
                    if let Some(ret) = self.registry.lookup_method_return_type_with_hint(
                        &model_class,
                        method_name,
                        true,
                    ) {
                        if Self::scope_chain_return_is_relation_like(&ret) {
                            return Some(relation_return());
                        }
                        if ret == Type::SelfType {
                            return Some(relation_return());
                        }
                    }
                }
                None
            }
        }
    }

    pub(super) fn refine_active_record_pluck_with_symbol_args(
        &self,
        receiver_type: &Type,
        method_name: &str,
        symbol_args: &[String],
    ) -> Option<Type> {
        if method_name != "pluck" || symbol_args.is_empty() {
            return None;
        }
        let elem_type = if let Some(et) = Self::active_record_relation_element_type(receiver_type) {
            et
        } else if let Type::Singleton(name) = receiver_type {
            if self.is_active_record_model_class(name) {
                Type::Class(*name)
            } else {
                return None;
            }
        } else {
            return None;
        };
        let model_name = match &elem_type {
            Type::Class(name) => *name,
            _ => return None,
        };
        let mut col_types = Vec::with_capacity(symbol_args.len());
        for col_name in symbol_args {
            let ty = self
                .registry
                .lookup_method_return_type(&model_name, col_name)?;
            if matches!(
                ty,
                Type::Untyped
                    | Type::ParamRef(_)
                    | Type::KeywordParamRef(_)
                    | Type::MethodReturnRef(..)
                    | Type::ReceiverMethodRef(..)
            ) {
                return None;
            }
            col_types.push(ty);
        }
        let element = if col_types.len() == 1 {
            col_types.into_iter().next().expect("len==1")
        } else {
            Type::Tuple(col_types)
        };
        Some(Type::Array(Some(Box::new(element))))
    }

    pub(super) fn refine_active_record_find_with_args(
        &self,
        receiver_type: &Type,
        method_name: &str,
        first_arg_type: Option<&Type>,
        positional_arg_count: usize,
    ) -> Option<Type> {
        let elem_type = if let Some(et) = Self::active_record_relation_element_type(receiver_type) {
            et
        } else if let Type::Singleton(name) = receiver_type {
            if self.is_active_record_model_class(name) {
                Type::Class(*name)
            } else {
                return None;
            }
        } else {
            return None;
        };
        let array_return = Type::Array(Some(Box::new(elem_type.clone())));
        match method_name {
            "find" => {
                if positional_arg_count >= 2 {
                    return Some(array_return);
                }
                if let Some(arg_ty) = first_arg_type {
                    let array_like = matches!(arg_ty, Type::Array(_) | Type::Tuple(_));
                    let range_like =
                        matches!(arg_ty, Type::Class(name) if name.starts_with("Range"));
                    if array_like || range_like {
                        return Some(array_return);
                    }
                }
                None
            }
            "first" | "last" | "take" => {
                if positional_arg_count >= 1 {
                    return Some(array_return);
                }
                None
            }
            _ => None,
        }
    }

    pub(super) fn synthetic_active_record_block_method_return(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        if !matches!(method_name, "find_each" | "find_in_batches" | "in_batches") {
            return None;
        }
        if Self::active_record_relation_element_type(receiver_type).is_some() {
            return Some(Type::Untyped);
        }
        if let Type::Singleton(name) = receiver_type
            && self.is_active_record_model_class(name)
        {
            return Some(Type::Untyped);
        }
        None
    }

    pub(super) fn synthetic_active_record_method_sig(
        &self,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<crate::types::MethodSig> {
        let receiver_class = self.type_to_class_name(receiver_type)?;
        if !matches!(
            receiver_class.as_str(),
            ACTIVE_RECORD_RELATION_CLASS | ACTIVE_RECORD_COLLECTION_PROXY_CLASS
        ) {
            return None;
        }

        let elem_type = Self::active_record_relation_element_type(receiver_type)?;
        let return_type = self.synthetic_active_record_method_return(receiver_type, method_name)?;
        let relation_type = Self::active_record_relation_type_for_elem(&elem_type);

        let params = match method_name {
            "async" | "load_async" | "async_count" | "async_ids" => Vec::new(),
            "kept" | "undiscarded" | "discarded" | "with_discarded" => Vec::new(),
            "any?" | "many?" | "none?" | "one?" | "count" | "ids" => Vec::new(),
            "all" | "none" | "invert_where" | "reverse_order" => Vec::new(),
            "second" | "second!" | "third" | "third!" | "fourth" | "fourth!" | "fifth"
            | "fifth!" | "forty_two" | "forty_two!" | "third_to_last" | "third_to_last!"
            | "second_to_last" | "second_to_last!" | "first" | "first!" | "last" | "last!"
            | "take" | "take!" | "sole" => Vec::new(),
            "find" => vec![crate::types::Param {
                name: "id_or_ids".to_string(),
                param_type: Type::Untyped,
                kind: ParamKind::Required,
            }],
            "find_by" | "find_by!" | "find_sole_by" | "rewhere" | "create_with" => {
                vec![crate::types::Param {
                    name: "attributes".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::Required,
                }]
            }
            "where" | "reorder" | "joins" | "left_outer_joins" | "group" | "select"
            | "eager_load" | "preload" | "references" | "with" | "with_recursive" | "reselect"
            | "regroup" | "unscope" | "having" | "extending" | "optimizer_hints" | "annotate"
            | "excluding" | "not" | "associated" | "missing" => vec![crate::types::Param {
                name: "args".to_string(),
                param_type: Type::Untyped,
                kind: ParamKind::Rest,
            }],
            "order" => vec![crate::types::Param {
                name: "ordering".to_string(),
                param_type: Type::Untyped,
                kind: ParamKind::Rest,
            }],
            "in_order_of" => vec![
                crate::types::Param {
                    name: "column".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::Required,
                },
                crate::types::Param {
                    name: "values".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::Required,
                },
            ],
            "and" | "or" | "merge" => vec![crate::types::Param {
                name: "relation".to_string(),
                param_type: relation_type,
                kind: ParamKind::Required,
            }],
            "limit" => vec![crate::types::Param {
                name: "value".to_string(),
                param_type: Type::Integer,
                kind: ParamKind::Required,
            }],
            "offset" => vec![crate::types::Param {
                name: "value".to_string(),
                param_type: Type::Integer,
                kind: ParamKind::Required,
            }],
            "lock" | "readonly" | "strict_loading" | "distinct" => vec![crate::types::Param {
                name: "value".to_string(),
                param_type: Type::Untyped,
                kind: ParamKind::Optional,
            }],
            "from" => vec![
                crate::types::Param {
                    name: "value".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::Required,
                },
                crate::types::Param {
                    name: "subquery_name".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::Optional,
                },
            ],
            "async_pick" | "async_pluck" | "pick" | "pluck" => vec![crate::types::Param {
                name: "columns".to_string(),
                param_type: Type::Untyped,
                kind: ParamKind::Rest,
            }],
            "async_sum" | "async_average" | "async_minimum" | "async_maximum" | "sum"
            | "average" | "minimum" | "maximum" => vec![crate::types::Param {
                name: "column".to_string(),
                param_type: Type::Untyped,
                kind: ParamKind::Required,
            }],
            "exists?" => vec![crate::types::Param {
                name: "conditions".to_string(),
                param_type: Type::Untyped,
                kind: ParamKind::Optional,
            }],
            "include?" => vec![crate::types::Param {
                name: "record".to_string(),
                param_type: elem_type.clone(),
                kind: ParamKind::Required,
            }],
            "find_each" | "find_in_batches" => vec![
                crate::types::Param {
                    name: "start".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::KeywordOptional,
                },
                crate::types::Param {
                    name: "finish".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::KeywordOptional,
                },
                crate::types::Param {
                    name: "batch_size".to_string(),
                    param_type: Type::Integer,
                    kind: ParamKind::KeywordOptional,
                },
                crate::types::Param {
                    name: "error_on_ignore".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::KeywordOptional,
                },
                crate::types::Param {
                    name: "order".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::KeywordOptional,
                },
            ],
            "in_batches" => vec![
                crate::types::Param {
                    name: "of".to_string(),
                    param_type: Type::Integer,
                    kind: ParamKind::KeywordOptional,
                },
                crate::types::Param {
                    name: "start".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::KeywordOptional,
                },
                crate::types::Param {
                    name: "finish".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::KeywordOptional,
                },
                crate::types::Param {
                    name: "load".to_string(),
                    param_type: Type::Bool,
                    kind: ParamKind::KeywordOptional,
                },
                crate::types::Param {
                    name: "error_on_ignore".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::KeywordOptional,
                },
                crate::types::Param {
                    name: "order".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::KeywordOptional,
                },
                crate::types::Param {
                    name: "use_ranges".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::KeywordOptional,
                },
            ],
            name if ACTIVE_RECORD_FIND_OR_CREATE_METHODS.contains(&name) => {
                vec![crate::types::Param {
                    name: "attributes".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::Required,
                }]
            }
            name if ACTIVE_RECORD_BUILDER_METHODS.contains(&name) => vec![crate::types::Param {
                name: "attributes".to_string(),
                param_type: Type::Untyped,
                kind: ParamKind::Optional,
            }],
            _ => Vec::new(),
        };

        Some(crate::types::MethodSig {
            name: method_name.to_string(),
            params,
            return_type,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: true,
            rbs_inline_annotated: false,
            rbs_file_source: true,
            synthetic_dsl_source: true,
            sig_annotated: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        })
    }

    pub(in crate::inference) fn ensure_active_record_query_methods(&mut self, loc: SourceLocation) {
        for class_name in [
            ACTIVE_RECORD_RELATION_CLASS,
            ACTIVE_RECORD_COLLECTION_PROXY_CLASS,
        ] {
            self.registry
                .add_class_type_param(class_name, "Elem".to_string());

            for (name, param_name, kind, default_type, return_type) in [
                (
                    "where",
                    "conditions",
                    ParamKind::Required,
                    Some(Type::Untyped),
                    Type::SelfType,
                ),
                (
                    "order",
                    "ordering",
                    ParamKind::Required,
                    Some(Type::Untyped),
                    Type::SelfType,
                ),
                (
                    "limit",
                    "value",
                    ParamKind::Required,
                    Some(Type::Integer),
                    Type::SelfType,
                ),
                (
                    "offset",
                    "value",
                    ParamKind::Required,
                    Some(Type::Integer),
                    Type::SelfType,
                ),
                (
                    "merge",
                    "relation",
                    ParamKind::Required,
                    Some(Type::Class(Sym::new(ACTIVE_RECORD_RELATION_CLASS))),
                    Type::SelfType,
                ),
            ] {
                self.registry.add_method_def_if_missing(
                    class_name,
                    MethodDef {
                        name: Sym::new(name),
                        param_infos: vec![ParamInfo {
                            name: param_name.to_string(),
                            kind,
                            default_type,
                        }],
                        raw_return_type: return_type.clone(),
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
                        loc: Some(loc),
                    },
                );
            }

            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new("includes"),
                    param_infos: vec![ParamInfo {
                        name: "associations".to_string(),
                        kind: ParamKind::Rest,
                        default_type: None,
                    }],
                    raw_return_type: Type::SelfType,
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
                    loc: Some(loc),
                },
            );

            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new("first"),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Untyped,
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
                    loc: Some(loc),
                },
            );

            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new("to_a"),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Array(Some(Box::new(Type::Untyped))),
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
                    loc: Some(loc),
                },
            );
        }

        for name in ["build", "create"] {
            self.registry.add_method_def_if_missing(
                ACTIVE_RECORD_COLLECTION_PROXY_CLASS,
                MethodDef {
                    name: Sym::new(name),
                    param_infos: vec![ParamInfo {
                        name: "attributes".to_string(),
                        kind: ParamKind::Optional,
                        default_type: Some(Type::Untyped),
                    }],
                    raw_return_type: Type::Untyped,
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
                    loc: Some(loc),
                },
            );
        }
    }

    pub(in crate::inference) fn apply_with_options_fallback(
        &self,
        options: &mut AssociationOptions,
    ) {
        for parent_opts in self.with_options_stack.iter().rev() {
            if options.class_name.is_none() && parent_opts.class_name.is_some() {
                options.class_name = parent_opts.class_name.clone();
            }
            if !options.optional && parent_opts.optional {
                options.optional = true;
            }
            if options.delegate_to.is_none() && parent_opts.delegate_to.is_some() {
                options.delegate_to = parent_opts.delegate_to.clone();
            }
            if !options.delegate_allow_nil && parent_opts.delegate_allow_nil {
                options.delegate_allow_nil = true;
            }
            if options.delegate_prefix.is_none() && parent_opts.delegate_prefix.is_some() {
                options.delegate_prefix = parent_opts.delegate_prefix.clone();
            }
            if !options.delegate_prefix_target && parent_opts.delegate_prefix_target {
                options.delegate_prefix_target = true;
            }
        }
    }

    pub(super) fn extract_association_options(
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) -> AssociationOptions {
        AssociationOptions {
            class_name: Self::extract_hash_option_str(call_node, "class_name", parse_result),
            optional: Self::extract_hash_option_bool(call_node, "optional", parse_result)
                .unwrap_or(false),
            polymorphic: Self::extract_hash_option_bool(call_node, "polymorphic", parse_result)
                .unwrap_or(false),
            through: Self::extract_hash_option_str(call_node, "through", parse_result),
            source: Self::extract_hash_option_str(call_node, "source", parse_result),
            inverse_of: Self::extract_hash_option_str(call_node, "inverse_of", parse_result),
            delegate_to: Self::extract_hash_option_str(call_node, "to", parse_result),
            delegate_allow_nil: Self::extract_hash_option_bool(
                call_node,
                "allow_nil",
                parse_result,
            )
            .unwrap_or(false),
            delegate_prefix: Self::extract_hash_option_str(call_node, "prefix", parse_result),
            delegate_prefix_target: Self::extract_hash_option_bool(
                call_node,
                "prefix",
                parse_result,
            )
            .unwrap_or(false),
        }
    }

    fn association_type_mentions_class(ty: &Type, class_name: &str) -> bool {
        match ty {
            Type::Class(name) => name.as_str() == class_name,
            Type::Generic { base, args } => {
                base.as_str() == class_name
                    || (matches!(
                        base.as_str(),
                        ACTIVE_RECORD_RELATION_CLASS | ACTIVE_RECORD_COLLECTION_PROXY_CLASS
                    ) && args
                        .iter()
                        .any(|arg| Self::association_type_mentions_class(arg, class_name)))
            }
            Type::Union(parts) => parts
                .iter()
                .any(|part| Self::association_type_mentions_class(part, class_name)),
            _ => false,
        }
    }

    fn resolve_target_class_from_inverse_of(
        &self,
        current_class: &str,
        inverse_of: &str,
    ) -> Option<String> {
        self.registry
            .user_defined_class_names()
            .into_iter()
            .find(|candidate| {
                self.registry
                    .class_data_for(candidate)
                    .into_iter()
                    .flat_map(|data| data.methods.iter())
                    .any(|method| {
                        !method.is_singleton
                            && method.name == inverse_of
                            && Self::association_type_mentions_class(
                                &method.raw_return_type,
                                current_class,
                            )
                    })
            })
    }

    pub(in crate::inference) fn infer_association_target_class(
        &self,
        current_class: &str,
        assoc_name: &str,
        options: &AssociationOptions,
        collection: bool,
    ) -> String {
        let known_classes = self.registry.user_defined_class_names();
        if let Some(class_name) = &options.class_name {
            return class_name.clone();
        }
        if options.through.is_some()
            && let Some(source) = &options.source
        {
            return self.classify_association_target(source, &known_classes);
        }
        if let Some(inverse_of) = &options.inverse_of
            && let Some(target) =
                self.resolve_target_class_from_inverse_of(current_class, inverse_of)
        {
            return target;
        }

        let _ = collection;
        self.classify_association_target(assoc_name, &known_classes)
    }

    fn classify_association_target(&self, name: &str, known_classes: &[String]) -> String {
        if let Some(ref root) = self.project_root {
            crate::rails::classify_with_project_known_classes(root, name, known_classes)
        } else {
            crate::rails::classify_with_known_classes(name, known_classes)
        }
    }

    pub(in crate::inference) fn singularize_association_name(&self, name: &str) -> String {
        if let Some(ref root) = self.project_root {
            crate::rails::singularize_with_project(root, name)
        } else {
            crate::rails::singularize(name)
        }
    }

    pub(in crate::inference) fn extract_scope_param_infos_from_node(
        &self,
        node: &Node<'_>,
    ) -> Vec<ParamInfo> {
        let mut params = Vec::new();
        let params_node = match node {
            Node::LambdaNode { .. } => node.as_lambda_node().and_then(|lambda| lambda.parameters()),
            Node::BlockNode { .. } => node.as_block_node().and_then(|block| block.parameters()),
            _ => return params,
        };
        let Some(inner) = params_node
            .and_then(|params| params.as_block_parameters_node())
            .and_then(|params| params.parameters())
        else {
            return params;
        };

        for req in inner.requireds().iter() {
            if let Some(name) = Self::extract_param_name(&req) {
                params.push(ParamInfo {
                    name,
                    kind: ParamKind::Required,
                    default_type: Some(Type::Untyped),
                });
            }
        }
        for opt in inner.optionals().iter() {
            if let Some(optional) = opt.as_optional_parameter_node() {
                params.push(ParamInfo {
                    name: String::from_utf8_lossy(optional.name().as_slice()).to_string(),
                    kind: ParamKind::Optional,
                    default_type: Some(self.static_node_type(&optional.value())),
                });
            }
        }
        if let Some(rest) = inner.rest()
            && let Some(name) = rest
                .as_rest_parameter_node()
                .and_then(|node| {
                    node.name()
                        .map(|name| String::from_utf8_lossy(name.as_slice()).to_string())
                })
                .or_else(|| self.supports_anonymous_rest_params().then(String::new))
        {
            params.push(ParamInfo {
                name,
                kind: ParamKind::Rest,
                default_type: None,
            });
        }
        for keyword in inner.keywords().iter() {
            match &keyword {
                Node::RequiredKeywordParameterNode { .. } => {
                    let Some(parameter) = keyword.as_required_keyword_parameter_node() else {
                        continue;
                    };
                    params.push(ParamInfo {
                        name: String::from_utf8_lossy(parameter.name().as_slice()).to_string(),
                        kind: ParamKind::KeywordRequired,
                        default_type: Some(Type::Untyped),
                    });
                }
                Node::OptionalKeywordParameterNode { .. } => {
                    let Some(parameter) = keyword.as_optional_keyword_parameter_node() else {
                        continue;
                    };
                    params.push(ParamInfo {
                        name: String::from_utf8_lossy(parameter.name().as_slice()).to_string(),
                        kind: ParamKind::KeywordOptional,
                        default_type: Some(self.static_node_type(&parameter.value())),
                    });
                }
                _ => {}
            }
        }
        if let Some(keyword_rest) = inner.keyword_rest()
            && let Some(parameter) = keyword_rest.as_keyword_rest_parameter_node()
        {
            params.push(ParamInfo {
                name: parameter
                    .name()
                    .map(|name| String::from_utf8_lossy(name.as_slice()).to_string())
                    .unwrap_or_default(),
                kind: ParamKind::DoubleRest,
                default_type: None,
            });
        }
        params
    }

    pub(super) fn extract_symbol_args(call_node: &ruby_prism::CallNode<'_>) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(args) = call_node.arguments() {
            for arg in args.arguments().iter() {
                if let Node::SymbolNode { .. } = &arg {
                    let sym = arg.as_symbol_node().expect("must be SymbolNode");
                    names.push(String::from_utf8_lossy(sym.unescaped()).to_string());
                }
            }
        }
        names
    }

    pub(super) fn extract_hash_option_node<'b>(
        call_node: &ruby_prism::CallNode<'b>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> Option<Node<'b>> {
        let args = call_node.arguments()?;
        for arg in args.arguments().iter() {
            if let Node::KeywordHashNode { .. } = &arg {
                let kh = arg.as_keyword_hash_node().expect("must be KeywordHashNode");
                for elem in kh.elements().iter() {
                    if let Node::AssocNode { .. } = &elem {
                        let assoc = elem.as_assoc_node().expect("must be AssocNode");
                        let k_name = Self::node_to_symbol_or_label(&assoc.key(), parse_result);
                        if k_name.as_deref() == Some(key) {
                            return Some(assoc.value());
                        }
                    }
                }
            }
        }
        None
    }

    pub(super) fn extract_hash_option_str(
        call_node: &ruby_prism::CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> Option<String> {
        if let Some(args) = call_node.arguments() {
            for arg in args.arguments().iter() {
                if let Node::KeywordHashNode { .. } = &arg {
                    let kh = arg.as_keyword_hash_node().expect("must be KeywordHashNode");
                    for elem in kh.elements().iter() {
                        if let Node::AssocNode { .. } = &elem {
                            let assoc = elem.as_assoc_node().expect("must be AssocNode");
                            let k = &assoc.key();
                            let k_name = Self::node_to_symbol_or_label(k, parse_result);
                            if k_name.as_deref() == Some(key) {
                                return Self::node_to_string_or_symbol(
                                    &assoc.value(),
                                    parse_result,
                                );
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub(super) fn extract_hash_option_bool(
        call_node: &ruby_prism::CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> Option<bool> {
        if let Some(args) = call_node.arguments() {
            for arg in args.arguments().iter() {
                if let Node::KeywordHashNode { .. } = &arg {
                    let kh = arg.as_keyword_hash_node().expect("must be KeywordHashNode");
                    for elem in kh.elements().iter() {
                        if let Node::AssocNode { .. } = &elem {
                            let assoc = elem.as_assoc_node().expect("must be AssocNode");
                            let k = &assoc.key();
                            let k_name = Self::node_to_symbol_or_label(k, parse_result);
                            if k_name.as_deref() == Some(key) {
                                let v = &assoc.value();
                                if let Node::TrueNode { .. } = v {
                                    return Some(true);
                                }
                                if let Node::FalseNode { .. } = v {
                                    return Some(false);
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub(super) fn extract_enum_value_names(
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Vec<String> {
        fn hash_keys_from_elements<'a>(
            elements: impl Iterator<Item = ruby_prism::Node<'a>>,
            parse_result: &ParseResult<'_>,
        ) -> Vec<String> {
            let mut keys = Vec::new();
            for elem in elements {
                if let Node::AssocNode { .. } = &elem {
                    let assoc = elem.as_assoc_node().expect("must be AssocNode");
                    if let Some(name) =
                        InferenceEngine::node_to_string_or_symbol(&assoc.key(), parse_result)
                    {
                        keys.push(name);
                    }
                }
            }
            keys
        }

        // An `enum` value definition can be either a Hash (`{ pending: 0 }`) or an Array (`[:pending, :done]`). For a Hash, the keys are the value names; for an Array, the elements are the value names.
        // To avoid mistaking option assocs like `prefix:` / `suffix:` for values, only look at the value-definition node (the enum attribute's value, or the positional argument right after the attribute).
        fn value_names_from_node(node: &Node<'_>, parse_result: &ParseResult<'_>) -> Vec<String> {
            match node {
                Node::HashNode { .. } => {
                    let hash = node.as_hash_node().expect("must be HashNode");
                    hash_keys_from_elements(hash.elements().iter(), parse_result)
                }
                Node::ArrayNode { .. } => {
                    let array = node.as_array_node().expect("must be ArrayNode");
                    array
                        .elements()
                        .iter()
                        .filter_map(|elem| {
                            InferenceEngine::node_to_string_or_symbol(&elem, parse_result)
                        })
                        .collect()
                }
                _ => Vec::new(),
            }
        }

        let Some(args) = call_node.arguments() else {
            return Vec::new();
        };
        let arg_nodes: Vec<_> = args.arguments().iter().collect();

        if let Some(first) = arg_nodes.first()
            && matches!(first, Node::SymbolNode { .. } | Node::StringNode { .. })
            && let Some(second) = arg_nodes.get(1)
        {
            return value_names_from_node(second, parse_result);
        }

        for arg in &arg_nodes {
            let elements = match arg {
                Node::KeywordHashNode { .. } => Some(
                    arg.as_keyword_hash_node()
                        .expect("must be KeywordHashNode")
                        .elements(),
                ),
                Node::HashNode { .. } => {
                    Some(arg.as_hash_node().expect("must be HashNode").elements())
                }
                _ => None,
            };
            let Some(elements) = elements else {
                continue;
            };
            for elem in elements.iter() {
                if let Node::AssocNode { .. } = &elem {
                    let assoc = elem.as_assoc_node().expect("must be AssocNode");
                    let value = assoc.value();
                    if matches!(&value, Node::HashNode { .. } | Node::ArrayNode { .. }) {
                        return value_names_from_node(&value, parse_result);
                    }
                }
            }
        }
        Vec::new()
    }

    pub(super) fn node_to_symbol_or_label(
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<String> {
        match node {
            Node::SymbolNode { .. } => {
                let sym = node.as_symbol_node().expect("must be SymbolNode");
                Some(String::from_utf8_lossy(sym.unescaped()).to_string())
            }
            _ => {
                let source = parse_result.source();
                let loc = node.location();
                let raw = &source[loc.start_offset()..loc.end_offset()];
                let s = String::from_utf8_lossy(raw).to_string();
                let label = s.trim_end_matches(':');
                if !label.is_empty() && label != s {
                    Some(label.to_string())
                } else {
                    None
                }
            }
        }
    }

    pub(super) fn node_to_string_or_symbol(
        node: &Node<'_>,
        _parse_result: &ParseResult<'_>,
    ) -> Option<String> {
        match node {
            Node::SymbolNode { .. } => {
                let sym = node.as_symbol_node().expect("must be SymbolNode");
                Some(String::from_utf8_lossy(sym.unescaped()).to_string())
            }
            Node::StringNode { .. } => {
                let sn = node.as_string_node().expect("must be StringNode");
                Some(String::from_utf8_lossy(sn.unescaped()).to_string())
            }
            _ => None,
        }
    }

    pub(in crate::inference) fn register_untyped_attribute_accessors(
        &mut self,
        class_name: &str,
        attr_name: &str,
        loc: SourceLocation,
    ) {
        self.register_untyped_attribute_accessors_inner(
            class_name,
            attr_name,
            loc,
            Some(format!("@{attr_name}")),
        );
    }

    pub(in crate::inference) fn register_untyped_virtual_attribute_accessors(
        &mut self,
        class_name: &str,
        attr_name: &str,
        loc: SourceLocation,
    ) {
        let attr_ivar = Some(format!("@{attr_name}"));
        self.register_untyped_attribute_accessors_inner(class_name, attr_name, loc, attr_ivar);
    }

    fn register_untyped_attribute_accessors_inner(
        &mut self,
        class_name: &str,
        attr_name: &str,
        loc: SourceLocation,
        attr_ivar: Option<String>,
    ) {
        let accessor_type = self
            .registry
            .schema_column_accessor_type(class_name, attr_name)
            .unwrap_or(Type::Untyped);
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(attr_name),
                param_infos: Vec::new(),
                raw_return_type: accessor_type.clone(),
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: attr_ivar.clone(),
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
                name: Sym::new(format!("{attr_name}=")),
                param_infos: vec![ParamInfo {
                    name: attr_name.to_string(),
                    kind: ParamKind::Required,
                    default_type: Some(accessor_type.clone()),
                }],
                raw_return_type: accessor_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
    }

    pub(in crate::inference) fn register_typed_virtual_attribute_accessors(
        &mut self,
        class_name: &str,
        attr_name: &str,
        ty: Type,
        loc: SourceLocation,
    ) {
        self.register_typed_attribute_accessors_inner(class_name, attr_name, ty, loc, None);
    }

    pub(in crate::inference) fn register_typed_attribute_accessors_inner(
        &mut self,
        class_name: &str,
        attr_name: &str,
        ty: Type,
        loc: SourceLocation,
        attr_ivar: Option<String>,
    ) {
        let is_virtual_attribute = attr_ivar.is_none();
        if !self
            .registry
            .has_method_variant(class_name, attr_name, false)
        {
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(attr_name),
                    param_infos: Vec::new(),
                    raw_return_type: ty.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: attr_ivar.clone(),
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        } else {
            let widened = if is_virtual_attribute {
                self.registry
                    .lookup_method_return_type(class_name, attr_name)
                    .map(|current| current.union_with(ty.clone()))
                    .unwrap_or_else(|| ty.clone())
            } else {
                ty.clone()
            };
            self.registry
                .update_instance_method_return_type(class_name, attr_name, widened);
        }

        let setter_name = format!("{attr_name}=");
        if !self
            .registry
            .has_method_variant(class_name, &setter_name, false)
        {
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(setter_name),
                    param_infos: vec![ParamInfo {
                        name: attr_name.to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(ty.clone()),
                    }],
                    raw_return_type: ty,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar,
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        } else {
            let widened = if is_virtual_attribute {
                self.registry
                    .lookup_method_return_type(class_name, &setter_name)
                    .map(|current| current.union_with(ty.clone()))
                    .unwrap_or_else(|| ty.clone())
            } else {
                ty.clone()
            };
            self.registry.update_instance_method_return_type(
                class_name,
                &setter_name,
                widened.clone(),
            );
            self.registry
                .update_method_param_default_type(class_name, &setter_name, 0, widened);
        }
    }

    pub(in crate::inference) fn register_dirty_attribute_methods(
        &mut self,
        class_name: &str,
        attr_name: &str,
        accessor_type: &Type,
        loc: SourceLocation,
    ) {
        let history_type = match accessor_type {
            Type::Union(parts) if parts.iter().any(|part| matches!(part, Type::Nil)) => {
                accessor_type.clone()
            }
            other => Type::Union(vec![other.clone(), Type::Nil]),
        };
        let change_type = Type::Tuple(vec![history_type.clone(), history_type.clone()]);
        let maybe_change_type = Type::Union(vec![
            Type::Array(Some(Box::new(history_type.clone()))),
            Type::Nil,
        ]);

        for name in [
            format!("{attr_name}_changed?"),
            format!("{attr_name}_previously_changed?"),
            format!("saved_change_to_{attr_name}?"),
            format!("will_save_change_to_{attr_name}?"),
        ] {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(name),
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
        }

        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(format!("{attr_name}_change")),
                param_infos: Vec::new(),
                raw_return_type: change_type,
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

        for name in [
            format!("{attr_name}_was"),
            format!("{attr_name}_previously_was"),
            format!("{attr_name}_before_last_save"),
            format!("{attr_name}_in_database"),
        ] {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(name),
                    param_infos: Vec::new(),
                    raw_return_type: history_type.clone(),
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

        for name in [
            format!("{attr_name}_previous_change"),
            format!("{attr_name}_change_to_be_saved"),
            format!("saved_change_to_{attr_name}"),
        ] {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(name),
                    param_infos: Vec::new(),
                    raw_return_type: maybe_change_type.clone(),
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

        for name in [
            format!("{attr_name}_will_change!"),
            format!("restore_{attr_name}!"),
            format!("clear_{attr_name}_change"),
        ] {
            self.registry.add_method_def_if_missing(
                class_name,
                MethodDef {
                    name: Sym::new(name),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Void,
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

    fn collect_attributes_from_super_call(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
        loc: SourceLocation,
    ) {
        match node {
            Node::StatementsNode { .. } => {
                let statements = node.as_statements_node().expect("must be StatementsNode");
                for stmt in statements.body().iter() {
                    self.collect_attributes_from_super_call(
                        class_name,
                        &stmt,
                        parse_result,
                        scope,
                        loc,
                    );
                }
            }
            Node::SuperNode { .. } => {
                let super_node = node.as_super_node().expect("must be SuperNode");
                let Some(args) = super_node.arguments() else {
                    return;
                };
                for arg in args.arguments().iter() {
                    if let Node::KeywordHashNode { .. } = &arg {
                        let kh = arg.as_keyword_hash_node().expect("must be KeywordHashNode");
                        for elem in kh.elements().iter() {
                            if let Node::AssocNode { .. } = &elem {
                                let assoc = elem.as_assoc_node().expect("must be AssocNode");
                                let Some(attr_name) =
                                    Self::node_to_symbol_or_label(&assoc.key(), parse_result)
                                else {
                                    continue;
                                };
                                let ty = self.infer_node_type(
                                    class_name,
                                    &assoc.value(),
                                    parse_result,
                                    scope,
                                );
                                self.register_typed_virtual_attribute_accessors(
                                    class_name, &attr_name, ty, loc,
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn refine_active_model_attributes_from_initialize(
        &mut self,
        class_name: &str,
        body: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
        loc: SourceLocation,
    ) {
        if !self.is_active_model_serializers_model_class(class_name)
            && !self.is_active_model_serializer_class(class_name)
        {
            return;
        }
        self.collect_attributes_from_super_call(class_name, body, parse_result, scope, loc);
    }

    pub(in crate::inference) fn extract_enum_attribute_name(
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<String> {
        let args = call_node.arguments()?;
        let arg_nodes: Vec<_> = args.arguments().iter().collect();
        let first = arg_nodes.first()?;

        if let Some(name) = Self::node_to_string_or_symbol(first, parse_result) {
            return Some(name);
        }

        match first {
            Node::KeywordHashNode { .. } => {
                let kh = first
                    .as_keyword_hash_node()
                    .expect("must be KeywordHashNode");
                for elem in kh.elements().iter() {
                    if let Node::AssocNode { .. } = &elem {
                        let assoc = elem.as_assoc_node().expect("must be AssocNode");
                        if let Some(name) =
                            Self::node_to_symbol_or_label(&assoc.key(), parse_result)
                        {
                            return Some(name);
                        }
                    }
                }
                None
            }
            Node::HashNode { .. } => {
                let hash = first.as_hash_node().expect("must be HashNode");
                for elem in hash.elements().iter() {
                    if let Node::AssocNode { .. } = &elem {
                        let assoc = elem.as_assoc_node().expect("must be AssocNode");
                        if let Some(name) =
                            Self::node_to_string_or_symbol(&assoc.key(), parse_result)
                        {
                            return Some(name);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub(in crate::inference) fn enum_method_affix(
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        option_name: &str,
        enum_attr_name: &str,
    ) -> Option<String> {
        let legacy_option_name = format!("_{option_name}");
        for name in [option_name, legacy_option_name.as_str()] {
            if let Some(custom) = Self::extract_hash_option_str(call_node, name, parse_result) {
                return Some(custom);
            }
            if Self::extract_hash_option_bool(call_node, name, parse_result).unwrap_or(false) {
                return Some(enum_attr_name.to_string());
            }
        }
        None
    }

    pub(in crate::inference) fn decorate_enum_method_name(
        value_name: &str,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) -> String {
        let mut parts = Vec::new();
        if let Some(prefix) = prefix
            && !prefix.is_empty()
        {
            parts.push(prefix.to_string());
        }
        parts.push(value_name.to_string());
        if let Some(suffix) = suffix
            && !suffix.is_empty()
        {
            parts.push(suffix.to_string());
        }
        parts.join("_")
    }

    fn node_to_string_or_constant_name(
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Option<String> {
        if let Some(name) = Self::node_to_string_or_symbol(node, parse_result) {
            return Some(name);
        }
        match node {
            Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. } => {
                Some(String::from_utf8_lossy(node.location().as_slice()).to_string())
            }
            _ => None,
        }
    }

    pub(in crate::inference) fn extract_hash_option_names(
        call_node: &ruby_prism::CallNode<'_>,
        key: &str,
        parse_result: &ParseResult<'_>,
    ) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(args) = call_node.arguments() {
            for arg in args.arguments().iter() {
                if let Node::KeywordHashNode { .. } = &arg {
                    let kh = arg.as_keyword_hash_node().expect("must be KeywordHashNode");
                    for elem in kh.elements().iter() {
                        if let Node::AssocNode { .. } = &elem {
                            let assoc = elem.as_assoc_node().expect("must be AssocNode");
                            let k_name = Self::node_to_symbol_or_label(&assoc.key(), parse_result);
                            if k_name.as_deref() != Some(key) {
                                continue;
                            }
                            match assoc.value() {
                                Node::ArrayNode { .. } => {
                                    let array =
                                        assoc.value().as_array_node().expect("must be ArrayNode");
                                    for item in array.elements().iter() {
                                        if let Some(name) = Self::node_to_string_or_constant_name(
                                            &item,
                                            parse_result,
                                        ) {
                                            names.push(name);
                                        }
                                    }
                                }
                                other => {
                                    if let Some(name) =
                                        Self::node_to_string_or_constant_name(&other, parse_result)
                                    {
                                        names.push(name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        names
    }

    pub(in crate::inference) fn snake_case_type_name(type_name: &str) -> String {
        let mut out = String::new();
        let mut prev_lower_or_digit = false;
        for ch in type_name.replace("::", "_").chars() {
            if ch.is_ascii_uppercase() {
                if prev_lower_or_digit && !out.ends_with('_') {
                    out.push('_');
                }
                out.push(ch.to_ascii_lowercase());
                prev_lower_or_digit = false;
            } else {
                out.push(ch);
                prev_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
            }
        }
        out
    }

    pub(in crate::inference) fn pluralize_name(name: &str) -> String {
        if name.ends_with('y')
            && !matches!(name.chars().nth_back(1), Some('a' | 'e' | 'i' | 'o' | 'u'))
        {
            format!("{}ies", &name[..name.len() - 1])
        } else if name.ends_with('s')
            || name.ends_with("sh")
            || name.ends_with("ch")
            || name.ends_with('x')
            || name.ends_with('z')
        {
            format!("{name}es")
        } else {
            format!("{name}s")
        }
    }

    pub(in crate::inference) fn delegate_target_type(
        &self,
        class_name: &str,
        target: &str,
    ) -> Type {
        if target.starts_with('@') {
            self.registry
                .lookup_ivar_type(class_name, target)
                .unwrap_or_else(|| Type::IvarRef(Sym::new(target)))
        } else {
            self.registry
                .lookup_ivar_type(class_name, &format!("@{target}"))
                .unwrap_or_else(|| Type::MethodReturnRef(class_name.into(), target.into()))
        }
    }

    pub(in crate::inference) fn delegate_param_signature(
        &self,
        target_obj_type: &Type,
        method_name: &str,
    ) -> (Vec<String>, Vec<ParamInfo>) {
        if let Some(sig) = self.delegate_target_method_sig(target_obj_type, method_name) {
            let param_names = sig.params.iter().map(|param| param.name.clone()).collect();
            let param_infos = sig
                .params
                .iter()
                .map(|param| ParamInfo {
                    name: param.name.clone(),
                    kind: param.kind,
                    default_type: Some(param.param_type.clone()),
                })
                .collect();
            return (param_names, param_infos);
        }

        if method_name == "[]" || method_name == "[]=" || method_name.ends_with('=') {
            (
                Vec::new(),
                vec![
                    ParamInfo {
                        name: "args".to_string(),
                        kind: ParamKind::Rest,
                        default_type: Some(Type::Untyped),
                    },
                    ParamInfo {
                        name: "kwargs".to_string(),
                        kind: ParamKind::DoubleRest,
                        default_type: Some(Type::Untyped),
                    },
                ],
            )
        } else {
            (Vec::new(), Vec::new())
        }
    }

    pub(in crate::inference) fn delegate_target_method_sig(
        &self,
        target_obj_type: &Type,
        method_name: &str,
    ) -> Option<MethodSig> {
        let resolved_target = self.resolve_delegate_target_type(target_obj_type);
        self.delegate_target_method_sig_for_resolved_type(&resolved_target, method_name)
    }

    fn delegate_target_method_sig_for_resolved_type(
        &self,
        target_type: &Type,
        method_name: &str,
    ) -> Option<MethodSig> {
        if let Type::Union(parts) = target_type {
            let mut signatures = Vec::new();
            for part in parts {
                if matches!(part, Type::Nil) {
                    continue;
                }
                signatures
                    .push(self.delegate_target_method_sig_for_resolved_type(part, method_name)?);
            }
            let mut merged = signatures.pop()?;
            for signature in signatures {
                if signature.params.len() != merged.params.len() {
                    return None;
                }
                for (merged_param, param) in merged.params.iter_mut().zip(signature.params) {
                    if merged_param.kind != param.kind {
                        return None;
                    }
                    merged_param.param_type =
                        merged_param.param_type.clone().union_with(param.param_type);
                }
                merged.return_type = merged.return_type.union_with(signature.return_type);
            }
            return Some(merged);
        }

        let receiver_class = self.type_to_class_name(target_type)?;
        let prefer_singleton = matches!(target_type, Type::Singleton(_));
        self.registry.lookup_method_sig_for_receiver_with_hint(
            &receiver_class,
            method_name,
            prefer_singleton,
        )
    }

    pub(in crate::inference) fn resolve_delegate_target_type(&self, ty: &Type) -> Type {
        match ty {
            Type::MethodReturnRef(class_name, method_name) => self
                .registry
                .lookup_method_return_type(class_name, method_name)
                .unwrap_or_else(|| ty.clone()),
            Type::Union(parts) => Type::from_type_vec(
                parts
                    .iter()
                    .map(|part| self.resolve_delegate_target_type(part))
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    pub(in crate::inference) fn delegate_target_may_be_nil(&self, target_obj_type: &Type) -> bool {
        match self.resolve_delegate_target_type(target_obj_type) {
            Type::Nil => true,
            Type::Union(parts) => parts
                .iter()
                .any(|part| self.delegate_target_may_be_nil(part)),
            Type::Untyped
            | Type::ParamRef(_)
            | Type::KeywordParamRef(_)
            | Type::IvarRef(_)
            | Type::MethodReturnRef(_, _)
            | Type::ReceiverMethodRef(_, _) => true,
            other => Self::contains_nil(&other),
        }
    }

    pub(in crate::inference) fn delegate_target_is_known(&self, target_obj_type: &Type) -> bool {
        match self.resolve_delegate_target_type(target_obj_type) {
            Type::Untyped
            | Type::ParamRef(_)
            | Type::KeywordParamRef(_)
            | Type::IvarRef(_)
            | Type::MethodReturnRef(_, _)
            | Type::ReceiverMethodRef(_, _) => false,
            Type::Union(parts) => parts.iter().any(|part| self.delegate_target_is_known(part)),
            _ => true,
        }
    }

    pub(in crate::inference) fn infer_alias_attribute_type(
        &self,
        class_name: &str,
        old_name: &str,
    ) -> Option<Type> {
        if let Some(assoc_name) = old_name.strip_suffix("_id")
            && self
                .registry
                .lookup_method_sig(class_name, assoc_name)
                .is_some()
        {
            return Some(Type::Integer);
        }
        if let Some(assoc_name) = old_name.strip_suffix("_type")
            && self
                .registry
                .lookup_method_sig(class_name, assoc_name)
                .is_some()
        {
            return Some(Type::String);
        }
        None
    }
}
