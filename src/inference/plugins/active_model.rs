use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::{SourceLocation, Sym, Type};
use ruby_prism::{CallNode, ParseResult};

const CLASS_BODY_METHODS: &[&str] = &[
    "attribute",
    "attributes",
    "has_secure_password",
    "validates_confirmation_of",
    "validates",
    "define_model_callbacks",
];

pub(super) struct ActiveModel;

static MANIFEST: PluginManifest = PluginManifest {
    id: "active_model",
    features: &[
        DslFeature {
            library: DslLibrary::ActiveModelValidations,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveModelAttributes,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveModelSecurePassword,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActiveModelValidationsConfirmation,
            gem_markers: &[],
        },
    ],
    base_classes: &[],
    rails_default: true,
};

impl Plugin for ActiveModel {
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
            "attribute"
                if cx.dsl_enabled(DslLibrary::ActiveModelAttributes)
                    || cx.dsl_enabled(DslLibrary::ActiveSupportCurrentAttributes) =>
            {
                cx.collect_attribute_dsl(class_name, call_node, parse_result);
                cx.collect_current_attributes_attribute_dsl(class_name, call_node, parse_result);
                true
            }
            "attributes" => {
                cx.collect_attributes_dsl(class_name, call_node, parse_result);
                true
            }
            "has_secure_password" if cx.dsl_enabled(DslLibrary::ActiveModelSecurePassword) => {
                cx.collect_secure_password_dsl(class_name, call_node, parse_result);
                true
            }
            "validates_confirmation_of"
                if cx.dsl_enabled(DslLibrary::ActiveModelValidationsConfirmation) =>
            {
                cx.collect_validates_confirmation_of_dsl(class_name, call_node, parse_result);
                true
            }
            "validates" if cx.dsl_enabled(DslLibrary::ActiveModelValidationsConfirmation) => {
                cx.collect_validates_confirmation_dsl(class_name, call_node, parse_result);
                cx.collect_validates_presence_dsl(class_name, call_node, parse_result);
                true
            }
            "define_model_callbacks" if cx.rails_feature_enabled() => {
                cx.collect_define_model_callbacks(class_name, call_node, parse_result);
                true
            }
            _ => false,
        }
    }

    fn register_on_mixin(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        if module_name == "ActiveModel::Callbacks" && cx.rails_feature_enabled() {
            cx.add_run_callbacks_method(class_name, loc);
        }
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        CLASS_BODY_METHODS
    }

    fn class_body_method_prefixes(&self) -> &'static [&'static str] {
        // `validates_presence_of`, `validates_length_of`, … — recognized for
        // diagnostics suppression even though only `validates` has a collector.
        &["validates_"]
    }
}

const ACTIVE_MODEL_MODULES: &[&str] = &[
    "ActiveModel::Model",
    "ActiveModel::Validations",
    "ActiveModel::API",
    "ActiveModel::Attributes",
];

fn is_active_model_class(engine: &PluginCx<'_, '_>, class_name: &str) -> bool {
    engine.is_active_record_model_class(class_name)
        || engine.is_active_model_serializers_model_class(class_name)
        || ACTIVE_MODEL_MODULES
            .iter()
            .any(|module| engine.class_or_ancestors_include_module(class_name, module))
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let Type::Class(class_name) = receiver_type else {
        return None;
    };
    if class_name.as_str() == "ActiveModel::Errors" {
        if !engine.dsl_enabled(DslLibrary::ActiveModelValidations) {
            return None;
        }
        return errors_method_return(method_name);
    }
    let result = match method_name {
        "errors" => Type::Class(Sym::new("ActiveModel::Errors")),
        "valid?" | "invalid?" | "validate" | "validate!" => Type::Bool,
        "read_attribute_for_validation" => Type::Untyped,
        _ => return None,
    };
    if !engine.dsl_enabled(DslLibrary::ActiveModelValidations) {
        return None;
    }
    is_active_model_class(engine, class_name).then_some(result)
}

fn errors_method_return(method_name: &str) -> Option<Type> {
    let string_array = || Type::Array(Some(Box::new(Type::String)));
    match method_name {
        "add" | "import" | "merge!" | "clear" | "delete" | "details" | "messages"
        | "group_by_attribute" | "each" | "objects" | "where" | "include?" => Some(Type::Untyped),
        "full_messages" | "full_messages_for" | "to_a" => Some(string_array()),
        "full_message" | "generate_message" => Some(Type::String),
        "any?" | "empty?" | "blank?" | "present?" | "added?" | "of_kind?" | "has_key?" | "key?" => {
            Some(Type::Bool)
        }
        "size" | "count" => Some(Type::Integer),
        "[]" | "messages_for" => Some(Type::Array(Some(Box::new(Type::Untyped)))),
        "attribute_names" => Some(Type::Array(Some(Box::new(Type::Symbol)))),
        "to_hash" | "as_json" => Some(Type::Hash(
            Some(Box::new(Type::Symbol)),
            Some(Box::new(Type::Untyped)),
        )),
        _ => None,
    }
}

use super::super::*;
use crate::registry::IncluderBoundDsl;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_define_model_callbacks(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let names = Self::extract_symbol_args(call_node);
        if names.is_empty() {
            return;
        }
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        // define_model_callbacks itself requires run_callbacks, so add it defensively
        // (covers the case where `extend` is invisible from another file).
        self.add_run_callbacks_method(class_name, loc);
        let kinds = self.callback_registrar_kinds(call_node, parse_result);
        for name in &names {
            for kind in &kinds {
                let method_name = format!("{kind}_{name}");
                self.add_callback_registrar_method(class_name, &method_name, loc);
            }
        }
    }

    pub(in crate::inference) fn add_run_callbacks_method(
        &mut self,
        class_name: &str,
        loc: SourceLocation,
    ) {
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("run_callbacks"),
                param_infos: vec![ParamInfo {
                    name: "kind".to_string(),
                    kind: ParamKind::Required,
                    default_type: Some(Type::Symbol),
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

    fn add_callback_registrar_method(
        &mut self,
        class_name: &str,
        method_name: &str,
        loc: SourceLocation,
    ) {
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(method_name),
                param_infos: vec![ParamInfo {
                    name: "args".to_string(),
                    kind: ParamKind::Rest,
                    default_type: Some(Type::Untyped),
                }],
                raw_return_type: Type::Untyped,
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

    fn callback_registrar_kinds(
        &self,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) -> Vec<String> {
        let all = || {
            vec![
                "before".to_string(),
                "after".to_string(),
                "around".to_string(),
            ]
        };
        let Some(only_node) = Self::extract_hash_option_node(call_node, "only", parse_result)
        else {
            return all();
        };
        let requested: Vec<String> = match &only_node {
            Node::ArrayNode { .. } => only_node
                .as_array_node()
                .expect("must be ArrayNode")
                .elements()
                .iter()
                .filter_map(|elem| Self::extract_symbol_literal_name(&elem))
                .collect(),
            _ => Self::extract_symbol_literal_name(&only_node)
                .into_iter()
                .collect(),
        };
        let filtered: Vec<String> = requested
            .into_iter()
            .filter(|kind| matches!(kind.as_str(), "before" | "after" | "around"))
            .collect();
        if filtered.is_empty() { all() } else { filtered }
    }

    pub(in crate::inference) fn collect_secure_password_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let password_name = self
            .first_symbol_or_string_arg(call_node)
            .unwrap_or_else(|| "password".to_string());
        let confirmation_name = format!("{password_name}_confirmation");
        let challenge_name = format!("{password_name}_challenge");
        self.add_accessor_methods(class_name, &password_name, Type::String, false, loc);
        self.add_accessor_methods(class_name, &confirmation_name, Type::String, false, loc);
        self.add_accessor_methods(class_name, &challenge_name, Type::String, false, loc);
        self.add_simple_method_if_missing(
            class_name,
            &format!("{password_name}_salt"),
            Type::String,
            false,
            loc,
        );
        self.add_method_with_param_if_missing(
            class_name,
            "authenticate",
            "unencrypted_password",
            Type::String,
            Type::Union(vec![Type::SelfType, Type::Bool]),
            false,
            loc,
        );
        self.add_method_with_param_if_missing(
            class_name,
            &format!("authenticate_{password_name}"),
            "unencrypted_password",
            Type::String,
            Type::Union(vec![Type::SelfType, Type::Bool]),
            false,
            loc,
        );
        if self.rails_at_least(7, 1) {
            self.add_method_with_param_if_missing(
                class_name,
                "authenticate_by",
                "attributes",
                Type::Untyped,
                Type::Union(vec![Type::SelfType, Type::Nil]),
                true,
                loc,
            );
        }
    }

    pub(in crate::inference) fn collect_validates_confirmation_of_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        for name in self.symbol_or_string_args(call_node) {
            let confirmation = format!("{name}_confirmation");
            self.add_accessor_methods(class_name, &confirmation, Type::Untyped, false, loc);
        }
    }

    pub(in crate::inference) fn collect_validates_confirmation_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let names = self.symbol_or_string_args(call_node);
        if !self.hash_option_bool(call_node, "confirmation", parse_result) {
            return;
        }
        for name in names {
            let confirmation = format!("{name}_confirmation");
            self.add_accessor_methods(class_name, &confirmation, Type::Untyped, false, loc);
        }
    }

    pub(in crate::inference) fn collect_validates_presence_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !Self::extract_hash_option_bool(call_node, "presence", parse_result).unwrap_or(false) {
            return;
        }
        for name in self.symbol_or_string_args(call_node) {
            let setter_name = format!("{name}=");
            let Some(current_type) = self.registry.lookup_method_return_type(class_name, &name)
            else {
                continue;
            };
            let tightened = Self::strip_nil_type(current_type);
            self.registry
                .update_instance_method_return_type(class_name, &name, tightened.clone());
            self.registry.update_instance_method_return_type(
                class_name,
                &setter_name,
                tightened.clone(),
            );
            self.registry
                .update_method_param_default_type(class_name, &setter_name, 0, tightened);
        }
    }

    fn strip_nil_type(ty: Type) -> Type {
        match ty {
            Type::Union(parts) => {
                let filtered: Vec<Type> = parts
                    .into_iter()
                    .filter(|part| !matches!(part, Type::Nil))
                    .collect();
                match filtered.len() {
                    0 => Type::Untyped,
                    1 => filtered.into_iter().next().expect("single type"),
                    _ => Type::Union(filtered),
                }
            }
            other => other,
        }
    }
}

impl<'a> InferenceEngine<'a> {
    fn attribute_accessor_type(declared_type: Option<Type>, has_default: bool) -> Type {
        match declared_type {
            Some(ty) if has_default => ty,
            Some(Type::Untyped) => Type::Untyped,
            Some(ty) => Type::Union(vec![ty, Type::Nil]),
            None => Type::Untyped,
        }
    }

    fn resolve_attribute_accessor_type(
        &self,
        owner: &str,
        attr_name: &str,
        declared_type: Option<Type>,
        has_default: bool,
    ) -> Type {
        if declared_type.is_some() {
            return Self::attribute_accessor_type(declared_type, has_default);
        }
        self.registry
            .schema_column_accessor_type(owner, attr_name)
            .unwrap_or(Type::Untyped)
    }

    pub(super) fn collect_attribute_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.rails_at_least(5, 0) {
            return;
        }
        let Some(args) = call_node.arguments() else {
            return;
        };
        let Some(first_arg) = args.arguments().iter().next() else {
            return;
        };
        let Some(attr_name) = Self::node_to_string_or_symbol(&first_arg, parse_result) else {
            return;
        };
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let declared_type = self.attribute_declared_type(call_node, parse_result);
        let has_default = self.has_hash_option(call_node, "default", parse_result);

        if self.is_active_record_model_class(class_name) {
            let accessor_type = self.resolve_attribute_accessor_type(
                class_name,
                &attr_name,
                declared_type.clone(),
                has_default,
            );
            self.register_typed_attribute_accessors_inner(
                class_name,
                &attr_name,
                accessor_type.clone(),
                loc,
                Some(format!("@{attr_name}")),
            );
            self.register_dirty_attribute_methods(class_name, &attr_name, &accessor_type, loc);
            return;
        }

        if self.class_or_ancestors_include_module(class_name, "ActiveModel::Attributes") {
            let accessor_type = self.resolve_attribute_accessor_type(
                class_name,
                &attr_name,
                declared_type,
                has_default,
            );
            self.register_typed_virtual_attribute_accessors(
                class_name,
                &attr_name,
                accessor_type,
                loc,
            );
            return;
        }

        if self.is_collecting_concern_included() {
            let accessor_type = self.resolve_attribute_accessor_type(
                class_name,
                &attr_name,
                declared_type,
                has_default,
            );
            self.with_concern_included_synthetic_marking(class_name, |engine| {
                engine.register_typed_attribute_accessors_inner(
                    class_name,
                    &attr_name,
                    accessor_type.clone(),
                    loc,
                    Some(format!("@{attr_name}")),
                );
                engine.register_dirty_attribute_methods(
                    class_name,
                    &attr_name,
                    &accessor_type,
                    loc,
                );
            });
        }
    }

    pub(super) fn collect_attributes_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if self.is_collecting_concern_included() {
            let names = Self::extract_symbol_args(call_node);
            let loc =
                offset_to_location(parse_result.source(), call_node.location().start_offset());
            if !names.is_empty() {
                self.registry.push_includer_bound_dsl(
                    class_name,
                    IncluderBoundDsl::AmsModelAttributes { names, loc },
                );
            }
        }
        if !self.is_active_model_serializers_model_class(class_name) {
            return;
        }
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        for attr_name in Self::extract_symbol_args(call_node) {
            self.register_untyped_virtual_attribute_accessors(class_name, &attr_name, loc);
        }
    }
}
