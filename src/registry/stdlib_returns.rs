//! Table of return-invariant stdlib return types for the deferred path (fallback after a registry lookup miss).

use crate::types::{RecordKey, Type};

pub(crate) fn stdlib_receiver_method_return(receiver: &Type, method_name: &str) -> Option<Type> {
    // a `nil` receiver is a sign of under-approximation, so leave it unresolved and let it degrade to untyped.
    if matches!(receiver, Type::Nil) {
        return None;
    }
    if matches!(method_name, "dup" | "clone" | "itself" | "freeze") {
        return Some(receiver.clone());
    }
    match receiver {
        Type::String | Type::LiteralString(_) => string_method_return(method_name),
        Type::Symbol | Type::LiteralSymbol(_) => symbol_method_return(method_name),
        Type::Integer | Type::LiteralInteger(_) => integer_method_return(method_name),
        Type::Float | Type::LiteralFloat(_) => float_method_return(method_name),
        Type::Array(elem) => array_method_return(
            method_name,
            elem.as_deref().cloned().unwrap_or(Type::Untyped),
        ),
        Type::Tuple(elems) => array_method_return(method_name, Type::from_type_vec(elems.clone())),
        Type::Hash(key, value) => hash_method_return(
            method_name,
            key.as_deref().cloned().unwrap_or(Type::Untyped),
            value.as_deref().cloned().unwrap_or(Type::Untyped),
        ),
        Type::Record(fields) => {
            let key_types: Vec<Type> = fields
                .iter()
                .map(|field| match &field.key {
                    RecordKey::Symbol(_) => Type::Symbol,
                    RecordKey::String(_) => Type::String,
                })
                .collect();
            let value_types: Vec<Type> = fields.iter().map(|f| f.value.clone()).collect();
            hash_method_return(
                method_name,
                Type::from_type_vec(key_types),
                Type::from_type_vec(value_types),
            )
        }
        _ => None,
    }
}

fn string_method_return(method_name: &str) -> Option<Type> {
    match method_name {
        "upcase" | "downcase" | "capitalize" | "swapcase" | "reverse" | "strip" | "lstrip"
        | "rstrip" | "chomp" | "chop" | "squeeze" | "succ" | "next" | "b" | "scrub" | "to_s"
        | "to_str" => Some(Type::String),
        "length" | "size" | "bytesize" | "ord" | "hash" => Some(Type::Integer),
        _ => None,
    }
}

fn symbol_method_return(method_name: &str) -> Option<Type> {
    match method_name {
        "upcase" | "downcase" | "capitalize" | "swapcase" => Some(Type::Symbol),
        "to_s" | "name" => Some(Type::String),
        "length" | "size" => Some(Type::Integer),
        _ => None,
    }
}

fn integer_method_return(method_name: &str) -> Option<Type> {
    match method_name {
        // things like `round` have overloads and aren't return-invariant.
        "succ" | "next" | "pred" | "abs" | "magnitude" | "to_int" | "bit_length" | "hash"
        | "ord" => Some(Type::Integer),
        "chr" => Some(Type::String),
        "digits" => Some(Type::Array(Some(Box::new(Type::Integer)))),
        _ => None,
    }
}

fn float_method_return(method_name: &str) -> Option<Type> {
    match method_name {
        "abs" | "magnitude" => Some(Type::Float),
        _ => None,
    }
}

fn array_method_return(method_name: &str, elem: Type) -> Option<Type> {
    match method_name {
        "sort" | "reverse" | "shuffle" | "uniq" | "rotate" | "compact" | "to_a" => {
            Some(Type::Array(Some(Box::new(elem))))
        }
        "join" => Some(Type::String),
        "length" | "size" | "count" | "hash" => Some(Type::Integer),
        _ => None,
    }
}

fn hash_method_return(method_name: &str, key: Type, value: Type) -> Option<Type> {
    match method_name {
        "keys" => Some(Type::Array(Some(Box::new(key)))),
        "values" => Some(Type::Array(Some(Box::new(value)))),
        "invert" => Some(Type::Hash(Some(Box::new(value)), Some(Box::new(key)))),
        "length" | "size" => Some(Type::Integer),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_upcase_is_string() {
        assert_eq!(
            stdlib_receiver_method_return(&Type::String, "upcase"),
            Some(Type::String)
        );
    }

    #[test]
    fn array_sort_preserves_element_type() {
        let arr = Type::Array(Some(Box::new(Type::Integer)));
        assert_eq!(
            stdlib_receiver_method_return(&arr, "sort"),
            Some(Type::Array(Some(Box::new(Type::Integer))))
        );
    }

    #[test]
    fn arity_overloaded_methods_are_not_in_the_table() {
        let arr = Type::Array(Some(Box::new(Type::Integer)));
        assert_eq!(stdlib_receiver_method_return(&arr, "first"), None);
        assert_eq!(stdlib_receiver_method_return(&Type::Integer, "round"), None);
        assert_eq!(stdlib_receiver_method_return(&Type::String, "chars"), None);
    }

    #[test]
    fn record_keys_and_values_return_unions() {
        let record = Type::Record(vec![
            crate::types::RecordField {
                key: RecordKey::Symbol("a".into()),
                value: Type::Integer,
                optional: false,
            },
            crate::types::RecordField {
                key: RecordKey::Symbol("b".into()),
                value: Type::String,
                optional: false,
            },
        ]);
        assert_eq!(
            stdlib_receiver_method_return(&record, "keys"),
            Some(Type::Array(Some(Box::new(Type::Symbol))))
        );
        let values = stdlib_receiver_method_return(&record, "values");
        assert_eq!(
            values,
            Some(Type::Array(Some(Box::new(Type::from_type_vec(vec![
                Type::Integer,
                Type::String
            ])))))
        );
    }

    #[test]
    fn unknown_receiver_kinds_resolve_nothing() {
        assert_eq!(
            stdlib_receiver_method_return(&Type::Untyped, "upcase"),
            None
        );
        assert_eq!(
            stdlib_receiver_method_return(&Type::Class(crate::sym::Sym::new("Foo")), "upcase"),
            None
        );
    }
}
