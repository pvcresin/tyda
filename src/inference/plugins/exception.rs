//! ! Exception surface for classes whose definition is unavailable (gem ! exceptions like `JSON::ParserError`, `ActiveRecord::RecordInvalid`).
//! ! ! Rescued exceptions are often typed by their class name only — the gem's ! RBS is not loaded, so the `Exception` core surface is unreachable through ! the superclass chain.

use super::{Plugin, PluginCx, PluginManifest};
use crate::types::Type;

pub(super) struct Exception;

static MANIFEST: PluginManifest = PluginManifest {
    id: "exception",
    features: &[],
    base_classes: EXCEPTION_BASES,
    rails_default: false,
};

impl Plugin for Exception {
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
}

const EXCEPTION_BASES: &[&str] = &[
    "Exception",
    "StandardError",
    "RuntimeError",
    "ArgumentError",
    "ScriptError",
];

fn is_exception_class(engine: &PluginCx<'_, '_>, class_name: &str) -> bool {
    class_name.ends_with("Error")
        || class_name.ends_with("Exception")
        || class_name.ends_with("Invalid")
        || class_name.ends_with("NotFound")
        || class_name.ends_with("NotUnique")
        || class_name.ends_with("Timeout")
        || engine.class_matches_or_inherits(class_name, EXCEPTION_BASES)
}

pub(in crate::inference) fn synthetic_method_return(
    engine: &mut PluginCx<'_, '_>,
    receiver_type: &Type,
    method_name: &str,
) -> Option<Type> {
    let Type::Class(class_name) = receiver_type else {
        return None;
    };
    let result = match method_name {
        "message" | "to_s" | "full_message" | "detailed_message" | "inspect" => Type::String,
        "backtrace" => Type::Array(Some(Box::new(Type::String))).union_with(Type::Nil),
        "backtrace_locations" => Type::Untyped,
        "cause" | "exception" | "record" => Type::Untyped,
        _ => return None,
    };
    is_exception_class(engine, class_name).then_some(result)
}
