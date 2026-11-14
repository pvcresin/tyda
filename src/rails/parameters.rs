//! Return types of `ActionController::Parameters` methods, shared by the inference engine and
//! the registry's deferred resolution (a concern's `params` resolves only through its includer).

use crate::types::{Sym, Type};

pub const PARAMETERS_CLASS: &str = "ActionController::Parameters";

pub fn method_return(method_name: &str) -> Option<Type> {
    let parameters = || Type::Class(Sym::new(PARAMETERS_CLASS));
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
        | "deep_transform_values!"
        | "permit!" => Some(parameters()),
        "to_h" | "to_unsafe_h" | "to_hash" => Some(Type::Hash(
            Some(Box::new(Type::String)),
            Some(Box::new(Type::Untyped)),
        )),
        "keys" => Some(Type::Array(Some(Box::new(Type::String)))),
        "values" | "values_at" => Some(Type::Array(Some(Box::new(Type::Untyped)))),
        "any?" | "all?" | "empty?" | "none?" | "has_key?" | "include?" | "key?" | "member?"
        | "present?" | "blank?" | "permitted?" => Some(Type::Bool),
        "[]" | "fetch" | "dig" | "delete" | "each_value" => Some(Type::Untyped),
        _ => None,
    }
}
