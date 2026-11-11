//! Ruby's binding of call arguments to source parameters, shared by method calls and proc
//! literal calls. Positional slots line up with the `ParamRef` indices a callable's body uses.

use std::collections::HashMap;

use crate::types::{ParamInfo, ParamKind, ProcKind, RecordField, RecordKey, Type};

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

/// Keywords passed to a callable without keyword parameters arrive as a trailing hash.
pub(crate) fn keyword_hash_argument<'a>(
    keywords: impl Iterator<Item = (&'a str, &'a Type)>,
) -> Type {
    let mut fields: Vec<RecordField> = keywords
        .map(|(name, ty)| RecordField {
            key: RecordKey::Symbol(name.to_string()),
            value: ty.clone(),
            optional: false,
        })
        .collect();
    fields.sort_by(|a, b| a.key.cmp(&b.key));
    Type::Record(fields)
}

/// A proc spreads a lone array argument unless it takes exactly one plain parameter or only a rest.
fn proc_auto_splats(params: &[ParamInfo]) -> bool {
    let single_plain = matches!(params, [param] if param.kind == ParamKind::Required);
    let takes_lead = params.iter().any(|param| {
        matches!(
            param.kind,
            ParamKind::Required
                | ParamKind::Optional
                | ParamKind::KeywordRequired
                | ParamKind::KeywordOptional
                | ParamKind::DoubleRest
        )
    });
    takes_lead && !single_plain
}

fn splat_lone_array(params: &[ParamInfo], arg: &Type) -> Option<Vec<Type>> {
    match arg {
        Type::Tuple(elems) => Some(elems.clone()),
        Type::Array(Some(elem)) => {
            let slots = params
                .iter()
                .filter(|param| matches!(param.kind, ParamKind::Required | ParamKind::Optional))
                .count();
            Some(vec![elem.as_ref().clone(); slots])
        }
        _ => None,
    }
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
    let splatted = match positional {
        [arg] if rules.is_proc() && keywords.is_empty() && proc_auto_splats(params) => {
            splat_lone_array(params, arg)
        }
        _ => None,
    };
    let mut args = splatted.unwrap_or_else(|| positional.to_vec());
    let positional_params: Vec<&ParamInfo> = params
        .iter()
        .filter(|param| is_positional(param.kind))
        .collect();
    if !keywords.is_empty() && !accepts_keywords(params) && args.len() < positional_params.len() {
        args.push(keyword_hash_argument(
            keywords.iter().map(|(name, ty)| (name.as_str(), ty)),
        ));
    }

    let positional = bind_positional_arguments(&positional_params, &args)
        .into_iter()
        .zip(&positional_params)
        .map(|(bound, param)| match param.kind {
            ParamKind::Rest if matches!(rules, ArgumentRules::Literal(_)) => Type::Tuple(bound),
            _ if !bound.is_empty() => finish(Type::from_type_vec_preserve_untyped(bound)),
            ParamKind::Optional => param.default_type.clone().map_or(Type::Untyped, finish),
            ParamKind::Required if rules.is_proc() => Type::Nil,
            _ => Type::Untyped,
        })
        .collect();

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
        // `*` stands for keywords whose names are unknown.
        if matches!(rules, ArgumentRules::Literal(_)) && !keywords.contains_key("*") {
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
}
