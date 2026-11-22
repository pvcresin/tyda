use super::super::{NarrowSubject, Scope};
use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::prism_compat::LocationExt;
use crate::project::DslLibrary;
use crate::types::MethodOrigin;
use crate::types::{Sym, Type};
use ruby_prism::{CallNode, Node, ParseResult, Visit};

const TAG_BUILDER: &str = "ActionView::Helpers::TagHelper::TagBuilder";
const MESSAGE_DELIVERY: &str = "ActionMailer::MessageDelivery";
const PARAMETERIZED_MAILER: &str = "ActionMailer::Parameterized::Mailer";
const PARAMETERIZED_DELIVERY: &str = "ActionMailer::Parameterized::MessageDelivery";

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

pub(super) fn is_controller_context(engine: &PluginCx<'_, '_>, class_name: &str) -> bool {
    engine.is_action_controller_class(class_name)
        || (class_name.ends_with("Helper") && engine.rails_feature_enabled())
        || engine.class_or_ancestors_include_module(class_name, "ActionController::Helpers")
}

const MAILER_BASES: &[&str] = &["ActionMailer::Base", "ApplicationMailer"];

fn response_method_return(method_name: &str) -> Option<Type> {
    match method_name {
        "status" => Some(Type::Integer),
        // `charset` falls back to the default charset.
        "body" | "code" | "message" | "status_message" | "charset" => Some(Type::String),
        "media_type" | "content_type" => Some(Type::String.union_with(Type::Nil)),
        "headers" | "cookies" | "set_header" | "get_header" | "location" => Some(Type::Untyped),
        "successful?" | "redirection?" | "committed?" | "sending?" | "sent?" => Some(Type::Bool),
        _ => None,
    }
}

/// A mailer action has a class-level proxy that returns a message delivery.
fn is_mailer_action(engine: &PluginCx<'_, '_>, method_name: &str) -> bool {
    let registry = engine.registry();
    let delivery = Type::Class(Sym::new(MESSAGE_DELIVERY));
    registry
        .user_defined_class_names_unsorted()
        .iter()
        .any(|class_name| {
            registry
                .lookup_method_def(class_name, method_name, true)
                .is_some_and(|method| method.raw_return_type == delivery)
        })
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
        {
            if method_name == "with" {
                return Some(Type::Class(Sym::new(PARAMETERIZED_MAILER)));
            }
            if engine
                .registry()
                .lookup_method_return_type_with_hint(class_name, method_name, false)
                .is_some()
            {
                return Some(Type::Class(Sym::new(MESSAGE_DELIVERY)));
            }
        }
        return None;
    }
    let Type::Class(class_name) = receiver_type else {
        return None;
    };
    // `Mailer.with(...)` forgets which mailer it wraps, so any mailer action name is accepted.
    if class_name.as_str() == PARAMETERIZED_MAILER {
        return is_mailer_action(engine, method_name)
            .then(|| Type::Class(Sym::new(PARAMETERIZED_DELIVERY)));
    }
    if class_name.as_str() == TAG_BUILDER && engine.dsl_enabled(DslLibrary::ActionControllerHelpers)
    {
        return Some(Type::String);
    }
    if class_name.as_str() == "ActionDispatch::Response" {
        if !engine.dsl_enabled(DslLibrary::ActionControllerHelpers) {
            return None;
        }
        return response_method_return(method_name);
    }
    if matches!(
        class_name.as_str(),
        MESSAGE_DELIVERY | PARAMETERIZED_DELIVERY
    ) {
        if !engine.dsl_enabled(DslLibrary::ActionControllerHelpers) {
            return None;
        }
        return engine.engine.message_delivery_method_return(method_name);
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
            // `t` can return a Hash or an Array for a scope key.
            "t" | "translate" => return Some(Type::Untyped),
            "l" | "localize" => return Some(Type::String),
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
        "t" | "translate" => Type::Untyped,
        "l" | "localize" => Type::String,
        "locale" => Type::Symbol,
        "render_to_string" => Type::String,
        "render" => Type::Untyped,
        "gon" => Type::Untyped,
        "action_name" | "controller_name" | "controller_path" => Type::String,
        "url_for" => Type::String,
        "performed?" => Type::Bool,
        "stale?" => Type::Bool,
        // Authorization predicates come from a gem or the app; `bool` only with CanCanCan.
        "can?" | "cannot?" => Type::Untyped,
        "tag" => Type::Class(Sym::new(TAG_BUILDER)),
        "content_tag" | "link_to" | "button_to" | "image_tag" | "safe_join" | "asset_path"
        | "asset_url" | "image_path" | "time_ago_in_words" | "pluralize" | "simple_format"
        | "dom_id" | "dom_class" => Type::String,
        // These return nil for nil input.
        "truncate"
        | "number_to_human"
        | "number_to_human_size"
        | "number_with_delimiter"
        | "number_to_currency"
        | "number_to_percentage"
        | "number_with_precision"
        | "number_to_phone" => Type::String.union_with(Type::Nil),
        "sanitize" => Type::Class(Sym::new("ActiveSupport::SafeBuffer")).union_with(Type::Nil),
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
                    origin: MethodOrigin::RubySource,
                    attr_ivar: None,
                    is_singleton: false,
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
        parse_result: &ParseResult<'_>,
    ) {
        let names = Self::extract_symbol_args(call_node);
        for name in &names {
            let _ = self.registry.has_method_named(class_name, name);
        }
        // Options such as `only:` or `if:` leave some actions unguarded.
        let unconditional = call_node.arguments().is_some_and(|arguments| {
            arguments
                .arguments()
                .iter()
                .all(|argument| argument.as_symbol_node().is_some())
        });
        if call_node.name().as_slice() != b"before_action" || !unconditional {
            return;
        }
        for callback in names {
            let guarded = halting_guards(parse_result, class_name, &callback);
            if guarded.is_empty() {
                continue;
            }
            let guards = self
                .action_guards
                .entry(class_name.to_string())
                .or_default();
            guards.callbacks.push(callback);
            guards.methods.extend(guarded);
        }
    }

    /// An action runs only after its `before_action` callbacks; one that halts unless a self
    /// method is truthy (`head :not_found unless entry`) leaves that method truthy there.
    pub(in crate::inference) fn seed_action_guard_facts(
        &self,
        class_name: &str,
        method_name: &str,
        scope: &mut Scope,
    ) {
        let Some(guards) = self.action_guards.get(class_name) else {
            return;
        };
        if guards
            .callbacks
            .iter()
            .any(|callback| callback == method_name)
            || guards.methods.iter().any(|guarded| guarded == method_name)
            || self
                .registry
                .method_visibility(class_name, method_name, false)
                .is_some()
        {
            return;
        }
        for guarded in &guards.methods {
            let subject = NarrowSubject::SelfMethod(guarded.clone());
            if let Some(narrowed) = self
                .resolve_subject_input_type(class_name, scope, &subject)
                .and_then(|input| Self::truthy_only(&input))
            {
                scope.narrow_subject(&subject, narrowed);
            }
        }
    }
}

/// Self methods that the callback `name` of `class_name` halts on when they are falsy.
fn halting_guards(parse_result: &ParseResult<'_>, class_name: &str, name: &str) -> Vec<String> {
    let mut finder = CallbackDefFinder {
        class_name,
        name: name.as_bytes(),
        namespace: Vec::new(),
        guards: None,
    };
    finder.visit(&parse_result.node());
    finder.guards.unwrap_or_default()
}

struct CallbackDefFinder<'a> {
    class_name: &'a str,
    name: &'a [u8],
    namespace: Vec<String>,
    guards: Option<Vec<String>>,
}

impl CallbackDefFinder<'_> {
    fn in_target_class(&self) -> bool {
        self.namespace.join("::") == self.class_name.trim_start_matches("::")
    }
}

impl<'pr> Visit<'pr> for CallbackDefFinder<'_> {
    fn visit_class_node(&mut self, node: &ruby_prism::ClassNode<'pr>) {
        self.namespace
            .push(String::from_utf8_lossy(node.constant_path().location().as_slice()).to_string());
        ruby_prism::visit_class_node(self, node);
        self.namespace.pop();
    }

    fn visit_module_node(&mut self, node: &ruby_prism::ModuleNode<'pr>) {
        self.namespace
            .push(String::from_utf8_lossy(node.constant_path().location().as_slice()).to_string());
        ruby_prism::visit_module_node(self, node);
        self.namespace.pop();
    }

    fn visit_def_node(&mut self, node: &ruby_prism::DefNode<'pr>) {
        if self.guards.is_none()
            && node.receiver().is_none()
            && node.parameters().is_none()
            && node.name().as_slice() == self.name
            && self.in_target_class()
        {
            let statements = node
                .body()
                .and_then(|body| body.as_statements_node())
                .map(|statements| statements.body().iter().collect::<Vec<_>>())
                .unwrap_or_default();
            self.guards = Some(statements.iter().filter_map(halting_guard).collect());
        }
    }
}

/// `halt unless m`, or `halt if m.nil?` / `m.blank?` / `!m`, where `halt` responds or raises.
fn halting_guard(statement: &Node<'_>) -> Option<String> {
    let (predicate, body, negated) = if let Some(unless_node) = statement.as_unless_node() {
        if unless_node.else_clause().is_some() {
            return None;
        }
        (unless_node.predicate(), unless_node.statements()?, false)
    } else {
        let if_node = statement.as_if_node()?;
        if if_node.subsequent().is_some() {
            return None;
        }
        (if_node.predicate(), if_node.statements()?, true)
    };
    let mut body = body.body().iter();
    let (Some(halt), None) = (body.next(), body.next()) else {
        return None;
    };
    if !halts(&halt) {
        return None;
    }
    let subject = if negated {
        let call = predicate.as_call_node()?;
        if !matches!(call.name().as_slice(), b"nil?" | b"blank?" | b"!")
            || call.arguments().is_some()
        {
            return None;
        }
        call.receiver()?
    } else {
        predicate
    };
    let call = subject.as_call_node()?;
    (call.receiver().is_none() && call.arguments().is_none() && call.block().is_none())
        .then(|| String::from_utf8_lossy(call.name().as_slice()).to_string())
}

fn halts(node: &Node<'_>) -> bool {
    if let Some(return_node) = node.as_return_node() {
        let mut values = return_node
            .arguments()
            .into_iter()
            .flat_map(|arguments| arguments.arguments().iter());
        return matches!((values.next(), values.next()), (Some(value), None) if halts(&value));
    }
    node.as_call_node().is_some_and(|call| {
        call.receiver().is_none()
            && matches!(
                call.name().as_slice(),
                b"head"
                    | b"render"
                    | b"redirect_to"
                    | b"redirect_back"
                    | b"redirect_back_or_to"
                    | b"raise"
            )
    })
}
