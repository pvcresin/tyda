use crate::types::{
    HoverBlockSig, HoverOverloadSig, MethodSig, OverloadSig, Param, ParamKind, RecordField, Type,
};

pub fn user_facing_type(ty: &Type) -> Type {
    match ty {
        Type::Todo => Type::Untyped,
        Type::Union(parts) => {
            Type::from_type_vec_preserve_untyped(parts.iter().map(user_facing_type).collect())
        }
        Type::Intersection(parts) => {
            Type::Intersection(parts.iter().map(user_facing_type).collect())
        }
        Type::Array(Some(inner)) => Type::Array(Some(Box::new(user_facing_type(inner)))),
        Type::Hash(Some(key), Some(value)) => Type::Hash(
            Some(Box::new(user_facing_type(key))),
            Some(Box::new(user_facing_type(value))),
        ),
        Type::Hash(Some(key), None) => Type::Hash(Some(Box::new(user_facing_type(key))), None),
        Type::Hash(None, Some(value)) => Type::Hash(None, Some(Box::new(user_facing_type(value)))),
        Type::Tuple(parts) => Type::Tuple(parts.iter().map(user_facing_type).collect()),
        Type::Record(fields) => Type::Record(
            fields
                .iter()
                .map(|field| RecordField {
                    key: field.key.clone(),
                    value: user_facing_type(&field.value),
                    optional: field.optional,
                })
                .collect(),
        ),
        Type::Proc {
            return_type,
            param_count,
        } => Type::Proc {
            return_type: Box::new(user_facing_type(return_type)),
            param_count: *param_count,
        },
        _ => ty.clone(),
    }
}

/// Format a method signature for CodeLens display (without class-body indentation).
pub fn format_method_sig_for_lens(method: &MethodSig) -> String {
    format_method_sig_for_lens_with_names(method, false)
}

/// Format a method signature for CodeLens display, optionally omitting
/// positional parameter names for TypeProf-compatible UX.
pub fn format_method_sig_for_lens_with_names(
    method: &MethodSig,
    output_parameter_names: bool,
) -> String {
    format_signature_with_names(
        &method.params,
        method.block.as_ref(),
        &method.return_type,
        output_parameter_names,
        !method.has_explicit_signature(),
    )
}

pub fn format_hover_method_sig(name: &str, overloads: &[HoverOverloadSig]) -> String {
    let lines: Vec<String> = overloads
        .iter()
        .enumerate()
        .map(|(idx, overload)| {
            let prefix = if idx == 0 {
                format!("{name}: ")
            } else {
                "    | ".to_string()
            };
            format!("{prefix}{}", format_hover_overload(overload))
        })
        .collect();
    lines.join("\n")
}

pub fn format_hover_inferred_method_sig(name: &str, method: &MethodSig) -> String {
    format!(
        "{name}: {}",
        format_signature_with_names(
            &method.params,
            method.block.as_ref(),
            &method.return_type,
            true,
            !method.has_explicit_signature(),
        )
    )
}

pub fn format_hover_callable_type(method: &MethodSig) -> String {
    format_signature_with_names(
        &method.params,
        method.block.as_ref(),
        &method.return_type,
        true,
        !method.has_explicit_signature(),
    )
}

fn format_signature_with_names(
    params: &[Param],
    block: Option<&HoverBlockSig>,
    return_type: &Type,
    output_parameter_names: bool,
    widen_params: bool,
) -> String {
    let filtered_params: Vec<&Param> = params
        .iter()
        .filter(|param| !matches!(param.kind, ParamKind::Block) || block.is_none())
        .collect();
    let return_str = format_return_type(return_type, !filtered_params.is_empty());
    if filtered_params.is_empty()
        && let Some(block) = block
    {
        return format!(
            "{} -> {return_str}",
            format_block_signature(block, widen_params).trim_start()
        );
    }
    let rendered = if filtered_params.is_empty() {
        format!("-> {return_str}")
    } else {
        let param_strs: Vec<String> = filtered_params
            .into_iter()
            .map(|param| format_param(param, output_parameter_names, widen_params))
            .collect();
        format!("({}) -> {return_str}", param_strs.join(", "))
    };
    if let Some(block) = block {
        if let Some((params, ret)) = rendered.split_once(" -> ") {
            format!(
                "{params}{} -> {ret}",
                format_block_signature(block, widen_params)
            )
        } else {
            rendered
        }
    } else {
        rendered
    }
}

fn format_hover_overload(overload: &HoverOverloadSig) -> String {
    let mut rendered =
        format_callable_signature(&overload.params, &overload.return_type, true, false);
    if let Some(block) = &overload.block {
        let block_rendered = format_block_signature(block, false);
        if let Some((params, ret)) = rendered.split_once(" -> ") {
            rendered = format!("{params}{block_rendered} -> {ret}");
        }
    }
    rendered
}

fn format_callable_signature(
    params: &[Param],
    return_type: &Type,
    output_parameter_names: bool,
    widen_params: bool,
) -> String {
    let params_str = if params.is_empty() {
        "()".to_string()
    } else {
        let param_strs: Vec<String> = params
            .iter()
            .map(|param| format_param(param, output_parameter_names, widen_params))
            .collect();
        format!("({})", param_strs.join(", "))
    };
    let return_str = format_return_type(return_type, !params.is_empty());
    format!("{params_str} -> {return_str}")
}

fn format_block_signature(block: &HoverBlockSig, widen_params: bool) -> String {
    let params_str = if block.params.is_empty() {
        "()".to_string()
    } else {
        let param_strs: Vec<String> = block
            .params
            .iter()
            .map(|param| format_param(param, true, widen_params))
            .collect();
        format!("({})", param_strs.join(", "))
    };
    let optional_prefix = if block.required { " " } else { " ?" };
    let return_str = format_return_type(&block.return_type, !block.params.is_empty());
    format!("{optional_prefix}{{ {params_str} -> {return_str} }}")
}

#[allow(dead_code)]
fn format_overload_signature(overload: &OverloadSig) -> String {
    format_signature_with_names(
        &overload.params,
        overload.block.as_ref(),
        &overload.return_type,
        true,
        false,
    )
}

fn format_param(param: &Param, output_parameter_names: bool, widen_param: bool) -> String {
    let param_type = if widen_param {
        param.param_type.widen()
    } else {
        param.param_type.clone()
    };
    let ty_str = format_param_type(&param_type);
    match param.kind {
        ParamKind::Required => {
            if output_parameter_names {
                format!("{ty_str} {}", param.name)
            } else {
                ty_str
            }
        }
        ParamKind::Optional => {
            if output_parameter_names {
                format!("?{ty_str} {}", param.name)
            } else {
                format!("?{ty_str}")
            }
        }
        ParamKind::Rest => {
            if output_parameter_names && !param.name.is_empty() {
                format!("*{ty_str} {}", param.name)
            } else {
                format!("*{ty_str}")
            }
        }
        ParamKind::KeywordRequired => format!("{}: {ty_str}", param.name),
        ParamKind::KeywordOptional => format!("?{}: {ty_str}", param.name),
        ParamKind::DoubleRest => {
            if output_parameter_names && !param.name.is_empty() {
                format!("**{ty_str} {}", param.name)
            } else {
                format!("**{ty_str}")
            }
        }
        ParamKind::Block => {
            if output_parameter_names {
                format!("?{ty_str} &{}", param.name)
            } else {
                format!("?{ty_str} &block")
            }
        }
    }
}

fn format_param_type(ty: &Type) -> String {
    let ty = user_facing_type(ty);
    ty.to_string()
}

fn format_return_type(ty: &Type, has_params: bool) -> String {
    let ty = user_facing_type(ty);
    if has_params {
        format!("{:#}", ty)
    } else {
        ty.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_preserves_explicit_literal_param_types() {
        let method = MethodSig {
            name: "initialize".to_string(),
            params: vec![Param {
                name: "name".to_string(),
                param_type: Type::LiteralString("test".to_string()),
                kind: ParamKind::Required,
            }],
            return_type: Type::Nil,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: true,
            rbs_inline_annotated: true,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        };

        assert_eq!(
            format_method_sig_for_lens_with_names(&method, true),
            "(\"test\" name) -> nil"
        );
        assert_eq!(
            format_hover_callable_type(&method),
            "(\"test\" name) -> nil"
        );
    }

    #[test]
    fn signature_widens_inferred_literal_param_types() {
        let method = MethodSig {
            name: "initialize".to_string(),
            params: vec![Param {
                name: "name".to_string(),
                param_type: Type::LiteralString("test".to_string()),
                kind: ParamKind::Required,
            }],
            return_type: Type::Nil,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        };

        assert_eq!(
            format_method_sig_for_lens_with_names(&method, true),
            "(String name) -> nil"
        );
    }

    #[test]
    fn lens_signature_renders_nested_todo_as_untyped() {
        let method = MethodSig {
            name: "items".to_string(),
            params: vec![Param {
                name: "values".to_string(),
                param_type: Type::Array(Some(Box::new(Type::Todo))),
                kind: ParamKind::Required,
            }],
            return_type: Type::Array(Some(Box::new(Type::Todo))),
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        };

        assert_eq!(
            format_method_sig_for_lens_with_names(&method, true),
            "(Array[untyped] values) -> Array[untyped]"
        );
    }

    #[test]
    fn lens_signature_hides_positional_names_by_default() {
        let method = MethodSig {
            name: "greet".to_string(),
            params: vec![
                Param {
                    name: "name".to_string(),
                    param_type: Type::String,
                    kind: ParamKind::Required,
                },
                Param {
                    name: "rest".to_string(),
                    param_type: Type::Integer,
                    kind: ParamKind::Rest,
                },
                Param {
                    name: "debug".to_string(),
                    param_type: Type::Bool,
                    kind: ParamKind::KeywordOptional,
                },
            ],
            return_type: Type::String,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        };

        assert_eq!(
            format_method_sig_for_lens(&method),
            "(String, *Integer, ?debug: bool) -> String"
        );
    }

    #[test]
    fn lens_signature_can_hide_positional_names() {
        let method = MethodSig {
            name: "greet".to_string(),
            params: vec![
                Param {
                    name: "name".to_string(),
                    param_type: Type::String,
                    kind: ParamKind::Required,
                },
                Param {
                    name: "rest".to_string(),
                    param_type: Type::Integer,
                    kind: ParamKind::Rest,
                },
                Param {
                    name: "debug".to_string(),
                    param_type: Type::Bool,
                    kind: ParamKind::KeywordOptional,
                },
            ],
            return_type: Type::String,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        };

        assert_eq!(
            format_method_sig_for_lens_with_names(&method, false),
            "(String, *Integer, ?debug: bool) -> String"
        );
    }

    #[test]
    fn hover_signature_formats_overloads_and_blocks() {
        let rendered = format_hover_method_sig(
            "each",
            &[
                HoverOverloadSig {
                    params: Vec::new(),
                    return_type: Type::Class(crate::types::Sym::new(
                        "Enumerator[Integer, Array[Integer]]",
                    )),
                    block: None,
                },
                HoverOverloadSig {
                    params: Vec::new(),
                    return_type: Type::Array(Some(Box::new(Type::Integer))),
                    block: Some(HoverBlockSig {
                        params: vec![Param {
                            name: "item".to_string(),
                            param_type: Type::Integer,
                            kind: ParamKind::Required,
                        }],
                        return_type: Type::Void,
                        required: true,
                    }),
                },
            ],
        );

        assert_eq!(
            rendered,
            concat!(
                "each: () -> Enumerator[Integer, Array[Integer]]\n",
                "    | () { (Integer item) -> void } -> Array[Integer]"
            )
        );
    }

    #[test]
    fn hover_inferred_signature_formats_method_sig() {
        let rendered = format_hover_inferred_method_sig(
            "foo",
            &MethodSig {
                name: "foo".to_string(),
                params: vec![Param {
                    name: "x".to_string(),
                    param_type: Type::Untyped,
                    kind: ParamKind::Required,
                }],
                return_type: Type::Untyped,
                block: None,
                sorbet_modifier_comments: Vec::new(),
                is_singleton: true,
                rbs_annotated: false,
                rbs_inline_annotated: false,
                sig_annotated: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                overloads: Vec::new(),
                loc: None,
                is_private: false,
            },
        );

        assert_eq!(rendered, "foo: (untyped x) -> untyped");
    }

    #[test]
    fn hover_callable_type_formats_method_without_name() {
        let rendered = format_hover_callable_type(&MethodSig {
            name: "tag_is_usable".to_string(),
            params: Vec::new(),
            return_type: Type::Nil,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        });

        assert_eq!(rendered, "-> nil");
    }

    #[test]
    fn lens_signature_formats_empty_tuple_as_rbs_empty_tuple() {
        let rendered = format_method_sig_for_lens(&MethodSig {
            name: "empty".to_string(),
            params: Vec::new(),
            return_type: Type::Tuple(Vec::new()),
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        });

        assert_eq!(rendered, "-> [ ]");
        assert!(rbs_sys::parse_inline_all_overloads(&format!(": {rendered}")).is_ok());
    }

    #[test]
    fn lens_signature_deduplicates_union_members() {
        let rendered = format_method_sig_for_lens(&MethodSig {
            name: "with_lock".to_string(),
            params: vec![Param {
                name: "autorelease".to_string(),
                param_type: Type::Union(vec![Type::Untyped, Type::Untyped]),
                kind: ParamKind::KeywordOptional,
            }],
            return_type: Type::Union(vec![Type::Untyped, Type::Untyped]),
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        });

        assert_eq!(rendered, "(?autorelease: untyped) -> untyped");
    }

    #[test]
    fn lens_signature_avoids_redundant_parentheses_for_single_param_type() {
        let rendered = format_method_sig_for_lens(&MethodSig {
            name: "wrap".to_string(),
            params: vec![Param {
                name: "value".to_string(),
                param_type: Type::Union(vec![Type::Untyped, Type::Untyped]),
                kind: ParamKind::Required,
            }],
            return_type: Type::Untyped,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        });

        assert_eq!(rendered, "(untyped) -> untyped");
    }

    #[test]
    fn lens_signature_keeps_parentheses_for_true_union_param_type() {
        let rendered = format_method_sig_for_lens(&MethodSig {
            name: "wrap".to_string(),
            params: vec![Param {
                name: "value".to_string(),
                param_type: Type::Union(vec![Type::Integer, Type::String]),
                kind: ParamKind::Required,
            }],
            return_type: Type::Untyped,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        });

        assert_eq!(rendered, "(Integer | String) -> untyped");
    }

    #[test]
    fn lens_signature_keeps_grouped_nilable_union_param_type() {
        let rendered = format_method_sig_for_lens(&MethodSig {
            name: "wrap".to_string(),
            params: vec![Param {
                name: "value".to_string(),
                param_type: Type::Union(vec![Type::String, Type::Bool, Type::Nil]),
                kind: ParamKind::Required,
            }],
            return_type: Type::Untyped,
            block: None,
            sorbet_modifier_comments: Vec::new(),
            is_singleton: false,
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            overloads: Vec::new(),
            loc: None,
            is_private: false,
        });

        assert_eq!(rendered, "((String | bool)?) -> untyped");
    }
}
