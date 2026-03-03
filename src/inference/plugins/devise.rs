//! ! Devise (the `devise` gem): per-scope controller / helper methods.
//! ! ! `devise_for :users` defines, for each scope, `current_user`, ! `user_signed_in?`, `authenticate_user!`, `user_session` on every ! controller and view helper.

use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::Type;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct Devise;

static MANIFEST: PluginManifest = PluginManifest {
    id: "devise",
    features: &[DslFeature {
        library: DslLibrary::Devise,
        gem_markers: &["devise"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Devise {
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
        if method_name == "devise" && cx.dsl_enabled(DslLibrary::Devise) {
            cx.collect_devise_dsl(class_name, call_node, parse_result);
            return true;
        }
        false
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        &["devise"]
    }
}

fn is_devise_context(engine: &PluginCx<'_, '_>, class_name: &str) -> bool {
    engine.is_action_controller_class(class_name)
        || class_name.ends_with("Helper")
        || class_name.ends_with("Mailer")
}

fn camelize(scope: &str) -> String {
    scope
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn scope_model_or_nil(engine: &mut PluginCx<'_, '_>, scope: &str) -> Type {
    let model_name = camelize(scope);
    if engine.registry().class_data_for(&model_name).is_some() {
        engine.record_reference(&model_name);
        Type::Class(model_name.into())
    } else {
        Type::Untyped
    }
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let Type::Class(class_name) = receiver_type else {
        return None;
    };
    if !engine.dsl_enabled(DslLibrary::Devise) {
        return None;
    }
    if !is_devise_context(engine, class_name) {
        return None;
    }
    match method_name {
        "sign_in"
        | "sign_out"
        | "bypass_sign_in"
        | "sign_in_and_redirect"
        | "sign_out_and_redirect"
        | "stored_location_for"
        | "after_sign_in_path_for"
        | "after_sign_out_path_for" => return Some(Type::Untyped),
        _ => {}
    }
    if let Some(scope) = method_name.strip_prefix("current_") {
        if !scope.is_empty() && scope.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            return Some(scope_model_or_nil(engine, scope));
        }
        return None;
    }
    if let Some(scope) = method_name.strip_suffix("_signed_in?") {
        if !scope.is_empty() {
            return Some(Type::Bool);
        }
        return None;
    }
    if let Some(rest) = method_name.strip_prefix("authenticate_") {
        if let Some(scope) = rest.strip_suffix('!')
            && !scope.is_empty()
        {
            return Some(Type::Untyped);
        }
        return None;
    }
    if let Some(scope) = method_name.strip_suffix("_session") {
        if !scope.is_empty() && engine.registry().class_data_for(&camelize(scope)).is_some() {
            return Some(Type::Untyped);
        }
        return None;
    }
    None
}

use super::super::*;
use crate::registry::IncluderBoundDsl;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_devise_dsl(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.devise_dsl_enabled() {
            return;
        }
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        if self.is_collecting_concern_included() {
            self.registry
                .push_includer_bound_dsl(class_name, IncluderBoundDsl::Devise { loc });
        } else if !self.is_active_record_model_class(class_name) {
            return;
        }

        self.registry.add_method_def_if_missing(
            "ActiveRecord::Base",
            MethodDef {
                name: Sym::new("devise"),
                param_infos: vec![ParamInfo {
                    name: "modules".to_string(),
                    kind: ParamKind::Rest,
                    default_type: Some(Type::Symbol),
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

        if self.is_collecting_concern_included() {
            return;
        }
        self.registry
            .register_devise_controller_helpers(class_name, loc);
    }

    pub(in crate::inference) fn apply_includer_bound_dsl_from_mixin(
        &mut self,
        includer: &str,
        mixin: &str,
    ) {
        let Some(pending) = self
            .registry
            .class_data_for(mixin)
            .map(|data| data.cold().includer_bound_dsl.clone())
        else {
            return;
        };
        for dsl in pending {
            self.registry.apply_includer_bound_dsl_to(includer, &dsl);
        }
    }
}
