use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::{Sym, Type};
use ruby_prism::{CallNode, ParseResult};

const CLASS_BODY_METHODS: &[&str] = &[
    "helper_method",
    "before_action",
    "after_action",
    "around_action",
    "skip_before_action",
    "skip_after_action",
    "skip_around_action",
    "prepend_before_action",
    "prepend_after_action",
    "prepend_around_action",
    "append_before_action",
    "append_after_action",
    "append_around_action",
    "allow_browser",
    "layout",
    "helper",
    "default",
    "protect_from_forgery",
    "skip_forgery_protection",
    "wrap_parameters",
    "add_flash_types",
    "force_ssl",
    "clear_helpers",
    "default_form_builder",
    "respond_to",
];

pub(super) struct ActionController;

static MANIFEST: PluginManifest = PluginManifest {
    id: "action_controller",
    features: &[
        DslFeature {
            library: DslLibrary::ActionControllerHelpers,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::ActionMailer,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::UrlHelpers,
            gem_markers: &[],
        },
        DslFeature {
            library: DslLibrary::RailsGenerators,
            gem_markers: &[],
        },
    ],
    base_classes: &[],
    rails_default: true,
};

impl Plugin for ActionController {
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
        if !cx.dsl_enabled(DslLibrary::ActionControllerHelpers) {
            return false;
        }
        match method_name {
            "helper_method" => {
                cx.collect_helper_method(class_name, call_node, parse_result);
                true
            }
            "before_action" | "after_action" | "around_action" | "skip_before_action"
            | "skip_after_action" | "skip_around_action" | "allow_browser" => {
                cx.collect_action_callback(class_name, call_node, parse_result);
                true
            }
            _ => false,
        }
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        CLASS_BODY_METHODS
    }
}

fn is_controller_context(engine: &PluginCx<'_, '_>, class_name: &str) -> bool {
    engine.is_action_controller_class(class_name)
        || (class_name.ends_with("Helper") && engine.rails_feature_enabled())
        || engine.class_or_ancestors_include_module(class_name, "ActionController::Helpers")
}

const MAILER_BASES: &[&str] = &["ActionMailer::Base", "ApplicationMailer"];

fn response_method_return(method_name: &str) -> Option<Type> {
    match method_name {
        "status" => Some(Type::Integer),
        "body" | "code" | "message" | "status_message" => Some(Type::String),
        "media_type" | "content_type" | "charset" => Some(Type::String.union_with(Type::Nil)),
        "headers" | "cookies" | "set_header" | "get_header" | "location" => Some(Type::Untyped),
        "successful?" | "redirection?" | "committed?" | "sending?" | "sent?" => Some(Type::Bool),
        _ => None,
    }
}

fn message_delivery_method_return(method_name: &str) -> Option<Type> {
    match method_name {
        "message" | "deliver_now" | "deliver_now!" | "deliver_later" | "deliver_later!" => {
            Some(Type::Untyped)
        }
        "processed?" => Some(Type::Bool),
        _ => None,
    }
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    if let Type::Singleton(class_name) = receiver_type {
        if method_name == "helpers"
            && engine.dsl_enabled(DslLibrary::ActionControllerHelpers)
            && engine.is_action_controller_class(class_name)
        {
            return Some(Type::Untyped);
        }
        if engine.rails_feature_enabled()
            && (class_name.ends_with("Mailer")
                || engine.class_matches_or_inherits(class_name, MAILER_BASES))
            && engine
                .registry()
                .lookup_method_return_type_with_hint(class_name, method_name, false)
                .is_some()
        {
            return Some(Type::Class(Sym::new("ActionMailer::MessageDelivery")));
        }
        return None;
    }
    let Type::Class(class_name) = receiver_type else {
        return None;
    };
    if class_name.as_str() == "ActionDispatch::Response" {
        if !engine.dsl_enabled(DslLibrary::ActionControllerHelpers) {
            return None;
        }
        return response_method_return(method_name);
    }
    if class_name.as_str() == "ActionMailer::MessageDelivery" {
        if !engine.dsl_enabled(DslLibrary::ActionControllerHelpers) {
            return None;
        }
        return message_delivery_method_return(method_name);
    }
    if engine.rails_feature_enabled()
        && (engine.class_matches_or_inherits(class_name, MAILER_BASES)
            || class_name.ends_with("Mailer"))
    {
        match method_name {
            "headers" | "attachments" | "mail" | "message" | "render" | "logger" | "helpers" => {
                return Some(Type::Untyped);
            }
            "render_to_string" => return Some(Type::String),
            "t" | "translate" | "l" | "localize" => return Some(Type::String),
            "locale" => return Some(Type::Symbol),
            name if (name.ends_with("_path") || name.ends_with("_url")) && name.len() > 5 => {
                return Some(Type::String);
            }
            _ => {}
        }
    }
    let result = match method_name {
        "request" => Type::Class(Sym::new("ActionDispatch::Request")),
        "response" => Type::Class(Sym::new("ActionDispatch::Response")),
        "flash" => Type::Class(Sym::new("ActionDispatch::Flash::FlashHash")),
        "t" | "translate" | "l" | "localize" => Type::String,
        "locale" => Type::Symbol,
        "render_to_string" => Type::String,
        "render" => Type::Untyped,
        "gon" => Type::Untyped,
        "action_name" | "controller_name" | "controller_path" => Type::String,
        "url_for" => Type::String,
        "performed?" => Type::Bool,
        "stale?" => Type::Bool,
        // Authorization predicates (CanCan / policy frameworks mix these in).
        "can?" | "cannot?" => Type::Bool,
        "content_tag"
        | "tag"
        | "link_to"
        | "button_to"
        | "image_tag"
        | "sanitize"
        | "safe_join"
        | "asset_path"
        | "asset_url"
        | "image_path"
        | "number_to_human"
        | "number_to_human_size"
        | "number_with_delimiter"
        | "time_ago_in_words"
        | "pluralize"
        | "truncate"
        | "simple_format" => Type::String,
        "session"
        | "cookies"
        | "headers"
        | "helpers"
        | "expires_in"
        | "expires_now"
        | "fresh_when"
        | "logger"
        | "request_forgery_protection_token" => Type::Untyped,
        name if (name.ends_with("_path") || name.ends_with("_url")) && name.len() > 5 => {
            Type::String
        }
        _ => return None,
    };
    if !engine.dsl_enabled(DslLibrary::ActionControllerHelpers) {
        return None;
    }
    is_controller_context(engine, class_name).then_some(result)
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(super) fn collect_helper_method(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let names = Self::extract_symbol_args(call_node);
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        for name in &names {
            self.registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(name),
                    param_infos: Vec::new(),
                    raw_return_type: Type::Untyped,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: false,
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

    pub(super) fn collect_action_callback(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        _parse_result: &ParseResult<'_>,
    ) {
        for name in Self::extract_symbol_args(call_node) {
            let _ = self.registry.has_method_named(class_name, &name);
        }
    }
}
