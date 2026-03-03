use crate::rbs::ir as rbs_ir;
use crate::types::Sym;
use crate::types::{RecordField, RecordKey, Type};

pub(crate) fn convert_rbs_builtin_alias<F>(
    name: &str,
    args: &[rbs_ir::RbsType],
    mut convert_arg: F,
) -> Option<Type>
where
    F: FnMut(&rbs_ir::RbsType) -> Type,
{
    match name.trim_start_matches("::") {
        "int" if args.is_empty() => Some(Type::Integer),
        "float" if args.is_empty() => Some(Type::Float),
        "bool" if args.is_empty() => Some(Type::Bool),
        "string" if args.is_empty() => Some(Type::String),
        "real" if args.is_empty() => Some(Type::Union(vec![
            Type::Integer,
            Type::Float,
            Type::Class(Sym::new("Rational")),
        ])),
        "path" if args.is_empty() => Some(Type::String),
        "encoding" if args.is_empty() => Some(Type::Union(vec![
            Type::Class(Sym::new("Encoding")),
            Type::String,
        ])),
        "interned" if args.is_empty() => Some(Type::Union(vec![Type::Symbol, Type::String])),
        "io" if args.is_empty() => Some(Type::Class(Sym::new("IO"))),
        "top" | "boolish" if args.is_empty() => Some(Type::Top),
        "array" if args.len() == 1 => Some(Type::Array(Some(Box::new(convert_arg(&args[0]))))),
        "array" if args.is_empty() => Some(Type::Array(None)),
        "hash" if args.len() == 2 => Some(Type::Hash(
            Some(Box::new(convert_arg(&args[0]))),
            Some(Box::new(convert_arg(&args[1]))),
        )),
        "hash" if args.is_empty() => Some(Type::Hash(None, None)),
        "range" if args.len() == 1 => {
            let elem = convert_arg(&args[0]);
            Some(Type::Generic {
                base: Sym::new("Range"),
                args: vec![elem].into(),
            })
        }
        "range" if args.is_empty() => Some(Type::Class(Sym::new("Range"))),
        _ => None,
    }
}

pub(crate) fn convert_rbs_type(rbs_ty: &rbs_ir::RbsType) -> Type {
    match rbs_ty {
        rbs_ir::RbsType::Integer => Type::Integer,
        rbs_ir::RbsType::Float => Type::Float,
        rbs_ir::RbsType::String => Type::String,
        rbs_ir::RbsType::Symbol => Type::Symbol,
        rbs_ir::RbsType::Bool => Type::Bool,
        rbs_ir::RbsType::Nil => Type::Nil,
        rbs_ir::RbsType::Void => Type::Void,
        rbs_ir::RbsType::Untyped => Type::Untyped,
        rbs_ir::RbsType::Top => Type::Top,
        rbs_ir::RbsType::Bottom => Type::Bot,
        rbs_ir::RbsType::SelfType => Type::SelfType,
        rbs_ir::RbsType::InstanceType => Type::SelfType,
        rbs_ir::RbsType::ClassType => Type::Class(Sym::new("Class")),
        rbs_ir::RbsType::Class(name, args) => {
            let bare = name.strip_prefix("::").unwrap_or(name);
            match bare {
                "Integer" => Type::Integer,
                "Float" => Type::Float,
                "String" => Type::String,
                "Symbol" => Type::Symbol,
                "NilClass" if args.is_empty() => Type::Nil,
                "TrueClass" if args.is_empty() => Type::True,
                "FalseClass" if args.is_empty() => Type::False,
                "Array" if args.is_empty() => Type::Array(None),
                "Array" if args.len() == 1 => {
                    Type::Array(Some(Box::new(convert_rbs_type(&args[0]))))
                }
                "Hash" if args.is_empty() => Type::Hash(None, None),
                "Hash" if args.len() == 2 => Type::Hash(
                    Some(Box::new(convert_rbs_type(&args[0]))),
                    Some(Box::new(convert_rbs_type(&args[1]))),
                ),
                _ => {
                    if args.is_empty() {
                        Type::Class(Sym::new(bare))
                    } else {
                        Type::Generic {
                            base: Sym::new(bare),
                            args: args.iter().map(convert_rbs_type).collect(),
                        }
                    }
                }
            }
        }
        rbs_ir::RbsType::Singleton(name) => Type::Singleton(Sym::new(name)),
        rbs_ir::RbsType::Union(types) => {
            let converted: Vec<Type> = types.iter().map(convert_rbs_type).collect();
            Type::from_type_vec(converted)
        }
        rbs_ir::RbsType::Optional(inner) => {
            let inner_ty = convert_rbs_type(inner);
            if inner_ty == Type::Untyped {
                Type::Untyped
            } else {
                inner_ty.union_with(Type::Nil)
            }
        }
        rbs_ir::RbsType::Tuple(types) => {
            let converted: Vec<Type> = types.iter().map(convert_rbs_type).collect();
            Type::Tuple(converted)
        }
        rbs_ir::RbsType::Record(fields) => {
            let converted: Vec<RecordField> = fields
                .iter()
                .map(|field| {
                    let record_key = match &field.key {
                        rbs_ir::RbsRecordKey::Symbol(name) => RecordKey::Symbol(name.to_string()),
                        rbs_ir::RbsRecordKey::String(name) => RecordKey::String(name.to_string()),
                    };
                    RecordField {
                        key: record_key,
                        value: convert_rbs_type(&field.type_),
                        optional: !field.required,
                    }
                })
                .collect();
            Type::Record(converted)
        }
        rbs_ir::RbsType::Proc(method_type) => {
            let ft = &method_type.function_type;
            let param_count = rbs_function_type_param_count(ft);
            let return_type = method_type
                .self_type
                .as_ref()
                .map(|self_type| {
                    convert_rbs_type(&ft.return_type)
                        .replace_self_type(&convert_rbs_type(self_type))
                })
                .unwrap_or_else(|| convert_rbs_type(&ft.return_type));
            Type::Proc {
                return_type: Box::new(return_type),
                param_count,
            }
        }
        rbs_ir::RbsType::Variable(_) => Type::Untyped,
        rbs_ir::RbsType::Alias(name, args) => {
            convert_rbs_builtin_alias(name.as_str(), args, convert_rbs_type)
                .unwrap_or_else(|| Type::Class(Sym::new(name)))
        }
        rbs_ir::RbsType::Literal(value) => match &**value {
            s if s.starts_with('"') => Type::LiteralString(s.trim_matches('"').to_string()),
            s if s.starts_with(':') => {
                let sym = s.strip_prefix(':').unwrap_or(s).to_string();
                Type::LiteralSymbol(Sym::new(sym))
            }
            "true" => Type::True,
            "false" => Type::False,
            "nil" => Type::Nil,
            s if s.parse::<i64>().is_ok() => Type::LiteralInteger(s.parse::<i64>().unwrap()),
            s if s
                .chars()
                .all(|c| c.is_ascii_digit() || c == '-' || c == '_')
                && !s.is_empty() =>
            {
                Type::Integer
            }
            s if s.parse::<f64>().is_ok() => Type::LiteralFloat(s.to_string()),
            _ => Type::Untyped,
        },
        rbs_ir::RbsType::Intersection(types) => {
            let converted: Vec<Type> = types.iter().map(convert_rbs_type).collect();
            if converted.len() == 1 {
                converted.into_iter().next().unwrap()
            } else {
                Type::Intersection(converted)
            }
        }
    }
}

fn rbs_function_type_param_count(ft: &rbs_ir::FunctionType) -> usize {
    ft.required_positionals.len()
        + ft.optional_positionals.len()
        + usize::from(ft.rest_positionals.is_some())
        + ft.trailing_positionals.len()
        + ft.required_keywords.len()
        + ft.optional_keywords.len()
        + usize::from(ft.rest_keywords.is_some())
}
