//! Compact RBS IR meant to stay resident in the registry (smaller slot/heap footprint than `rbs_sys`).

use crate::types::Sym;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RbsRecordKey {
    Symbol(Box<str>),
    String(Box<str>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbsRecordField {
    pub key: RbsRecordKey,
    pub type_: RbsType,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RbsType {
    Integer,
    Float,
    String,
    Symbol,
    Bool,
    Nil,
    Void,
    Untyped,
    Top,
    Bottom,
    SelfType,
    ClassType,
    InstanceType,
    Class(Sym, Box<[RbsType]>),
    Singleton(Sym),
    Union(Box<[RbsType]>),
    Intersection(Box<[RbsType]>),
    Optional(Box<RbsType>),
    Tuple(Box<[RbsType]>),
    Record(Box<[RbsRecordField]>),
    Proc(Box<MethodType>),
    Variable(Sym),
    Alias(Sym, Box<[RbsType]>),
    Literal(Box<str>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionParam {
    pub type_: RbsType,
    pub name: Option<Sym>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType {
    pub required_positionals: Box<[FunctionParam]>,
    pub optional_positionals: Box<[FunctionParam]>,
    pub rest_positionals: Option<Box<FunctionParam>>,
    pub trailing_positionals: Box<[FunctionParam]>,
    pub required_keywords: Box<[(Sym, FunctionParam)]>,
    pub optional_keywords: Box<[(Sym, FunctionParam)]>,
    pub rest_keywords: Option<Box<FunctionParam>>,
    pub return_type: RbsType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockType {
    pub function_type: FunctionType,
    pub required: bool,
    pub self_type: Option<Box<RbsType>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodType {
    pub function_type: FunctionType,
    pub block: Option<Box<BlockType>>,
    pub self_type: Option<Box<RbsType>>,
    pub type_params: Box<[Sym]>,
    pub type_param_bounds: Box<[(Sym, RbsType)]>,
    pub type_param_lower_bounds: Box<[(Sym, RbsType)]>,
    pub annotations: Box<[Sym]>,
}

// ── Conversion from rbs_sys (runs once, at the parse boundary) ──────────────────

impl From<&rbs_sys::RbsRecordKey> for RbsRecordKey {
    fn from(key: &rbs_sys::RbsRecordKey) -> Self {
        match key {
            rbs_sys::RbsRecordKey::Symbol(name) => RbsRecordKey::Symbol(name.as_str().into()),
            rbs_sys::RbsRecordKey::String(name) => RbsRecordKey::String(name.as_str().into()),
        }
    }
}

impl From<&rbs_sys::RbsRecordField> for RbsRecordField {
    fn from(field: &rbs_sys::RbsRecordField) -> Self {
        RbsRecordField {
            key: RbsRecordKey::from(&field.key),
            type_: RbsType::from(&field.type_),
            required: field.required,
        }
    }
}

impl From<&rbs_sys::RbsType> for RbsType {
    fn from(ty: &rbs_sys::RbsType) -> Self {
        match ty {
            rbs_sys::RbsType::Integer => RbsType::Integer,
            rbs_sys::RbsType::Float => RbsType::Float,
            rbs_sys::RbsType::String => RbsType::String,
            rbs_sys::RbsType::Symbol => RbsType::Symbol,
            rbs_sys::RbsType::Bool => RbsType::Bool,
            rbs_sys::RbsType::Nil => RbsType::Nil,
            rbs_sys::RbsType::Void => RbsType::Void,
            rbs_sys::RbsType::Untyped => RbsType::Untyped,
            rbs_sys::RbsType::Top => RbsType::Top,
            rbs_sys::RbsType::Bottom => RbsType::Bottom,
            rbs_sys::RbsType::SelfType => RbsType::SelfType,
            rbs_sys::RbsType::ClassType => RbsType::ClassType,
            rbs_sys::RbsType::InstanceType => RbsType::InstanceType,
            rbs_sys::RbsType::Class(name, args) => {
                RbsType::Class(Sym::new(name), args.iter().map(RbsType::from).collect())
            }
            rbs_sys::RbsType::Singleton(name) => RbsType::Singleton(Sym::new(name)),
            rbs_sys::RbsType::Union(types) => {
                RbsType::Union(types.iter().map(RbsType::from).collect())
            }
            rbs_sys::RbsType::Intersection(types) => {
                RbsType::Intersection(types.iter().map(RbsType::from).collect())
            }
            rbs_sys::RbsType::Optional(inner) => {
                RbsType::Optional(Box::new(RbsType::from(inner.as_ref())))
            }
            rbs_sys::RbsType::Tuple(types) => {
                RbsType::Tuple(types.iter().map(RbsType::from).collect())
            }
            rbs_sys::RbsType::Record(fields) => {
                RbsType::Record(fields.iter().map(RbsRecordField::from).collect())
            }
            rbs_sys::RbsType::Proc(method_type) => {
                RbsType::Proc(Box::new(MethodType::from(method_type.as_ref())))
            }
            rbs_sys::RbsType::Variable(name) => RbsType::Variable(Sym::new(name)),
            rbs_sys::RbsType::Alias(name, args) => {
                RbsType::Alias(Sym::new(name), args.iter().map(RbsType::from).collect())
            }
            rbs_sys::RbsType::Literal(value) => RbsType::Literal(value.as_str().into()),
        }
    }
}

impl From<&rbs_sys::FunctionParam> for FunctionParam {
    fn from(param: &rbs_sys::FunctionParam) -> Self {
        FunctionParam {
            type_: RbsType::from(&param.type_),
            name: param.name.as_deref().map(Sym::new),
        }
    }
}

impl From<&rbs_sys::FunctionType> for FunctionType {
    fn from(ft: &rbs_sys::FunctionType) -> Self {
        let keyword = |(name, param): &(std::string::String, rbs_sys::FunctionParam)| {
            (Sym::new(name), FunctionParam::from(param))
        };
        FunctionType {
            required_positionals: ft
                .required_positionals
                .iter()
                .map(FunctionParam::from)
                .collect(),
            optional_positionals: ft
                .optional_positionals
                .iter()
                .map(FunctionParam::from)
                .collect(),
            rest_positionals: ft
                .rest_positionals
                .as_ref()
                .map(|param| Box::new(FunctionParam::from(param))),
            trailing_positionals: ft
                .trailing_positionals
                .iter()
                .map(FunctionParam::from)
                .collect(),
            required_keywords: ft.required_keywords.iter().map(keyword).collect(),
            optional_keywords: ft.optional_keywords.iter().map(keyword).collect(),
            rest_keywords: ft
                .rest_keywords
                .as_ref()
                .map(|param| Box::new(FunctionParam::from(param))),
            return_type: RbsType::from(&ft.return_type),
        }
    }
}

impl From<&rbs_sys::BlockType> for BlockType {
    fn from(block: &rbs_sys::BlockType) -> Self {
        BlockType {
            function_type: FunctionType::from(&block.function_type),
            required: block.required,
            self_type: block
                .self_type
                .as_ref()
                .map(|ty| Box::new(RbsType::from(ty))),
        }
    }
}

impl From<&rbs_sys::MethodType> for MethodType {
    fn from(mt: &rbs_sys::MethodType) -> Self {
        let bound = |(name, ty): &(std::string::String, rbs_sys::RbsType)| {
            (Sym::new(name), RbsType::from(ty))
        };
        MethodType {
            function_type: FunctionType::from(&mt.function_type),
            block: mt
                .block
                .as_ref()
                .map(|block| Box::new(BlockType::from(block))),
            self_type: mt.self_type.as_ref().map(|ty| Box::new(RbsType::from(ty))),
            type_params: mt.type_params.iter().map(Sym::new).collect(),
            type_param_bounds: mt.type_param_bounds.iter().map(bound).collect(),
            type_param_lower_bounds: mt.type_param_lower_bounds.iter().map(bound).collect(),
            annotations: mt.annotations.iter().map(Sym::new).collect(),
        }
    }
}

/// Converts a parsed list of overloads to IR in bulk.
pub fn method_types_from_rbs(method_types: &[rbs_sys::MethodType]) -> Vec<MethodType> {
    method_types.iter().map(MethodType::from).collect()
}

// Deep byte estimate: `Sym` counts as 0; Box/slice count only the data size (allocator overhead excluded).

fn record_key_extra_bytes(key: &RbsRecordKey) -> usize {
    match key {
        RbsRecordKey::Symbol(name) | RbsRecordKey::String(name) => name.len(),
    }
}

/// Heap-side byte count for `RbsType` (excluding the enum body itself).
pub fn rbs_type_extra_bytes(ty: &RbsType) -> usize {
    match ty {
        RbsType::Class(_, args) | RbsType::Alias(_, args) => {
            args.len() * std::mem::size_of::<RbsType>()
                + args.iter().map(rbs_type_extra_bytes).sum::<usize>()
        }
        RbsType::Union(parts) | RbsType::Intersection(parts) | RbsType::Tuple(parts) => {
            parts.len() * std::mem::size_of::<RbsType>()
                + parts.iter().map(rbs_type_extra_bytes).sum::<usize>()
        }
        RbsType::Optional(inner) => std::mem::size_of::<RbsType>() + rbs_type_extra_bytes(inner),
        RbsType::Record(fields) => {
            fields.len() * std::mem::size_of::<RbsRecordField>()
                + fields
                    .iter()
                    .map(|field| {
                        record_key_extra_bytes(&field.key) + rbs_type_extra_bytes(&field.type_)
                    })
                    .sum::<usize>()
        }
        RbsType::Proc(method_type) => {
            std::mem::size_of::<MethodType>() + method_type_extra_bytes(method_type)
        }
        RbsType::Literal(value) => value.len(),
        _ => 0,
    }
}

fn function_param_extra_bytes(param: &FunctionParam) -> usize {
    rbs_type_extra_bytes(&param.type_)
}

fn function_type_extra_bytes(ft: &FunctionType) -> usize {
    let positionals = ft
        .required_positionals
        .iter()
        .chain(ft.optional_positionals.iter())
        .chain(ft.trailing_positionals.iter());
    let positional_len = ft.required_positionals.len()
        + ft.optional_positionals.len()
        + ft.trailing_positionals.len();
    let keywords = ft
        .required_keywords
        .iter()
        .chain(ft.optional_keywords.iter());
    let keyword_len = ft.required_keywords.len() + ft.optional_keywords.len();
    positional_len * std::mem::size_of::<FunctionParam>()
        + positionals.map(function_param_extra_bytes).sum::<usize>()
        + keyword_len * std::mem::size_of::<(Sym, FunctionParam)>()
        + keywords
            .map(|(_, param)| function_param_extra_bytes(param))
            .sum::<usize>()
        + ft.rest_positionals
            .as_deref()
            .map(|param| std::mem::size_of::<FunctionParam>() + function_param_extra_bytes(param))
            .unwrap_or(0)
        + ft.rest_keywords
            .as_deref()
            .map(|param| std::mem::size_of::<FunctionParam>() + function_param_extra_bytes(param))
            .unwrap_or(0)
        + rbs_type_extra_bytes(&ft.return_type)
}

/// Heap-side byte count for `MethodType` (excluding the enum/struct body itself).
pub fn method_type_extra_bytes(mt: &MethodType) -> usize {
    function_type_extra_bytes(&mt.function_type)
        + mt.block
            .as_deref()
            .map(|block| {
                std::mem::size_of::<BlockType>()
                    + function_type_extra_bytes(&block.function_type)
                    + block
                        .self_type
                        .as_deref()
                        .map(|ty| std::mem::size_of::<RbsType>() + rbs_type_extra_bytes(ty))
                        .unwrap_or(0)
            })
            .unwrap_or(0)
        + mt.self_type
            .as_deref()
            .map(|ty| std::mem::size_of::<RbsType>() + rbs_type_extra_bytes(ty))
            .unwrap_or(0)
        + mt.type_params.len() * std::mem::size_of::<Sym>()
        + (mt.type_param_bounds.len() + mt.type_param_lower_bounds.len())
            * std::mem::size_of::<(Sym, RbsType)>()
        + mt.type_param_bounds
            .iter()
            .chain(mt.type_param_lower_bounds.iter())
            .map(|(_, ty)| rbs_type_extra_bytes(ty))
            .sum::<usize>()
        + mt.annotations.len() * std::mem::size_of::<Sym>()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the IR's inline size to its shrink target. If this regresses, the stdlib RBS
    /// cache's resident memory grows back, so only update it for an intentional size increase.
    #[test]
    fn ir_struct_sizes_stay_compact() {
        assert!(std::mem::size_of::<RbsType>() <= 40);
        assert!(std::mem::size_of::<FunctionType>() <= 136);
        assert!(std::mem::size_of::<MethodType>() <= 216);
    }

    #[test]
    fn conversion_is_faithful_for_representative_signature() {
        let parsed = rbs_sys::parse_method_type(
            "[T < Comparable] (Integer size, ?::String name, *untyped rest, key: [Symbol, 1], ?opt: { a: bool, \"b\" => nil }) ?{ (T, self_ty: ::Array[T]) -> void } -> (T | ::Range[Integer] | ^(untyped) -> self)?",
        )
        .expect("parse");
        let ir = MethodType::from(&parsed);

        // Verifies round-trip fidelity: structure, names, and literals still match after conversion.
        assert_eq!(ir.type_params.len(), 1);
        assert_eq!(ir.type_params[0], "T");
        assert_eq!(ir.type_param_bounds.len(), 1);
        assert_eq!(ir.function_type.required_positionals.len(), 1);
        assert_eq!(
            ir.function_type.required_positionals[0]
                .name
                .map(|n| n.to_string()),
            Some("size".to_string())
        );
        assert_eq!(ir.function_type.optional_positionals.len(), 1);
        assert!(ir.function_type.rest_positionals.is_some());
        assert_eq!(ir.function_type.required_keywords.len(), 1);
        assert_eq!(ir.function_type.optional_keywords.len(), 1);
        assert!(ir.block.is_some());
    }
}
