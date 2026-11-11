//! Ruby's binding of call arguments to source parameters, shared by method calls and proc
//! literal calls. Positional slots line up with the `ParamRef` indices a callable's body uses.

use std::collections::HashMap;

use crate::registry::ParamInfo;
use crate::types::{ParamKind, RecordField, RecordKey, Type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcKind {
    Proc,
    Lambda,
}

/// How a callable treats its arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgumentRules {
    /// A method summarizes every call site, so bound values are widened and a rest slot holds the
    /// element type its body wraps in an `Array`.
    Method,
    /// A literal is resolved per call: values stay exact and a rest slot holds the whole tuple.
    Literal(ProcKind),
}

impl ArgumentRules {
    fn widens(self) -> bool {
        self == Self::Method
    }

    fn is_proc(self) -> bool {
        self == Self::Literal(ProcKind::Proc)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundArguments {
    pub positional: Vec<Type>,
    pub keywords: HashMap<String, Type>,
}

pub(crate) fn is_positional(kind: ParamKind) -> bool {
    matches!(
        kind,
        ParamKind::Required | ParamKind::Optional | ParamKind::Rest
    )
}

pub(crate) fn accepts_keywords(params: &[ParamInfo]) -> bool {
    params.iter().any(|param| {
        matches!(
            param.kind,
            ParamKind::KeywordRequired | ParamKind::KeywordOptional | ParamKind::DoubleRest
        )
    })
}

/// Ruby's positional binding: required parameters first, then optionals, then the rest.
pub(crate) fn bind_positional_arguments(params: &[&ParamInfo], args: &[Type]) -> Vec<Vec<Type>> {
    let required_count = params
        .iter()
        .filter(|param| matches!(param.kind, ParamKind::Required))
        .count();
    let optional_count = params
        .iter()
        .filter(|param| matches!(param.kind, ParamKind::Optional))
        .count();
    let mut optional_args = args.len().saturating_sub(required_count);
    let mut rest_args = optional_args.saturating_sub(optional_count);
    let mut args = args.iter().cloned();
    params
        .iter()
        .map(|param| match param.kind {
            ParamKind::Optional if optional_args == 0 => Vec::new(),
            ParamKind::Optional => {
                optional_args -= 1;
                args.next().into_iter().collect()
            }
            ParamKind::Rest => {
                let collected = args.by_ref().take(rest_args).collect();
                rest_args = 0;
                collected
            }
            _ => args.next().into_iter().collect(),
        })
        .collect()
}

/// The splat at `open_index` may give any number of its element, so each arity-valid count
/// binds once and the parameters take the union.
pub(crate) fn bind_positional_arguments_with_open_splat(
    params: &[&ParamInfo],
    args: &[Type],
    open_index: usize,
) -> Vec<Vec<Type>> {
    let Some(open_elem) = args.get(open_index) else {
        return bind_positional_arguments(params, args);
    };
    let required = params
        .iter()
        .filter(|param| matches!(param.kind, ParamKind::Required))
        .count();
    let optional = params
        .iter()
        .filter(|param| matches!(param.kind, ParamKind::Optional))
        .count();
    let has_rest = params
        .iter()
        .any(|param| matches!(param.kind, ParamKind::Rest));
    let fixed = args.len() - 1;
    // Past `required + optional` only the rest grows, which a single extra element already covers.
    let max_count = (required + optional + usize::from(has_rest)).saturating_sub(fixed);
    let mut merged: Vec<Vec<Type>> = vec![Vec::new(); params.len()];
    let mut bound_any = false;
    for count in 0..=max_count {
        let total = fixed + count;
        if total < required || (!has_rest && total > required + optional) {
            continue;
        }
        let expanded: Vec<Type> = args[..open_index]
            .iter()
            .cloned()
            .chain(std::iter::repeat_n(open_elem.clone(), count))
            .chain(args[open_index + 1..].iter().cloned())
            .collect();
        for (slot, bound) in merged
            .iter_mut()
            .zip(bind_positional_arguments(params, &expanded))
        {
            for ty in bound {
                if !slot.contains(&ty) {
                    slot.push(ty);
                }
            }
        }
        bound_any = true;
    }
    if bound_any {
        merged
    } else {
        bind_positional_arguments(params, args)
    }
}

/// Keywords passed to a callable without keyword parameters arrive as one trailing hash.
pub(crate) fn keyword_hash_argument<'a>(
    keywords: impl Iterator<Item = (&'a str, &'a Type)>,
) -> Type {
    let mut fields: Vec<RecordField> = Vec::new();
    let mut unknown_names = false;
    for (name, ty) in keywords {
        // `*` stands for keywords whose names are unknown.
        unknown_names |= name == "*";
        fields.push(RecordField {
            key: RecordKey::Symbol(name.to_string()),
            value: ty.clone(),
            optional: false,
        });
    }
    if unknown_names {
        let values = fields.into_iter().map(|field| field.value).collect();
        return Type::Hash(
            Some(Box::new(Type::Symbol)),
            Some(Box::new(Type::from_type_vec_preserve_untyped(values))),
        );
    }
    fields.sort_by(|a, b| a.key.cmp(&b.key));
    Type::Record(fields)
}

/// Ruby spreads a lone array over a proc's parameters only when it has more than one positional
/// slot besides the rest, or a rest next to another positional (`|a, *b|`, `|a,|`).
fn proc_auto_splats(params: &[ParamInfo]) -> bool {
    let mut slots = 0;
    let mut has_rest = false;
    for param in params {
        match param.kind {
            ParamKind::Required | ParamKind::Optional => slots += 1,
            ParamKind::Rest => has_rest = true,
            _ => {}
        }
    }
    slots > 1 || (has_rest && slots > 0)
}

/// An array of unknown length gives each slot its element or `nil`, and the rest an array.
fn bind_spread_array(params: &[&ParamInfo], elem: &Type) -> Vec<Type> {
    params
        .iter()
        .map(|param| match param.kind {
            ParamKind::Rest => Type::Array(Some(Box::new(elem.clone()))),
            ParamKind::Optional => {
                let fallback = param.default_type.clone().unwrap_or(Type::Nil);
                elem.clone().union_with(fallback)
            }
            _ => elem.clone().union_with(Type::Nil),
        })
        .collect()
}

pub(crate) fn bind_arguments(
    params: &[ParamInfo],
    positional: &[Type],
    keywords: &HashMap<String, Type>,
    rules: ArgumentRules,
) -> BoundArguments {
    let finish = |ty: Type| {
        if rules.widens() {
            ty.widen_arg_for_param()
        } else {
            ty
        }
    };
    let positional_params: Vec<&ParamInfo> = params
        .iter()
        .filter(|param| is_positional(param.kind))
        .collect();
    let spread = match positional {
        [arg] if rules.is_proc() && keywords.is_empty() && proc_auto_splats(params) => Some(arg),
        _ => None,
    };
    let positional = match spread {
        Some(Type::Array(Some(elem))) => bind_spread_array(&positional_params, elem),
        _ => {
            let mut args = match spread {
                Some(Type::Tuple(elems)) => elems.clone(),
                _ => positional.to_vec(),
            };
            if !keywords.is_empty() && !accepts_keywords(params) {
                args.push(keyword_hash_argument(
                    keywords.iter().map(|(name, ty)| (name.as_str(), ty)),
                ));
            }
            bind_positional_arguments(&positional_params, &args)
                .into_iter()
                .zip(&positional_params)
                .map(|(bound, param)| match param.kind {
                    ParamKind::Rest if matches!(rules, ArgumentRules::Literal(_)) => {
                        Type::Tuple(bound)
                    }
                    _ if !bound.is_empty() => finish(Type::from_type_vec_preserve_untyped(bound)),
                    ParamKind::Optional => param.default_type.clone().map_or(Type::Untyped, finish),
                    ParamKind::Required if rules.is_proc() => Type::Nil,
                    _ => Type::Untyped,
                })
                .collect()
        }
    };

    let mut bound_keywords: HashMap<String, Type> = params
        .iter()
        .filter(|param| {
            matches!(
                param.kind,
                ParamKind::KeywordRequired | ParamKind::KeywordOptional
            )
        })
        .map(|param| {
            let ty = keywords
                .get(&param.name)
                .or(param.default_type.as_ref())
                .cloned()
                .map_or(Type::Untyped, finish);
            (param.name.clone(), ty)
        })
        .collect();
    if let Some(rest) = params
        .iter()
        .find(|param| param.kind == ParamKind::DoubleRest)
    {
        let remaining = keywords
            .iter()
            .filter(|(name, _)| !bound_keywords.contains_key(*name));
        if matches!(rules, ArgumentRules::Literal(_)) {
            let shape = keyword_hash_argument(remaining.map(|(name, ty)| (name.as_str(), ty)));
            bound_keywords.insert(rest.name.clone(), shape);
            return BoundArguments {
                positional,
                keywords: bound_keywords,
            };
        }
        let collected: Vec<Type> = remaining.map(|(_, ty)| finish(ty.clone())).collect();
        if !collected.is_empty() {
            bound_keywords.insert(
                rest.name.clone(),
                Type::Hash(
                    Some(Box::new(Type::Symbol)),
                    Some(Box::new(Type::from_type_vec_preserve_untyped(collected))),
                ),
            );
        }
    }
    BoundArguments {
        positional,
        keywords: bound_keywords,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(name: &str, kind: ParamKind, default_type: Option<Type>) -> ParamInfo {
        ParamInfo {
            name: name.to_string(),
            kind,
            default_type,
        }
    }

    fn int(value: i64) -> Type {
        Type::LiteralInteger(value)
    }

    fn bind(params: &[ParamInfo], args: &[Type], rules: ArgumentRules) -> BoundArguments {
        bind_arguments(params, args, &HashMap::new(), rules)
    }

    const LAMBDA: ArgumentRules = ArgumentRules::Literal(ProcKind::Lambda);
    const PROC: ArgumentRules = ArgumentRules::Literal(ProcKind::Proc);

    #[test]
    fn optional_falls_back_to_default_and_post_takes_last_argument() {
        let params = [
            param("a", ParamKind::Required, None),
            param("b", ParamKind::Optional, Some(int(2))),
            param("rest", ParamKind::Rest, None),
            param("last", ParamKind::Required, None),
        ];
        let bound = bind(&params, &[int(1), int(9)], LAMBDA);
        assert_eq!(
            bound.positional,
            vec![int(1), int(2), Type::Tuple(Vec::new()), int(9)]
        );
    }

    #[test]
    fn method_rest_slot_is_the_widened_element_type() {
        let params = [param("items", ParamKind::Rest, None)];
        let bound = bind(&params, &[int(1), int(2)], ArgumentRules::Method);
        assert_eq!(bound.positional, vec![Type::Integer]);
        let bound = bind(&params, &[int(1), int(2)], LAMBDA);
        assert_eq!(bound.positional, vec![Type::Tuple(vec![int(1), int(2)])]);
    }

    #[test]
    fn keywords_become_a_hash_without_keyword_parameters() {
        let params = [param("options", ParamKind::Required, None)];
        let keywords = HashMap::from([("key".to_string(), int(1))]);
        let bound = bind_arguments(&params, &[], &keywords, LAMBDA);
        assert_eq!(
            bound.positional,
            vec![keyword_hash_argument([("key", &int(1))].into_iter())]
        );
    }

    #[test]
    fn keyword_parameters_take_the_argument_or_default() {
        let params = [param("key", ParamKind::KeywordOptional, Some(int(1)))];
        let bound = bind(&params, &[], LAMBDA);
        assert_eq!(bound.keywords.get("key"), Some(&int(1)));
        let keywords = HashMap::from([("key".to_string(), Type::LiteralString("x".into()))]);
        let bound = bind_arguments(&params, &[], &keywords, LAMBDA);
        assert_eq!(
            bound.keywords.get("key"),
            Some(&Type::LiteralString("x".into()))
        );
    }

    #[test]
    fn keyword_rest_keeps_the_literal_shape_but_a_method_element_type() {
        let params = [
            param("key", ParamKind::KeywordOptional, Some(int(1))),
            param("rest", ParamKind::DoubleRest, None),
        ];
        let keywords = HashMap::from([("key".to_string(), int(2)), ("other".to_string(), int(3))]);
        let literal = bind_arguments(&params, &[], &keywords, LAMBDA);
        assert_eq!(
            literal.keywords.get("rest"),
            Some(&keyword_hash_argument([("other", &int(3))].into_iter()))
        );
        let method = bind_arguments(&params, &[], &keywords, ArgumentRules::Method);
        assert_eq!(
            method.keywords.get("rest"),
            Some(&Type::Hash(
                Some(Box::new(Type::Symbol)),
                Some(Box::new(Type::Integer))
            ))
        );
    }

    #[test]
    fn only_procs_spread_a_lone_array() {
        let params = [
            param("a", ParamKind::Required, None),
            param("b", ParamKind::Required, None),
        ];
        let pair = Type::Tuple(vec![int(1), int(2)]);
        assert_eq!(
            bind(&params, std::slice::from_ref(&pair), PROC).positional,
            vec![int(1), int(2)]
        );
        assert_eq!(
            bind(&params, std::slice::from_ref(&pair), LAMBDA).positional,
            vec![pair.clone(), Type::Untyped]
        );
        let single = [param("a", ParamKind::Required, None)];
        assert_eq!(
            bind(&single, std::slice::from_ref(&pair), PROC).positional,
            vec![pair]
        );
    }

    #[test]
    fn procs_fill_missing_arguments_with_nil() {
        let params = [
            param("a", ParamKind::Required, None),
            param("b", ParamKind::Required, None),
        ];
        assert_eq!(
            bind(&params, &[int(1)], PROC).positional,
            vec![int(1), Type::Nil]
        );
        assert_eq!(
            bind(&params, &[int(1)], LAMBDA).positional,
            vec![int(1), Type::Untyped]
        );
    }

    #[test]
    fn open_splat_binds_every_arity_valid_count() {
        let params = [
            param("a", ParamKind::Required, None),
            param("b", ParamKind::Required, None),
            param("c", ParamKind::Optional, None),
        ];
        let params: Vec<&ParamInfo> = params.iter().collect();
        let bound =
            bind_positional_arguments_with_open_splat(&params, &[Type::Symbol, Type::String], 1);
        assert_eq!(
            bound,
            vec![vec![Type::Symbol], vec![Type::String], vec![Type::String]]
        );
    }

    #[test]
    fn procs_spread_only_with_several_positional_slots() {
        let pair = Type::Tuple(vec![int(1), int(2)]);
        let spreads = |params: &[ParamInfo]| bind(params, std::slice::from_ref(&pair), PROC);
        let a = param("a", ParamKind::Required, None);
        let rest = param("", ParamKind::Rest, None);
        for params in [
            vec![a.clone(), param("b", ParamKind::Block, None)],
            vec![
                a.clone(),
                param("k", ParamKind::KeywordOptional, Some(int(1))),
            ],
            vec![param("a", ParamKind::Optional, Some(int(5)))],
        ] {
            assert_eq!(spreads(&params).positional[0], pair, "{params:?}");
        }
        assert_eq!(
            spreads(std::slice::from_ref(&rest)).positional,
            vec![Type::Tuple(vec![pair.clone()])]
        );
        assert_eq!(spreads(&[a, rest]).positional[0], int(1));
    }

    #[test]
    fn spreading_an_array_of_unknown_length_may_leave_slots_nil() {
        let params = [
            param("a", ParamKind::Required, None),
            param("b", ParamKind::Optional, Some(Type::String)),
            param("rest", ParamKind::Rest, None),
        ];
        let array = Type::Array(Some(Box::new(Type::Integer)));
        assert_eq!(
            bind(&params, std::slice::from_ref(&array), PROC).positional,
            vec![
                Type::Integer.union_with(Type::Nil),
                Type::Integer.union_with(Type::String),
                array,
            ]
        );
    }

    #[test]
    fn open_splat_without_a_valid_count_binds_one_element() {
        let params = [param("a", ParamKind::Required, None)];
        let params: Vec<&ParamInfo> = params.iter().collect();
        let bound = bind_positional_arguments_with_open_splat(
            &params,
            &[Type::Symbol, Type::Symbol, Type::String],
            2,
        );
        assert_eq!(bound, vec![vec![Type::Symbol]]);
    }

    #[test]
    fn rest_receives_keywords_as_a_hash_without_keyword_parameters() {
        let params = [param("rest", ParamKind::Rest, None)];
        let keywords = HashMap::from([("k".to_string(), int(2))]);
        let bound = bind_arguments(&params, &[int(1)], &keywords, LAMBDA);
        assert_eq!(
            bound.positional,
            vec![Type::Tuple(vec![
                int(1),
                keyword_hash_argument([("k", &int(2))].into_iter())
            ])]
        );
    }
}
