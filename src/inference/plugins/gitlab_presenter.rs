//! ! Gitlab::View::Presenter (GitLab's in-tree presenter framework).
//! ! ! There is no gem marker for this framework — inheriting from ! `Gitlab::View::Presenter::Base` / `Delegated` / `Simple` only happens in ! repositories that vendor it, so the inheritance check *is* the source ! detection and no `DslLibrary` gate is applied.

use super::super::InferenceEngine;
use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;
use ruby_prism::{Node, ParseResult};

pub(super) struct GitlabPresenter;

static MANIFEST: PluginManifest = PluginManifest {
    id: "gitlab_presenter",
    features: &[DslFeature {
        library: DslLibrary::GitlabPresenter,
        gem_markers: &[],
    }],
    base_classes: PRESENTER_BASES,
    rails_default: false,
};

impl Plugin for GitlabPresenter {
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

    fn synthetic_method_return_fallback(
        &self,
        cx: &mut PluginCx<'_, '_>,
        receiver_type: &Type,
        method_name: &str,
    ) -> Option<Type> {
        delegated_fallback_method_return(cx, receiver_type, method_name)
    }

    fn collect_class_body_call(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        method_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
        comments: &[super::super::RbsComment],
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

const PRESENTER_BASES: &[&str] = &[
    "Gitlab::View::Presenter::Base",
    "Gitlab::View::Presenter::Delegated",
    "Gitlab::View::Presenter::Simple",
];

fn is_presenter_class(engine: &PluginCx<'_, '_>, class_name: &str) -> bool {
    engine.class_matches_or_inherits(class_name, PRESENTER_BASES)
        || engine.class_or_ancestors_include_module(class_name, "Gitlab::View::Presenter::Base")
}

/// Class-body DSL words this plugin recognizes (diagnostics suppression).
pub(in crate::inference) fn consumes_class_body_call(
    engine: &mut PluginCx<'_, '_>,
    class_name: &str,
    method_name: &str,
) -> bool {
    matches!(
        method_name,
        "presents" | "delegator_override" | "delegator_override_with"
    ) && is_presenter_class(engine, class_name)
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let Type::Class(class_name) = receiver_type else {
        return None;
    };
    if !matches!(
        method_name,
        "subject" | "current_user" | "present" | "can?" | "declarative_policy_delegate"
    ) {
        return None;
    }
    if !is_presenter_class(engine, class_name) {
        return None;
    }
    match method_name {
        "can?" => Some(Type::Bool),
        _ => Some(Type::Untyped),
    }
}

/// `Gitlab::View::Presenter::Delegated` forwards unknown methods to the presented subject via `delegate_missing_to`, so any method is callable at runtime.
/// This is a catch-all: it must run **last** in the plugin chain so name-based plugins (gettext's `s_`, controller helpers, …) keep their more precise types on presenter classes.
pub(in crate::inference) fn delegated_fallback_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    _method_name: &str,
) -> Option<Type> {
    let Type::Class(class_name) = receiver_type else {
        return None;
    };
    engine
        .class_matches_or_inherits(class_name, &["Gitlab::View::Presenter::Delegated"])
        .then_some(Type::Untyped)
}

pub(in crate::inference) fn collect_class_body_call(
    engine: &mut PluginCx<'_, '_>,
    class_name: &str,
    method_name: &str,
    call_node: &ruby_prism::CallNode<'_>,
    parse_result: &ParseResult<'_>,
    _comments: &[super::super::RbsComment],
) -> bool {
    if !matches!(
        method_name,
        "presents" | "delegator_override" | "delegator_override_with"
    ) {
        return false;
    }
    if !is_presenter_class(engine, class_name) {
        return false;
    }
    if method_name != "presents" {
        return true;
    }

    let mut presented_class: Option<String> = None;
    if let Some(args) = call_node.arguments() {
        for arg in args.arguments().iter() {
            if matches!(
                arg,
                Node::ConstantReadNode { .. } | Node::ConstantPathNode { .. }
            ) {
                let name = engine.resolve_constant_path(&arg, parse_result);
                if !name.is_empty() {
                    presented_class = Some(name.trim_start_matches("::").to_string());
                    break;
                }
            }
        }
    }
    let return_type = match &presented_class {
        Some(name) => {
            engine.record_reference(name);
            Type::Class(name.clone().into())
        }
        None => Type::Untyped,
    };

    let mut reader_names: Vec<String> = Vec::new();
    if let Some(as_name) = InferenceEngine::extract_hash_option_str(call_node, "as", parse_result) {
        reader_names.push(as_name);
    } else {
        reader_names.extend(InferenceEngine::extract_symbol_args(call_node));
    }
    let loc = super::super::offset_to_location(
        parse_result.source(),
        call_node.location().start_offset(),
    );
    for reader in reader_names {
        engine.add_simple_method_if_missing(class_name, &reader, return_type.clone(), false, loc);
    }
    true
}
