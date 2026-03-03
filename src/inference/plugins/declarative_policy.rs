//! ! DeclarativePolicy (the `declarative_policy` gem, used by GitLab).
//! ! ! Policy classes inherit `DeclarativePolicy::Base` (commonly through a ! project `BasePolicy`).

use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::{Sym, Type};

pub(super) struct DeclarativePolicy;

static MANIFEST: PluginManifest = PluginManifest {
    id: "declarative_policy",
    features: &[DslFeature {
        library: DslLibrary::DeclarativePolicy,
        gem_markers: &["declarative_policy"],
    }],
    base_classes: POLICY_BASES,
    rails_default: false,
};

impl Plugin for DeclarativePolicy {
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

    fn consumes_class_body_call(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        method_name: &str,
    ) -> bool {
        consumes_class_body_call(cx, class_name, method_name)
    }
}

const POLICY_BASES: &[&str] = &["DeclarativePolicy::Base", "BasePolicy", "::BasePolicy"];

const RULE_DSL_CLASS: &str = "DeclarativePolicy::RuleDsl";

const POLICY_METHODS: &[&str] = &[
    "condition",
    "desc",
    "with_options",
    "with_scope",
    "with_score",
    "enable",
    "prevent",
    "prevent_all",
    "policy",
    "overrides",
    "include_subject",
    "scope",
];

const RULE_CHAIN_METHODS: &[&str] = &["enable", "prevent", "prevent_all", "policy"];

fn is_policy_class(engine: &PluginCx<'_, '_>, class_name: &str) -> bool {
    engine.dsl_enabled(DslLibrary::DeclarativePolicy)
        && engine.class_matches_or_inherits(class_name, POLICY_BASES)
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
    if class_name == RULE_DSL_CLASS {
        if !engine.dsl_enabled(DslLibrary::DeclarativePolicy) {
            return None;
        }
        return RULE_CHAIN_METHODS
            .contains(&method_name)
            .then(|| Type::Class(Sym::new(RULE_DSL_CLASS)));
    }
    if method_name == "rule" {
        return is_policy_class(engine, class_name).then(|| Type::Class(Sym::new(RULE_DSL_CLASS)));
    }
    if matches!(method_name, "can?" | "cannot?") {
        return is_policy_class(engine, class_name).then_some(Type::Bool);
    }
    if !POLICY_METHODS.contains(&method_name) {
        return None;
    }
    is_policy_class(engine, class_name).then_some(Type::Untyped)
}

/// Class-body DSL words this plugin recognizes (used to suppress
/// missing-method diagnostics for `rule { … }` style statements).
pub(in crate::inference) fn consumes_class_body_call(
    engine: &mut PluginCx<'_, '_>,
    class_name: &str,
    method_name: &str,
) -> bool {
    (method_name == "rule" || POLICY_METHODS.contains(&method_name))
        && is_policy_class(engine, class_name)
}
