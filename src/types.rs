use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

pub use crate::sym::Sym;

pub type SharedPath = Arc<str>;
pub type SharedName = Arc<str>;

pub(crate) fn escape_rbs_string_literal(value: &str) -> std::string::String {
    let mut escaped = std::string::String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RecordKey {
    Symbol(std::string::String),
    String(std::string::String),
}

impl fmt::Display for RecordKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordKey::Symbol(name) => write!(f, "{name}:"),
            RecordKey::String(name) => write!(f, "\"{}\" =>", escape_rbs_string_literal(name)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecordField {
    pub key: RecordKey,
    pub value: Type,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeferredCallArgs {
    pub method: Sym,
    pub args: Box<[Type]>,
}

impl fmt::Display for RecordField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.key {
            RecordKey::Symbol(name) => {
                if self.optional {
                    write!(f, "?{name}: {}", self.value)
                } else {
                    write!(f, "{name}: {}", self.value)
                }
            }
            RecordKey::String(name) => {
                if self.optional {
                    write!(f, "?\"{}\" => {}", name.replace('"', "\\\""), self.value)
                } else {
                    write!(f, "\"{}\" => {}", name.replace('"', "\\\""), self.value)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Type {
    Integer,
    Float,
    String,
    Symbol,
    Bool,
    True,
    False,
    Nil,
    Untyped,
    Todo,
    // A caller parameter slot that has not been evaluated in the current fixpoint yet.
    ParamPending,
    Void,
    Top,
    Bot,
    Intersection(Vec<Type>),
    LiteralInteger(i64),
    LiteralFloat(std::string::String),
    LiteralString(std::string::String),
    LiteralSymbol(Sym),
    Array(Option<Box<Type>>),
    Hash(Option<Box<Type>>, Option<Box<Type>>),
    Record(Vec<RecordField>),
    Union(Vec<Type>),
    Class(Sym),
    Singleton(Sym),
    ParamRef(usize),
    KeywordParamRef(Sym),
    IvarRef(Sym),
    // A read with no method-local flow value resolves to the program-wide global type (so `def f = $g` doesn't depend on definition order).
    GlobalVariableRef(Sym),
    MethodReturnRef(Sym, Sym),
    BlockReturnRef,
    PatternIndexRef(Box<Type>, usize),
    PatternRestRef(Box<Type>),
    // Trailing destructure slot: index 0 is the last element.
    PatternTrailingRef(Box<Type>, usize),
    // `RecordKey` is 32B inline, so the key is boxed.
    PatternKeyRef(Box<Type>, Box<RecordKey>),
    PatternBindingKeyRef(Box<Type>, Box<RecordKey>),
    PatternKeyRestRef(Box<Type>, Box<[RecordKey]>),
    ReceiverMethodRef(Box<Type>, Sym),
    DeferredCallRef {
        receiver: Box<Type>,
        call: Box<DeferredCallArgs>,
    },
    SelfType,
    // At a call site this resolves to the receiver; when rendering a declaration, it resolves to an instance of the owner.
    InstanceType,
    Proc {
        return_type: Box<Type>,
        param_count: usize,
    },
    Tuple(Vec<Type>),
    TruthyProjection(Box<Type>),
    FalsyProjection(Box<Type>),
    // `args` is stored as `Box<[Type]>` rather than `Vec` for a narrower slot.
    Generic {
        base: Sym,
        args: Box<[Type]>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ParamArgShape {
    Hash(bool, bool),
    Record(Vec<(RecordKey, bool)>),
}

impl RecordKey {
    pub(crate) fn deep_extra_bytes(&self) -> usize {
        match self {
            RecordKey::Symbol(s) | RecordKey::String(s) => s.capacity(),
        }
    }
}

impl Type {
    pub(crate) fn deferred_call_ref(
        receiver: Type,
        method: Sym,
        args: impl Into<Box<[Type]>>,
    ) -> Self {
        Type::DeferredCallRef {
            receiver: Box::new(receiver),
            call: Box::new(DeferredCallArgs {
                method,
                args: args.into(),
            }),
        }
    }

    pub(crate) fn contains_deferred_call_ref(&self) -> bool {
        match self {
            Type::DeferredCallRef { .. } => true,
            Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => {
                parts.iter().any(Type::contains_deferred_call_ref)
            }
            Type::Array(Some(inner))
            | Type::PatternIndexRef(inner, _)
            | Type::PatternRestRef(inner)
            | Type::PatternTrailingRef(inner, _)
            | Type::PatternKeyRef(inner, _)
            | Type::PatternBindingKeyRef(inner, _)
            | Type::PatternKeyRestRef(inner, _)
            | Type::ReceiverMethodRef(inner, _)
            | Type::TruthyProjection(inner)
            | Type::FalsyProjection(inner) => inner.contains_deferred_call_ref(),
            Type::Hash(key, value) => {
                key.as_deref().is_some_and(Type::contains_deferred_call_ref)
                    || value
                        .as_deref()
                        .is_some_and(Type::contains_deferred_call_ref)
            }
            Type::Record(fields) => fields
                .iter()
                .any(|field| field.value.contains_deferred_call_ref()),
            Type::Generic { args, .. } => args.iter().any(Type::contains_deferred_call_ref),
            Type::Proc { return_type, .. } => return_type.contains_deferred_call_ref(),
            _ => false,
        }
    }

    pub(crate) fn contains_param_pending(&self) -> bool {
        match self {
            Type::ParamPending => true,
            Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => {
                parts.iter().any(Type::contains_param_pending)
            }
            Type::Array(Some(inner))
            | Type::PatternIndexRef(inner, _)
            | Type::PatternRestRef(inner)
            | Type::PatternTrailingRef(inner, _)
            | Type::PatternKeyRef(inner, _)
            | Type::PatternBindingKeyRef(inner, _)
            | Type::PatternKeyRestRef(inner, _)
            | Type::ReceiverMethodRef(inner, _)
            | Type::TruthyProjection(inner)
            | Type::FalsyProjection(inner) => inner.contains_param_pending(),
            Type::Hash(key, value) => {
                key.as_deref().is_some_and(Type::contains_param_pending)
                    || value.as_deref().is_some_and(Type::contains_param_pending)
            }
            Type::Record(fields) => fields
                .iter()
                .any(|field| field.value.contains_param_pending()),
            Type::Generic { args, .. } => args.iter().any(Type::contains_param_pending),
            Type::Proc { return_type, .. } => return_type.contains_param_pending(),
            Type::DeferredCallRef { receiver, call } => {
                receiver.contains_param_pending()
                    || call.args.iter().any(Type::contains_param_pending)
            }
            _ => false,
        }
    }

    pub fn deep_extra_bytes(&self) -> usize {
        const SLOT: usize = std::mem::size_of::<Type>();
        fn vec_bytes(types: &[Type]) -> usize {
            std::mem::size_of_val(types) + types.iter().map(Type::deep_extra_bytes).sum::<usize>()
        }
        fn boxed(ty: &Option<Box<Type>>) -> usize {
            ty.as_ref()
                .map(|t| std::mem::size_of::<Type>() + t.deep_extra_bytes())
                .unwrap_or(0)
        }
        match self {
            Type::Intersection(args) | Type::Union(args) | Type::Tuple(args) => vec_bytes(args),
            Type::Generic { args, .. } => vec_bytes(args),
            Type::LiteralFloat(s) | Type::LiteralString(s) => s.capacity(),
            Type::Array(elem) => boxed(elem),
            Type::Hash(k, v) => boxed(k) + boxed(v),
            Type::Record(fields) => {
                fields.len() * std::mem::size_of::<RecordField>()
                    + fields
                        .iter()
                        .map(|f| f.key.deep_extra_bytes() + f.value.deep_extra_bytes())
                        .sum::<usize>()
            }
            Type::PatternIndexRef(subject, _)
            | Type::PatternRestRef(subject)
            | Type::PatternTrailingRef(subject, _) => SLOT + subject.deep_extra_bytes(),
            Type::PatternKeyRef(subject, key) | Type::PatternBindingKeyRef(subject, key) => {
                SLOT + subject.deep_extra_bytes()
                    + std::mem::size_of::<RecordKey>()
                    + key.deep_extra_bytes()
            }
            Type::PatternKeyRestRef(subject, keys) => {
                SLOT + subject.deep_extra_bytes()
                    + keys.len() * std::mem::size_of::<RecordKey>()
                    + keys.iter().map(RecordKey::deep_extra_bytes).sum::<usize>()
            }
            Type::ReceiverMethodRef(receiver, _) => SLOT + receiver.deep_extra_bytes(),
            Type::DeferredCallRef { receiver, call } => {
                SLOT + receiver.deep_extra_bytes()
                    + SLOT
                    + std::mem::size_of_val(call.args.as_ref())
                    + call.args.iter().map(Type::deep_extra_bytes).sum::<usize>()
            }
            Type::TruthyProjection(inner) | Type::FalsyProjection(inner) => {
                SLOT + inner.deep_extra_bytes()
            }
            Type::Proc { return_type, .. } => SLOT + return_type.deep_extra_bytes(),
            _ => 0,
        }
    }

    pub(crate) fn project_truthy(ty: &Type) -> Option<Type> {
        match ty {
            Type::Nil | Type::False | Type::Void | Type::Bot => None,
            Type::ParamPending => Some(Type::ParamPending),
            Type::Bool => Some(Type::True),
            Type::Untyped | Type::Todo | Type::Top => Some(ty.clone()),
            Type::Union(parts) => {
                let projected: Vec<Type> = parts.iter().filter_map(Self::project_truthy).collect();
                match projected.len() {
                    0 => None,
                    1 => projected.into_iter().next(),
                    _ => Some(Type::from_type_vec_preserve_untyped(projected)),
                }
            }
            Type::MethodReturnRef(..)
            | Type::ReceiverMethodRef(..)
            | Type::DeferredCallRef { .. } => Some(Type::TruthyProjection(Box::new(ty.clone()))),
            Type::TruthyProjection(_) => Some(ty.clone()),
            Type::FalsyProjection(_) => None,
            _ => Some(ty.clone()),
        }
    }

    pub(crate) fn project_falsy(ty: &Type) -> Option<Type> {
        match ty {
            Type::Nil => Some(Type::Nil),
            Type::False => Some(Type::False),
            Type::Bool => Some(Type::False),
            Type::Untyped | Type::Todo | Type::Top | Type::ParamPending => Some(ty.clone()),
            Type::Union(parts) => {
                let projected: Vec<Type> = parts.iter().filter_map(Self::project_falsy).collect();
                match projected.len() {
                    0 => None,
                    1 => projected.into_iter().next(),
                    _ => Some(Type::from_type_vec_preserve_untyped(projected)),
                }
            }
            Type::MethodReturnRef(..)
            | Type::ReceiverMethodRef(..)
            | Type::DeferredCallRef { .. } => Some(Type::FalsyProjection(Box::new(ty.clone()))),
            Type::TruthyProjection(_) => None,
            Type::FalsyProjection(_) => Some(ty.clone()),
            _ => None,
        }
    }
}

impl PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        // Same address returns true without a structural walk (shared via clone); the direct match here is lighter than going through cmp_inner (`Type::eq` is a hot path).
        if std::ptr::eq(self as *const Type, other as *const Type) {
            return true;
        }
        match (self, other) {
            (Type::Integer, Type::Integer)
            | (Type::Float, Type::Float)
            | (Type::String, Type::String)
            | (Type::Symbol, Type::Symbol)
            | (Type::Bool, Type::Bool)
            | (Type::True, Type::True)
            | (Type::False, Type::False)
            | (Type::Nil, Type::Nil)
            | (Type::Untyped, Type::Untyped)
            | (Type::Todo, Type::Todo)
            | (Type::ParamPending, Type::ParamPending)
            | (Type::Void, Type::Void)
            | (Type::Top, Type::Top)
            | (Type::Bot, Type::Bot)
            | (Type::BlockReturnRef, Type::BlockReturnRef)
            | (Type::SelfType, Type::SelfType)
            | (Type::InstanceType, Type::InstanceType) => true,
            (Type::LiteralInteger(a), Type::LiteralInteger(b)) => a == b,
            (Type::LiteralFloat(a), Type::LiteralFloat(b))
            | (Type::LiteralString(a), Type::LiteralString(b)) => a == b,
            (Type::LiteralSymbol(a), Type::LiteralSymbol(b)) => a == b,
            (Type::Class(a), Type::Class(b))
            | (Type::Singleton(a), Type::Singleton(b))
            | (Type::KeywordParamRef(a), Type::KeywordParamRef(b))
            | (Type::IvarRef(a), Type::IvarRef(b))
            | (Type::GlobalVariableRef(a), Type::GlobalVariableRef(b)) => a == b,
            (Type::ParamRef(a), Type::ParamRef(b)) => a == b,
            (Type::MethodReturnRef(c1, m1), Type::MethodReturnRef(c2, m2)) => c1 == c2 && m1 == m2,
            (Type::Array(a), Type::Array(b)) => a == b,
            (Type::Hash(k1, v1), Type::Hash(k2, v2)) => k1 == k2 && v1 == v2,
            (Type::Record(a), Type::Record(b)) => a == b,
            (Type::Union(a), Type::Union(b))
            | (Type::Intersection(a), Type::Intersection(b))
            | (Type::Tuple(a), Type::Tuple(b)) => a == b,
            (Type::PatternIndexRef(t1, i1), Type::PatternIndexRef(t2, i2)) => t1 == t2 && i1 == i2,
            (Type::PatternRestRef(t1), Type::PatternRestRef(t2)) => t1 == t2,
            (Type::PatternTrailingRef(t1, i1), Type::PatternTrailingRef(t2, i2)) => {
                t1 == t2 && i1 == i2
            }
            (Type::PatternKeyRef(t1, k1), Type::PatternKeyRef(t2, k2)) => t1 == t2 && k1 == k2,
            (Type::PatternBindingKeyRef(t1, k1), Type::PatternBindingKeyRef(t2, k2)) => {
                t1 == t2 && k1 == k2
            }
            (Type::PatternKeyRestRef(t1, k1), Type::PatternKeyRestRef(t2, k2)) => {
                t1 == t2 && k1 == k2
            }
            (Type::ReceiverMethodRef(t1, m1), Type::ReceiverMethodRef(t2, m2)) => {
                t1 == t2 && m1 == m2
            }
            (
                Type::DeferredCallRef {
                    receiver: r1,
                    call: c1,
                },
                Type::DeferredCallRef {
                    receiver: r2,
                    call: c2,
                },
            ) => r1 == r2 && c1 == c2,
            (Type::TruthyProjection(t1), Type::TruthyProjection(t2))
            | (Type::FalsyProjection(t1), Type::FalsyProjection(t2)) => t1 == t2,
            (
                Type::Proc {
                    return_type: r1,
                    param_count: p1,
                },
                Type::Proc {
                    return_type: r2,
                    param_count: p2,
                },
            ) => p1 == p2 && r1 == r2,
            (Type::Generic { base: b1, args: a1 }, Type::Generic { base: b2, args: a2 }) => {
                b1 == b2 && a1 == a2
            }
            _ => false,
        }
    }
}

impl Eq for Type {}

impl std::hash::Hash for Type {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.discriminant().hash(state);
        match self {
            Type::LiteralInteger(v) => v.hash(state),
            Type::LiteralFloat(v) | Type::LiteralString(v) => v.hash(state),
            Type::LiteralSymbol(v) => v.hash(state),
            Type::Array(inner) => inner.hash(state),
            Type::Hash(k, v) => {
                k.hash(state);
                v.hash(state);
            }
            Type::Record(fields) => fields.hash(state),
            Type::Union(types) => types.hash(state),
            Type::Class(name)
            | Type::Singleton(name)
            | Type::KeywordParamRef(name)
            | Type::IvarRef(name)
            | Type::GlobalVariableRef(name) => name.hash(state),
            Type::ParamRef(idx) => idx.hash(state),
            Type::MethodReturnRef(c, m) => {
                c.hash(state);
                m.hash(state);
            }
            Type::BlockReturnRef => {}
            Type::PatternIndexRef(ty, idx) => {
                ty.hash(state);
                idx.hash(state);
            }
            Type::PatternRestRef(ty) => ty.hash(state),
            Type::PatternTrailingRef(ty, idx) => {
                ty.hash(state);
                idx.hash(state);
            }
            Type::PatternKeyRef(ty, key) | Type::PatternBindingKeyRef(ty, key) => {
                ty.hash(state);
                key.hash(state);
            }
            Type::PatternKeyRestRef(ty, keys) => {
                ty.hash(state);
                keys.hash(state);
            }
            Type::ReceiverMethodRef(t, m) => {
                t.hash(state);
                m.hash(state);
            }
            Type::DeferredCallRef { receiver, call } => {
                receiver.hash(state);
                call.hash(state);
            }
            Type::TruthyProjection(inner) | Type::FalsyProjection(inner) => inner.hash(state),
            Type::Proc {
                return_type,
                param_count,
            } => {
                return_type.hash(state);
                param_count.hash(state);
            }
            Type::Tuple(types) => types.hash(state),
            Type::Generic { base, args } => {
                base.hash(state);
                args.hash(state);
            }
            _ => {}
        }
    }
}

impl PartialOrd for Type {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Type {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let d = self.discriminant().cmp(&other.discriminant());
        if d != std::cmp::Ordering::Equal {
            return d;
        }
        self.cmp_inner(other)
    }
}

impl Type {
    fn discriminant(&self) -> u8 {
        match self {
            Type::Integer => 0,
            Type::Float => 1,
            Type::String => 2,
            Type::Symbol => 3,
            Type::Bool => 4,
            Type::True => 5,
            Type::False => 6,
            Type::Nil => 7,
            Type::Untyped => 8,
            Type::Todo => 9,
            Type::ParamPending => 44,
            Type::Void => 10,
            Type::LiteralInteger(_) => 11,
            Type::LiteralFloat(_) => 12,
            Type::LiteralString(_) => 13,
            Type::LiteralSymbol(_) => 14,
            Type::Array(_) => 15,
            Type::Hash(_, _) => 16,
            Type::Record(_) => 17,
            Type::Union(_) => 18,
            Type::Class(_) => 19,
            Type::Singleton(_) => 20,
            Type::ParamRef(_) => 21,
            Type::KeywordParamRef(_) => 22,
            Type::IvarRef(_) => 23,
            Type::GlobalVariableRef(_) => 37,
            Type::MethodReturnRef(_, _) => 24,
            Type::BlockReturnRef => 25,
            Type::PatternIndexRef(_, _) => 26,
            Type::PatternRestRef(_) => 27,
            Type::PatternTrailingRef(_, _) => 39,
            Type::PatternKeyRef(_, _) => 28,
            Type::PatternBindingKeyRef(_, _) => 42,
            Type::PatternKeyRestRef(_, _) => 29,
            Type::ReceiverMethodRef(_, _) => 30,
            Type::DeferredCallRef { .. } => 43,
            Type::TruthyProjection(_) => 40,
            Type::FalsyProjection(_) => 41,
            Type::SelfType => 31,
            Type::InstanceType => 38,
            Type::Proc { .. } => 32,
            Type::Top => 33,
            Type::Bot => 34,
            Type::Intersection(_) => 35,
            Type::Tuple(_) => 36,
            // Shares its discriminant with `Class` for ordering compatibility with the old `Class("Base[args]")` representation.
            Type::Generic { .. } => 19,
        }
    }

    fn cmp_inner(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, other) {
            (Type::LiteralInteger(a), Type::LiteralInteger(b)) => a.cmp(b),
            (Type::LiteralFloat(a), Type::LiteralFloat(b)) => a.cmp(b),
            (Type::LiteralString(a), Type::LiteralString(b)) => a.cmp(b),
            (Type::LiteralSymbol(a), Type::LiteralSymbol(b)) => a.cmp(b),
            (Type::Array(a), Type::Array(b)) => a.cmp(b),
            (Type::Hash(k1, v1), Type::Hash(k2, v2)) => k1.cmp(k2).then(v1.cmp(v2)),
            (Type::Record(a), Type::Record(b)) => a.len().cmp(&b.len()).then_with(|| {
                for (f1, f2) in a.iter().zip(b.iter()) {
                    let c = f1.cmp(f2);
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                Ordering::Equal
            }),
            (Type::Union(a), Type::Union(b)) => a.cmp(b),
            (Type::Class(a), Type::Class(b)) => a.cmp(b),
            (Type::Singleton(a), Type::Singleton(b)) => a.cmp(b),
            (Type::ParamRef(a), Type::ParamRef(b)) => a.cmp(b),
            (Type::KeywordParamRef(a), Type::KeywordParamRef(b)) => a.cmp(b),
            (Type::IvarRef(a), Type::IvarRef(b)) => a.cmp(b),
            (Type::MethodReturnRef(c1, m1), Type::MethodReturnRef(c2, m2)) => {
                c1.cmp(c2).then(m1.cmp(m2))
            }
            (Type::BlockReturnRef, Type::BlockReturnRef) => Ordering::Equal,
            (Type::PatternIndexRef(t1, i1), Type::PatternIndexRef(t2, i2)) => {
                t1.cmp(t2).then(i1.cmp(i2))
            }
            (Type::PatternRestRef(t1), Type::PatternRestRef(t2)) => t1.cmp(t2),
            (Type::PatternTrailingRef(t1, i1), Type::PatternTrailingRef(t2, i2)) => {
                t1.cmp(t2).then(i1.cmp(i2))
            }
            (Type::PatternKeyRef(t1, k1), Type::PatternKeyRef(t2, k2)) => {
                t1.cmp(t2).then(k1.cmp(k2))
            }
            (Type::PatternBindingKeyRef(t1, k1), Type::PatternBindingKeyRef(t2, k2)) => {
                t1.cmp(t2).then(k1.cmp(k2))
            }
            (Type::PatternKeyRestRef(t1, k1), Type::PatternKeyRestRef(t2, k2)) => {
                t1.cmp(t2).then(k1.cmp(k2))
            }
            (Type::ReceiverMethodRef(t1, m1), Type::ReceiverMethodRef(t2, m2)) => {
                t1.cmp(t2).then(m1.cmp(m2))
            }
            (
                Type::DeferredCallRef {
                    receiver: r1,
                    call: c1,
                },
                Type::DeferredCallRef {
                    receiver: r2,
                    call: c2,
                },
            ) => r1.cmp(r2).then(c1.cmp(c2)),
            (Type::TruthyProjection(t1), Type::TruthyProjection(t2))
            | (Type::FalsyProjection(t1), Type::FalsyProjection(t2)) => t1.cmp(t2),
            (
                Type::Proc {
                    return_type: a,
                    param_count: pa,
                },
                Type::Proc {
                    return_type: b,
                    param_count: pb,
                },
            ) => a.cmp(b).then(pa.cmp(pb)),
            (Type::Tuple(a), Type::Tuple(b)) => a.cmp(b),
            (Type::Intersection(a), Type::Intersection(b)) => a.cmp(b),
            // Since it shares a discriminant, comparisons involving `Generic` fall back to Display strings (compatible with the old flat representation).
            (Type::Generic { .. }, _) | (_, Type::Generic { .. }) => {
                self.to_string().cmp(&other.to_string())
            }
            _ => Ordering::Equal,
        }
    }
}

impl Type {
    pub(crate) fn replace_self_type(&self, self_type: &Type) -> Type {
        match self {
            Type::SelfType => self_type.clone(),
            Type::Union(parts) => Type::from_type_vec_preserve_untyped(
                parts
                    .iter()
                    .map(|part| part.replace_self_type(self_type))
                    .collect(),
            ),
            Type::Intersection(parts) => Type::Intersection(
                parts
                    .iter()
                    .map(|part| part.replace_self_type(self_type))
                    .collect(),
            ),
            Type::Array(Some(inner)) => {
                Type::Array(Some(Box::new(inner.replace_self_type(self_type))))
            }
            Type::Hash(Some(key), Some(value)) => Type::Hash(
                Some(Box::new(key.replace_self_type(self_type))),
                Some(Box::new(value.replace_self_type(self_type))),
            ),
            Type::Hash(Some(key), None) => {
                Type::Hash(Some(Box::new(key.replace_self_type(self_type))), None)
            }
            Type::Hash(None, Some(value)) => {
                Type::Hash(None, Some(Box::new(value.replace_self_type(self_type))))
            }
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: field.value.replace_self_type(self_type),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Proc {
                return_type,
                param_count,
            } => Type::Proc {
                return_type: Box::new(return_type.replace_self_type(self_type)),
                param_count: *param_count,
            },
            Type::DeferredCallRef { receiver, call } => Type::DeferredCallRef {
                receiver: Box::new(receiver.replace_self_type(self_type)),
                call: Box::new(DeferredCallArgs {
                    method: call.method,
                    args: call
                        .args
                        .iter()
                        .map(|arg| arg.replace_self_type(self_type))
                        .collect(),
                }),
            },
            Type::TruthyProjection(inner) => {
                Type::TruthyProjection(Box::new(inner.replace_self_type(self_type)))
            }
            Type::FalsyProjection(inner) => {
                Type::FalsyProjection(Box::new(inner.replace_self_type(self_type)))
            }
            Type::Tuple(types) => Type::Tuple(
                types
                    .iter()
                    .map(|ty| ty.replace_self_type(self_type))
                    .collect(),
            ),
            Type::Generic { base, args } => Type::Generic {
                base: *base,
                args: args
                    .iter()
                    .map(|ty| ty.replace_self_type(self_type))
                    .collect(),
            },
            _ => self.clone(),
        }
    }

    /// Gate to avoid triggering union renormalization for subtrees that don't contain `instance`.
    pub(crate) fn contains_instance_type(&self) -> bool {
        match self {
            Type::InstanceType => true,
            Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => {
                parts.iter().any(Type::contains_instance_type)
            }
            Type::Generic { args, .. } => args.iter().any(Type::contains_instance_type),
            Type::Array(Some(inner)) => inner.contains_instance_type(),
            Type::Hash(key, value) => {
                key.as_deref().is_some_and(Type::contains_instance_type)
                    || value.as_deref().is_some_and(Type::contains_instance_type)
            }
            Type::Record(fields) => fields.iter().any(|f| f.value.contains_instance_type()),
            Type::TruthyProjection(inner) | Type::FalsyProjection(inner) => {
                inner.contains_instance_type()
            }
            Type::Proc { return_type, .. } => return_type.contains_instance_type(),
            Type::DeferredCallRef { receiver, call } => {
                receiver.contains_instance_type()
                    || call.args.iter().any(Type::contains_instance_type)
            }
            _ => false,
        }
    }

    pub(crate) fn replace_instance_type(&self, instance_type: &Type) -> Type {
        if !self.contains_instance_type() {
            return self.clone();
        }
        match self {
            Type::InstanceType => instance_type.clone(),
            Type::Union(parts) => Type::from_type_vec_preserve_untyped(
                parts
                    .iter()
                    .map(|part| part.replace_instance_type(instance_type))
                    .collect(),
            ),
            Type::Intersection(parts) => Type::Intersection(
                parts
                    .iter()
                    .map(|part| part.replace_instance_type(instance_type))
                    .collect(),
            ),
            Type::Array(Some(inner)) => {
                Type::Array(Some(Box::new(inner.replace_instance_type(instance_type))))
            }
            Type::Hash(key, value) => Type::Hash(
                key.as_ref()
                    .map(|key| Box::new(key.replace_instance_type(instance_type))),
                value
                    .as_ref()
                    .map(|value| Box::new(value.replace_instance_type(instance_type))),
            ),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: field.value.replace_instance_type(instance_type),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Proc {
                return_type,
                param_count,
            } => Type::Proc {
                return_type: Box::new(return_type.replace_instance_type(instance_type)),
                param_count: *param_count,
            },
            Type::DeferredCallRef { receiver, call } => Type::DeferredCallRef {
                receiver: Box::new(receiver.replace_instance_type(instance_type)),
                call: Box::new(DeferredCallArgs {
                    method: call.method,
                    args: call
                        .args
                        .iter()
                        .map(|arg| arg.replace_instance_type(instance_type))
                        .collect(),
                }),
            },
            Type::TruthyProjection(inner) => {
                Type::TruthyProjection(Box::new(inner.replace_instance_type(instance_type)))
            }
            Type::FalsyProjection(inner) => {
                Type::FalsyProjection(Box::new(inner.replace_instance_type(instance_type)))
            }
            Type::Tuple(types) => Type::Tuple(
                types
                    .iter()
                    .map(|ty| ty.replace_instance_type(instance_type))
                    .collect(),
            ),
            Type::Generic { base, args } => Type::Generic {
                base: *base,
                args: args
                    .iter()
                    .map(|ty| ty.replace_instance_type(instance_type))
                    .collect(),
            },
            _ => self.clone(),
        }
    }

    fn is_booleanish(&self) -> bool {
        matches!(self, Type::Bool | Type::True | Type::False)
    }

    fn dedup_rendered_preserve_order(types: &[Type]) -> Vec<String> {
        let mut unique = Vec::new();
        for ty in types {
            let rendered = ty.to_string();
            if !unique.contains(&rendered) {
                unique.push(rendered);
            }
        }
        unique
    }

    pub fn union_with(self, other: Type) -> Type {
        let mut parts = Vec::new();
        Self::collect_union_parts(self, &mut parts);
        Self::collect_union_parts(other, &mut parts);
        Self::sort_dedup_parts(&mut parts);

        let had_untyped = parts.contains(&Type::Untyped);
        parts.retain(|ty| *ty != Type::Untyped);
        // Don't narrow `untyped | bot` down to bot (the untyped branch "may still return a value").
        if had_untyped
            && !parts.is_empty()
            && parts
                .iter()
                .all(|ty| matches!(ty, Type::Bot | Type::ParamPending))
        {
            return Type::Untyped;
        }
        if parts.len() > 1 {
            parts.retain(|ty| !matches!(ty, Type::Bot | Type::ParamPending));
            if parts.is_empty() && had_untyped {
                return Type::Untyped;
            }
            if parts.is_empty() {
                return Type::ParamPending;
            }
        }

        let types = Self::merge_array_union(parts);
        let types = Self::subsume_literals(types);
        match types.len() {
            0 => Type::Untyped,
            1 => types.into_iter().next().unwrap(),
            _ => Type::Union(types),
        }
    }

    fn collect_union_parts(ty: Type, parts: &mut Vec<Type>) {
        match ty {
            Type::Union(inner) => {
                for t in inner {
                    Self::collect_union_parts(t, parts);
                }
            }
            other => {
                parts.push(other);
            }
        }
    }

    fn sort_dedup_parts(parts: &mut Vec<Type>) {
        parts.sort_unstable();
        parts.dedup();
    }

    fn subsume_literals(types: Vec<Type>) -> Vec<Type> {
        let has_integer = types.iter().any(|t| matches!(t, Type::Integer));
        let has_float = types.iter().any(|t| matches!(t, Type::Float));
        let has_string = types.iter().any(|t| matches!(t, Type::String));
        let has_symbol = types.iter().any(|t| matches!(t, Type::Symbol));
        let has_bool = types.iter().any(|t| matches!(t, Type::Bool));

        types
            .into_iter()
            .filter(|t| match t {
                Type::LiteralInteger(_) => !has_integer,
                Type::LiteralFloat(_) => !has_float,
                Type::LiteralString(_) => !has_string,
                Type::LiteralSymbol(_) => !has_symbol,
                Type::True | Type::False => !has_bool,
                _ => true,
            })
            .collect()
    }

    fn merge_array_union(types: Vec<Type>) -> Vec<Type> {
        let mut array_like: Vec<Type> = Vec::new();
        let mut others: Vec<Type> = Vec::new();
        for t in types {
            if matches!(t, Type::Array(Some(_)) | Type::Tuple(_)) {
                array_like.push(t);
            } else {
                others.push(t);
            }
        }
        if array_like.len() > 1 {
            let all_tuples = array_like.iter().all(|t| matches!(t, Type::Tuple(_)));
            let keep_tuple_union = all_tuples && {
                let mut lens = array_like.iter().filter_map(|t| match t {
                    Type::Tuple(elems) => Some(elems.len()),
                    _ => None,
                });
                match lens.next() {
                    Some(len) if len > 0 => lens.all(|other| other == len),
                    _ => false,
                }
            };
            if keep_tuple_union {
                others.extend(array_like);
            } else {
                let mut elem_types = Vec::new();
                for a in array_like {
                    match a {
                        Type::Array(Some(inner)) => elem_types.push(*inner),
                        Type::Tuple(elems) => elem_types.extend(elems),
                        _ => {}
                    }
                }
                let merged_elem = Self::from_type_vec(elem_types);
                others.push(Type::Array(Some(Box::new(merged_elem))));
            }
        } else {
            others.extend(array_like);
        }
        others
    }

    fn merge_array_union_preserve_untyped(types: Vec<Type>) -> Vec<Type> {
        let mut array_like = Vec::new();
        let mut others = Vec::new();
        for ty in types {
            if matches!(ty, Type::Array(Some(_)) | Type::Tuple(_)) {
                array_like.push(ty);
            } else {
                others.push(ty);
            }
        }
        if array_like.len() > 1 {
            let all_tuples = array_like.iter().all(|ty| matches!(ty, Type::Tuple(_)));
            let keep_tuple_union = all_tuples && {
                let mut lengths = array_like.iter().filter_map(|ty| match ty {
                    Type::Tuple(elems) => Some(elems.len()),
                    _ => None,
                });
                match lengths.next() {
                    Some(length) if length > 0 => lengths.all(|other| other == length),
                    _ => false,
                }
            };
            if keep_tuple_union {
                others.extend(array_like);
            } else {
                let mut element_types = Vec::new();
                for ty in array_like {
                    match ty {
                        Type::Array(Some(inner)) => element_types.push(*inner),
                        Type::Tuple(elems) => element_types.extend(elems),
                        _ => {}
                    }
                }
                let merged_element = Self::from_type_vec_preserve_untyped(element_types);
                others.push(Type::Array(Some(Box::new(merged_element))));
            }
        } else {
            others.extend(array_like);
        }
        others
    }

    pub fn simple_array() -> Type {
        Type::Array(None)
    }

    pub fn simple_hash() -> Type {
        Type::Hash(None, None)
    }

    pub fn union_parts_saturated(types: &[Type]) -> bool {
        if types.len() <= Self::UNION_CARDINALITY_LIMIT {
            return false;
        }
        let mut sorted: Vec<&Type> = types.iter().collect();
        sorted.sort_unstable();
        sorted.dedup();
        sorted.len() > Self::UNION_CARDINALITY_LIMIT
    }

    // Avoids sorting on every append to stay O(N) (merging param slots for popular methods was quadratic otherwise).
    pub fn append_union_parts(types: &mut Vec<Type>, new_type: Type) {
        let tail_start = types.len();
        Self::collect_union_parts(new_type, types);
        let mut idx = tail_start;
        while idx < types.len() {
            if types[idx] == Type::Untyped {
                types.swap_remove(idx);
            } else {
                idx += 1;
            }
        }
        if types.len() > Self::UNION_DEDUP_FLUSH_LIMIT {
            Self::sort_dedup_parts(types);
        }
    }

    pub fn append_param_arg_parts(types: &mut Vec<Type>, new_type: Type) {
        Self::collect_union_parts(new_type, types);
        if types.len() > Self::UNION_DEDUP_FLUSH_LIMIT {
            Self::sort_dedup_parts(types);
        }
    }

    pub fn merge_into_vec(types: &mut Vec<Type>, new_type: Type) {
        let mut parts = Vec::with_capacity(types.len() + 1);
        for ty in types.drain(..) {
            Self::collect_union_parts(ty, &mut parts);
        }
        Self::collect_union_parts(new_type, &mut parts);
        Self::sort_dedup_parts(&mut parts);
        parts.retain(|ty| *ty != Type::Untyped);

        *types = parts;
    }

    // Exceeding the union cap degrades to `untyped` (keeps RBS output and sort/dedup cost bounded).
    pub const UNION_CARDINALITY_LIMIT: usize = 4096;
    const UNION_DEDUP_FLUSH_LIMIT: usize = Self::UNION_CARDINALITY_LIMIT * 2;

    pub fn from_type_vec(types: Vec<Type>) -> Type {
        let mut parts = Vec::new();
        for t in types {
            Self::collect_union_parts(t, &mut parts);
            if parts.len() > Self::UNION_DEDUP_FLUSH_LIMIT {
                Self::sort_dedup_parts(&mut parts);
                if parts.len() > Self::UNION_CARDINALITY_LIMIT {
                    return Type::Untyped;
                }
            }
        }
        Self::sort_dedup_parts(&mut parts);
        if parts.len() > Self::UNION_CARDINALITY_LIMIT {
            return Type::Untyped;
        }
        let had_untyped = parts.contains(&Type::Untyped);
        parts.retain(|ty| *ty != Type::Untyped);
        // Don't narrow `nil | untyped` down to nil (if all that's left is nil, it's still "unknown").
        if had_untyped && !parts.is_empty() && parts.iter().all(|ty| *ty == Type::Nil) {
            return Type::Untyped;
        }
        // Don't narrow `untyped | bot` down to bot either (same rule as `union_with`).
        if had_untyped
            && !parts.is_empty()
            && parts
                .iter()
                .all(|ty| matches!(ty, Type::Bot | Type::ParamPending))
        {
            return Type::Untyped;
        }
        if parts.len() > 1 {
            parts.retain(|ty| !matches!(ty, Type::Bot | Type::ParamPending));
            if parts.is_empty() {
                return if had_untyped {
                    Type::Untyped
                } else {
                    Type::ParamPending
                };
            }
        }

        let result = Self::merge_array_union(parts);
        let result = Self::subsume_literals(result);
        match result.len() {
            0 => Type::Untyped,
            1 => result.into_iter().next().unwrap(),
            _ => Type::Union(result),
        }
    }

    pub fn from_type_vec_preserve_untyped(types: Vec<Type>) -> Type {
        let mut parts = Vec::new();
        for t in types {
            Self::collect_union_parts(t, &mut parts);
            if parts.len() > Self::UNION_DEDUP_FLUSH_LIMIT {
                Self::sort_dedup_parts(&mut parts);
                if parts.len() > Self::UNION_CARDINALITY_LIMIT {
                    return Type::Untyped;
                }
            }
        }
        Self::sort_dedup_parts(&mut parts);
        if parts.len() > Self::UNION_CARDINALITY_LIMIT {
            return Type::Untyped;
        }
        if parts.len() > 1 {
            parts.retain(|ty| !matches!(ty, Type::Bot | Type::ParamPending));
            if parts.is_empty() {
                return Type::ParamPending;
            }
        }

        let result = Self::merge_array_union_preserve_untyped(parts);
        let result = Self::subsume_literals(result);
        match result.len() {
            0 => Type::Untyped,
            1 => result.into_iter().next().unwrap(),
            _ => Type::Union(result),
        }
    }

    pub fn widen_tuple_to_array(self) -> Type {
        match self {
            Type::Tuple(elems) => {
                let widened_elems: Vec<Type> = elems
                    .into_iter()
                    .map(|e| e.widen_tuple_to_array())
                    .collect();
                let elem_ty = Self::from_type_vec(widened_elems);
                Type::Array(Some(Box::new(elem_ty)))
            }
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(inner.widen_tuple_to_array()))),
            Type::Union(parts) => {
                let widened: Vec<Type> = parts
                    .into_iter()
                    .map(|p| p.widen_tuple_to_array())
                    .collect();
                Self::from_type_vec(widened)
            }
            other => other,
        }
    }

    // Equal-length Tuples are unioned positionally to preserve shape; a length mismatch or mixed-in plain Array degrades to Array.
    pub fn merge_param_arg_vec(types: Vec<Type>) -> Type {
        if types.is_empty() {
            return Type::Untyped;
        }
        if !types.iter().any(Self::contains_untyped_leaf) {
            return Self::merge_param_arg_vec_without_untyped(types);
        }
        let mut flat: Vec<Type> = Vec::new();
        for t in types {
            Self::collect_union_parts(t, &mut flat);
        }
        Self::merge_param_arg_parts(flat)
    }

    fn merge_param_arg_vec_without_untyped(types: Vec<Type>) -> Type {
        let mut flat: Vec<Type> = Vec::new();
        for ty in types {
            Self::collect_union_parts(ty, &mut flat);
        }

        let mut tuple_groups = Vec::new();
        let mut has_plain_array = false;
        let mut others = Vec::new();
        for part in flat {
            match part {
                Type::Tuple(elems) => tuple_groups.push(elems),
                array @ Type::Array(Some(_)) => {
                    has_plain_array = true;
                    others.push(array);
                }
                other => others.push(other),
            }
        }

        if !tuple_groups.is_empty() {
            let len = tuple_groups[0].len();
            let uniform = !has_plain_array && tuple_groups.iter().all(|tuple| tuple.len() == len);
            if uniform {
                let columns = (0..len)
                    .map(|index| {
                        Self::from_type_vec(
                            tuple_groups
                                .iter()
                                .map(|tuple| tuple[index].clone())
                                .collect(),
                        )
                    })
                    .collect();
                others.push(Type::Tuple(columns));
            } else {
                let elements = tuple_groups.into_iter().flatten().collect();
                others.push(Type::Array(Some(Box::new(Self::from_type_vec(elements)))));
            }
        }

        Self::from_type_vec(others)
    }

    pub(crate) fn contains_untyped_leaf(ty: &Type) -> bool {
        match ty {
            Type::Untyped => true,
            Type::Intersection(parts) | Type::Union(parts) | Type::Tuple(parts) => {
                parts.iter().any(Self::contains_untyped_leaf)
            }
            Type::Array(Some(inner))
            | Type::PatternRestRef(inner)
            | Type::TruthyProjection(inner)
            | Type::FalsyProjection(inner) => Self::contains_untyped_leaf(inner),
            Type::Hash(key, value) => {
                key.as_deref().is_some_and(Self::contains_untyped_leaf)
                    || value.as_deref().is_some_and(Self::contains_untyped_leaf)
            }
            Type::Record(fields) => fields
                .iter()
                .any(|field| Self::contains_untyped_leaf(&field.value)),
            Type::Generic { args, .. } => args.iter().any(Self::contains_untyped_leaf),
            Type::Proc { return_type, .. } => Self::contains_untyped_leaf(return_type),
            Type::PatternIndexRef(inner, _) | Type::PatternTrailingRef(inner, _) => {
                Self::contains_untyped_leaf(inner)
            }
            Type::PatternKeyRef(inner, _)
            | Type::PatternBindingKeyRef(inner, _)
            | Type::PatternKeyRestRef(inner, _)
            | Type::ReceiverMethodRef(inner, _) => Self::contains_untyped_leaf(inner),
            Type::DeferredCallRef { receiver, call } => {
                Self::contains_untyped_leaf(receiver)
                    || call.args.iter().any(Self::contains_untyped_leaf)
            }
            _ => false,
        }
    }

    fn merge_param_arg_parts(mut parts: Vec<Type>) -> Type {
        Self::sort_dedup_parts(&mut parts);
        let had_pending = parts.contains(&Type::ParamPending);
        parts.retain(|ty| *ty != Type::ParamPending);
        if parts.is_empty() && had_pending {
            return Type::ParamPending;
        }
        if had_pending && parts.iter().all(|ty| *ty == Type::Bot) {
            return Type::ParamPending;
        }
        if parts.len() > 1 {
            parts.retain(|ty| *ty != Type::Bot);
        }
        if parts.is_empty() {
            return if had_pending {
                Type::ParamPending
            } else {
                Type::Bot
            };
        }
        if parts.len() > Self::UNION_CARDINALITY_LIMIT {
            return Type::Untyped;
        }

        if !parts.iter().any(|part| {
            matches!(
                part,
                Type::Array(Some(_))
                    | Type::Tuple(_)
                    | Type::Hash(_, _)
                    | Type::Generic { .. }
                    | Type::Record(_)
            )
        }) {
            let mut parts = Self::subsume_literals(parts);
            return match parts.len() {
                0 => Type::Untyped,
                1 => parts.pop().unwrap_or(Type::Untyped),
                _ => Type::Union(parts),
            };
        }

        let mut array_parts = Vec::new();
        let mut structural: HashMap<ParamArgShape, Vec<Type>> = HashMap::new();
        let mut others = Vec::new();
        for part in parts {
            match part {
                Type::Array(Some(_)) | Type::Tuple(_) => array_parts.push(part),
                Type::Hash(key, value) => {
                    let shape = ParamArgShape::Hash(key.is_some(), value.is_some());
                    structural
                        .entry(shape)
                        .or_default()
                        .push(Type::Hash(key, value));
                }
                generic @ Type::Generic { .. } => others.push(generic),
                Type::Record(fields) => {
                    let shape = ParamArgShape::Record(
                        fields
                            .iter()
                            .map(|field| (field.key.clone(), field.optional))
                            .collect(),
                    );
                    structural
                        .entry(shape)
                        .or_default()
                        .push(Type::Record(fields));
                }
                other => others.push(other),
            }
        }

        for (shape, mut group) in structural {
            if group.len() == 1 {
                if let Some(ty) = group.pop() {
                    others.push(ty);
                }
                continue;
            }
            match shape {
                ParamArgShape::Hash(has_key, has_value) => {
                    let mut keys = Vec::new();
                    let mut values = Vec::new();
                    for ty in group {
                        let (key, value) = match ty {
                            Type::Hash(key, value) => (key, value),
                            other => {
                                others.push(other);
                                continue;
                            }
                        };
                        if let Some(key) = key {
                            keys.push(*key);
                        }
                        if let Some(value) = value {
                            values.push(*value);
                        }
                    }
                    let key = has_key.then(|| Box::new(Self::merge_param_arg_vec(keys)));
                    let value = has_value.then(|| Box::new(Self::merge_param_arg_vec(values)));
                    others.push(Type::Hash(key, value));
                }
                ParamArgShape::Record(shape) => {
                    let Some(Type::Record(mut fields)) = group.pop() else {
                        others.extend(group);
                        continue;
                    };
                    for (index, field) in fields.iter_mut().enumerate() {
                        let values = group
                            .iter()
                            .filter_map(|ty| {
                                let Type::Record(other_fields) = ty else {
                                    return None;
                                };
                                other_fields.get(index).map(|other| other.value.clone())
                            })
                            .chain(std::iter::once(field.value.clone()))
                            .collect();
                        field.value = Self::merge_param_arg_vec(values);
                    }
                    debug_assert_eq!(
                        fields
                            .iter()
                            .map(|field| (field.key.clone(), field.optional))
                            .collect::<Vec<_>>(),
                        shape
                    );
                    others.push(Type::Record(fields));
                }
            }
        }

        if !array_parts.is_empty() {
            let mut tuples = Vec::new();
            let mut elements = Vec::new();
            let mut arrays = Vec::new();
            for part in array_parts {
                match part {
                    Type::Tuple(parts) => tuples.push(parts),
                    Type::Array(Some(inner)) => arrays.push(*inner),
                    other => others.push(other),
                }
            }
            if tuples.is_empty() && arrays.len() == 1 {
                if let Some(inner) = arrays.pop() {
                    others.push(Type::Array(Some(Box::new(inner))));
                }
            } else if tuples.is_empty() {
                others.push(Type::Array(Some(Box::new(Self::merge_param_arg_vec(
                    arrays,
                )))));
            } else {
                let tuple_len = tuples.first().map(Vec::len).unwrap_or_default();
                let uniform_tuples =
                    arrays.is_empty() && tuples.iter().all(|tuple| tuple.len() == tuple_len);
                if uniform_tuples {
                    let columns = (0..tuple_len)
                        .map(|index| {
                            Self::merge_param_arg_vec(
                                tuples
                                    .iter()
                                    .filter_map(|tuple| tuple.get(index).cloned())
                                    .collect(),
                            )
                        })
                        .collect();
                    others.push(Type::Tuple(columns));
                } else {
                    elements.extend(arrays);
                    elements.extend(tuples.into_iter().flatten());
                    others.push(Type::Array(Some(Box::new(Self::merge_param_arg_vec(
                        elements,
                    )))));
                }
            }
        }

        Self::sort_dedup_parts(&mut others);
        if others.len() > Self::UNION_CARDINALITY_LIMIT {
            return Type::Untyped;
        }
        let others = Self::subsume_literals(others);
        match others.len() {
            0 => Type::Untyped,
            1 => others.into_iter().next().unwrap_or(Type::Untyped),
            _ => Type::Union(others),
        }
    }

    pub fn widen_param_type(self) -> Type {
        match self {
            Type::LiteralInteger(_) => Type::Integer,
            Type::LiteralFloat(_) => Type::Float,
            Type::LiteralString(_) => Type::String,
            Type::LiteralSymbol(_) => Type::Symbol,
            Type::True | Type::False => Type::Bool,
            Type::Record(fields) => Type::Record(
                fields
                    .into_iter()
                    .map(|field| RecordField {
                        value: field.value.widen_param_type(),
                        key: field.key,
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Tuple(elems) => {
                let widened: Vec<Type> = elems.into_iter().map(Type::widen_param_type).collect();
                match widened.first() {
                    Some(first) if widened.iter().all(|elem| elem == first) => {
                        Type::Array(Some(Box::new(first.clone())))
                    }
                    _ => Type::Tuple(widened),
                }
            }
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(inner.widen_param_type()))),
            Type::Hash(key, value) => Type::Hash(
                key.map(|key| Box::new(key.widen_param_type())),
                value.map(|value| Box::new(value.widen_param_type())),
            ),
            Type::Union(parts) => {
                Self::merge_param_arg_vec(parts.into_iter().map(Type::widen_param_type).collect())
            }
            Type::Intersection(parts) => {
                Type::Intersection(parts.into_iter().map(Type::widen_param_type).collect())
            }
            Type::Generic { base, args } => Type::Generic {
                base,
                args: args
                    .into_vec()
                    .into_iter()
                    .map(Type::widen_param_type)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
            other => other,
        }
    }

    // Widens scalars/literals to their class; a heterogeneous Tuple keeps its shape, a homogeneous one becomes an Array.
    pub fn widen_arg_for_param(self) -> Type {
        match self {
            Type::LiteralInteger(_) => Type::Integer,
            Type::LiteralFloat(_) => Type::Float,
            Type::LiteralString(_) => Type::String,
            Type::LiteralSymbol(_) => Type::Symbol,
            Type::True | Type::False => Type::Bool,
            Type::Record(fields) => Type::Record(
                fields
                    .into_iter()
                    .map(|field| RecordField {
                        value: field.value.widen_arg_for_param(),
                        key: field.key,
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Tuple(elems) => {
                let widened: Vec<Type> = elems.into_iter().map(Type::widen_arg_for_param).collect();
                // A homogeneous tuple becomes an Array to avoid overfitting to a fixed arity.
                match widened.first() {
                    Some(first) if widened.iter().all(|e| e == first) => {
                        Type::Array(Some(Box::new(widened.into_iter().next().unwrap())))
                    }
                    _ => Type::Tuple(widened),
                }
            }
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(inner.widen_arg_for_param()))),
            Type::Hash(k, v) => Type::Hash(
                k.map(|k| Box::new(k.widen_arg_for_param())),
                v.map(|v| Box::new(v.widen_arg_for_param())),
            ),
            Type::Intersection(types) => {
                Type::Intersection(types.into_iter().map(|t| t.widen_arg_for_param()).collect())
            }
            Type::Union(parts) => Self::merge_param_arg_vec(
                parts.into_iter().map(Type::widen_arg_for_param).collect(),
            ),
            Type::Generic { base, args } => Type::Generic {
                base,
                args: args.into_iter().map(Type::widen_arg_for_param).collect(),
            },
            other => other,
        }
    }

    pub fn widen(&self) -> Type {
        match self {
            Type::LiteralInteger(_) => Type::Integer,
            Type::LiteralFloat(_) => Type::Float,
            Type::LiteralString(_) => Type::String,
            Type::LiteralSymbol(_) => Type::Symbol,
            Type::True | Type::False => Type::Bool,
            Type::Union(types) => {
                let widened: Vec<Type> = types.iter().map(|t| t.widen()).collect();
                Self::from_type_vec(widened)
            }
            Type::Intersection(types) => {
                Type::Intersection(types.iter().map(|t| t.widen()).collect())
            }
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(inner.widen()))),
            Type::Hash(Some(k), Some(v)) => {
                Type::Hash(Some(Box::new(k.widen())), Some(Box::new(v.widen())))
            }
            Type::Hash(Some(k), None) => Type::Hash(Some(Box::new(k.widen())), None),
            Type::Hash(None, Some(v)) => Type::Hash(None, Some(Box::new(v.widen()))),
            Type::Tuple(types) => Type::Tuple(types.iter().map(|t| t.widen()).collect()),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: field.value.widen(),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Generic { base, args } => Type::Generic {
                base: *base,
                args: args.iter().map(|t| t.widen()).collect(),
            },
            _ => self.clone(),
        }
    }

    pub fn class_like_base(&self) -> Option<&str> {
        match self {
            Type::Class(name) | Type::Singleton(name) => Some(name.as_str()),
            Type::Generic { base, .. } => Some(base.as_str()),
            _ => None,
        }
    }

    pub fn generic_args(&self) -> &[Type] {
        match self {
            Type::Generic { args, .. } => args,
            _ => &[],
        }
    }

    #[inline]
    pub fn generic(base: Sym, args: impl Into<Box<[Type]>>) -> Type {
        Type::Generic {
            base,
            args: args.into(),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Integer => write!(f, "Integer"),
            Type::Float => write!(f, "Float"),
            Type::String => write!(f, "String"),
            Type::Symbol => write!(f, "Symbol"),
            Type::Bool => write!(f, "bool"),
            Type::True => write!(f, "true"),
            Type::False => write!(f, "false"),
            Type::Nil => write!(f, "nil"),
            Type::Untyped => write!(f, "untyped"),
            Type::Todo => write!(f, "__todo__"),
            Type::ParamPending => write!(f, "untyped"),
            Type::Void => write!(f, "void"),
            Type::Top => write!(f, "top"),
            Type::Bot => write!(f, "bot"),
            Type::Intersection(types) => {
                let parts = Self::dedup_rendered_preserve_order(types);
                write!(f, "{}", parts.join(" & "))
            }
            Type::LiteralInteger(v) => write!(f, "{v}"),
            Type::LiteralFloat(v) => write!(f, "{v}"),
            Type::LiteralString(v) => {
                write!(f, "\"{}\"", escape_rbs_string_literal(v))
            }
            Type::LiteralSymbol(v) => write!(f, ":{v}"),
            Type::Array(None) => write!(f, "Array"),
            Type::Array(Some(inner)) => write!(f, "Array[{inner}]"),
            Type::Hash(None, None) => write!(f, "Hash"),
            Type::Hash(Some(k), Some(v)) => write!(f, "Hash[{k}, {v}]"),
            Type::Hash(Some(k), None) => write!(f, "Hash[{k}, untyped]"),
            Type::Hash(None, Some(v)) => write!(f, "Hash[untyped, {v}]"),
            Type::Record(fields) => {
                write!(f, "{{ ")?;
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{field}")?;
                }
                write!(f, " }}")
            }
            Type::Union(types) => {
                let has_nil = types.iter().any(|t| matches!(t, Type::Nil));
                let non_nil: Vec<&Type> =
                    types.iter().filter(|t| !matches!(t, Type::Nil)).collect();
                let boolean_union =
                    non_nil.len() > 1 && non_nil.iter().all(|ty| ty.is_booleanish());
                let absorbs_nil = non_nil
                    .iter()
                    .any(|t| matches!(t, Type::Untyped | Type::Top | Type::Bot));
                if boolean_union {
                    if has_nil {
                        write!(f, "bool?")
                    } else {
                        write!(f, "bool")
                    }
                } else if has_nil
                    && non_nil.len() == 1
                    && matches!(non_nil[0], Type::True | Type::False)
                {
                    write!(f, "{} | nil", non_nil[0])
                } else if has_nil && !non_nil.is_empty() && !absorbs_nil {
                    let non_nil_parts = Self::dedup_rendered_preserve_order(
                        &non_nil.iter().copied().cloned().collect::<Vec<_>>(),
                    );
                    let needs_parens = non_nil_parts.len() > 1
                        || non_nil.len() == 1 && matches!(non_nil[0], Type::Intersection(_));
                    if needs_parens {
                        write!(f, "({})?", non_nil_parts.join(" | "))
                    } else {
                        write!(f, "{}?", non_nil_parts[0])
                    }
                } else {
                    let needs_parens = types.len() > 1;
                    if needs_parens && f.alternate() {
                        write!(f, "(")?;
                    }
                    let parts = Self::dedup_rendered_preserve_order(types);
                    write!(f, "{}", parts.join(" | "))?;
                    if needs_parens && f.alternate() {
                        write!(f, ")")?;
                    }
                    Ok(())
                }
            }
            Type::Class(name) if name.starts_with("MatchData[") => write!(f, "MatchData"),
            Type::Class(name) => write!(f, "{name}"),
            Type::Singleton(name) => write!(f, "singleton({name})"),
            Type::ParamRef(_) => write!(f, "untyped"),
            Type::KeywordParamRef(_) => write!(f, "untyped"),
            Type::IvarRef(_) => write!(f, "untyped"),
            Type::GlobalVariableRef(_) => write!(f, "untyped"),
            Type::MethodReturnRef(_, _) => write!(f, "untyped"),
            Type::BlockReturnRef => write!(f, "untyped"),
            Type::PatternIndexRef(_, _) => write!(f, "untyped"),
            Type::PatternRestRef(_) => write!(f, "untyped"),
            Type::PatternTrailingRef(_, _) => write!(f, "untyped"),
            Type::PatternKeyRef(_, _) => write!(f, "untyped"),
            Type::PatternBindingKeyRef(_, _) => write!(f, "untyped"),
            Type::PatternKeyRestRef(_, _) => write!(f, "untyped"),
            Type::ReceiverMethodRef(_, _) | Type::DeferredCallRef { .. } => {
                write!(f, "untyped")
            }
            Type::TruthyProjection(_) | Type::FalsyProjection(_) => write!(f, "untyped"),
            Type::SelfType => write!(f, "self"),
            Type::InstanceType => write!(f, "instance"),
            Type::Proc { .. } => write!(f, "Proc"),
            Type::Tuple(types) => {
                if types.is_empty() {
                    return write!(f, "[ ]");
                }
                write!(f, "[")?;
                let parts: Vec<std::string::String> = types.iter().map(|t| t.to_string()).collect();
                write!(f, "{}", parts.join(", "))?;
                write!(f, "]")
            }
            // MatchData keeps its capture shape internally but drops it for display (compatible with the old flat representation).
            Type::Generic { base, .. } if base.as_str() == "MatchData" => write!(f, "MatchData"),
            Type::Generic { base, args } => {
                if args.is_empty() {
                    return write!(f, "{base}");
                }
                let parts: Vec<std::string::String> = args.iter().map(|t| t.to_string()).collect();
                write!(f, "{base}[{}]", parts.join(", "))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceLocation {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamKind {
    Required,
    Optional,
    Rest,
    KeywordRequired,
    KeywordOptional,
    DoubleRest,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HoverBlockSig {
    pub params: Vec<Param>,
    pub return_type: Type,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub struct MethodSig {
    pub name: std::string::String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub block: Option<HoverBlockSig>,
    pub sorbet_modifier_comments: Vec<std::string::String>,
    pub is_singleton: bool,
    pub rbs_annotated: bool,
    pub rbs_inline_annotated: bool,
    pub sig_annotated: bool,
    pub rbs_file_source: bool,
    pub synthetic_dsl_source: bool,
    pub overloads: Vec<OverloadSig>,
    pub loc: Option<SourceLocation>,
    // RBS has no `protected`, so private/protected both collapse to private.
    pub is_private: bool,
}

impl MethodSig {
    pub fn has_explicit_signature(&self) -> bool {
        self.rbs_inline_annotated
            || self.sig_annotated
            || self.rbs_file_source && !self.synthetic_dsl_source
    }

    pub fn is_external_rbs_source(&self) -> bool {
        self.rbs_file_source && !self.synthetic_dsl_source
    }
}

#[derive(Debug, Clone)]
pub struct OverloadSig {
    pub params: Vec<Param>,
    pub return_type: Type,
    pub block: Option<HoverBlockSig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HoverOverloadSig {
    pub method_type_params: Vec<Type>,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub block: Option<HoverBlockSig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Param {
    pub name: std::string::String,
    pub param_type: Type,
    pub kind: ParamKind,
}

#[derive(Debug, Clone)]
pub struct ConstantSig {
    pub name: std::string::String,
    pub const_type: Type,
    pub loc: Option<SourceLocation>,
    pub file_path: Option<std::string::String>,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: std::string::String,
    pub type_params: Vec<std::string::String>,
    pub methods: Vec<MethodSig>,
    pub aliases: Vec<MethodAliasSig>,
    pub constants: Vec<ConstantSig>,
    pub sorbet_modifier_comments: Vec<std::string::String>,
    pub superclass: Option<std::string::String>,
    pub mixins: Vec<(std::string::String, std::string::String)>,
    pub is_module: bool,
    pub loc: Option<SourceLocation>,
    pub file_path: Option<std::string::String>,
}

#[derive(Clone, Debug)]
pub struct MethodAliasSig {
    pub new_name: std::string::String,
    pub old_name: std::string::String,
    pub is_singleton: bool,
    pub loc: Option<SourceLocation>,
}

#[cfg(test)]
mod tests {
    use super::{RecordField, RecordKey, Sym, Type};

    #[test]
    fn display_deduplicates_union_members() {
        let ty = Type::Union(vec![Type::Untyped, Type::Untyped, Type::String]);
        assert_eq!(ty.to_string(), "untyped | String");
    }

    #[test]
    fn union_of_untyped_and_bot_stays_untyped() {
        // Don't narrow `untyped ∪ bot` down to bot.
        assert_eq!(Type::Untyped.union_with(Type::Bot), Type::Untyped);
        assert_eq!(
            Type::from_type_vec(vec![Type::Untyped, Type::Bot]),
            Type::Untyped
        );
        // bot alone, or merged with a concrete type, behaves as before.
        assert_eq!(Type::Bot.union_with(Type::Bot), Type::Bot);
        assert_eq!(
            Type::from_type_vec(vec![Type::String, Type::Bot]),
            Type::String
        );
    }

    #[test]
    fn display_deduplicates_intersection_members() {
        let ty = Type::Intersection(vec![Type::String, Type::String, Type::Untyped]);
        assert_eq!(ty.to_string(), "String & untyped");
    }

    #[test]
    fn display_renders_todo_distinct_from_untyped() {
        assert_eq!(Type::Todo.to_string(), "__todo__");
        assert_eq!(Type::Untyped.to_string(), "untyped");
        assert_eq!(Type::ParamPending.to_string(), "untyped");
    }

    #[test]
    fn pending_parameter_slots_are_bottom_until_finalized() {
        assert_eq!(
            Type::from_type_vec_preserve_untyped(vec![Type::ParamPending]),
            Type::ParamPending
        );
        assert_eq!(
            Type::from_type_vec_preserve_untyped(vec![Type::Bot, Type::ParamPending]),
            Type::ParamPending
        );
        assert_eq!(
            Type::merge_param_arg_vec(vec![Type::Integer, Type::ParamPending]),
            Type::Integer
        );
        assert_eq!(
            Type::merge_param_arg_vec(vec![Type::Bot, Type::ParamPending]),
            Type::ParamPending
        );
        assert_eq!(
            Type::from_type_vec_preserve_untyped(vec![Type::String, Type::ParamPending]),
            Type::String
        );
        assert_eq!(
            Type::from_type_vec_preserve_untyped(vec![Type::ParamPending, Type::Untyped]),
            Type::Untyped
        );
    }

    #[test]
    fn pending_parameter_is_detected_inside_structural_types() {
        let pending = Type::generic(Sym::new("Box"), [Type::ParamPending]);
        assert!(pending.contains_param_pending());
        assert!(!Type::generic(Sym::new("Box"), [Type::String]).contains_param_pending());
    }

    #[test]
    fn display_normalizes_boolean_union_to_bool() {
        let ty = Type::Union(vec![Type::True, Type::False]);
        assert_eq!(ty.to_string(), "bool");
    }

    #[test]
    fn display_normalizes_nilable_boolean_union_to_bool_optional() {
        let ty = Type::Union(vec![Type::True, Type::False, Type::Nil]);
        assert_eq!(ty.to_string(), "bool?");
    }

    #[test]
    fn display_keeps_true_nil_union_explicit() {
        let ty = Type::Union(vec![Type::True, Type::Nil]);
        assert_eq!(ty.to_string(), "true | nil");
    }

    #[test]
    fn from_type_vec_deduplicates_before_cardinality_cap() {
        let ty = Type::from_type_vec(vec![Type::String; Type::UNION_DEDUP_FLUSH_LIMIT + 1]);
        assert_eq!(ty, Type::String);
    }

    #[test]
    fn parameter_widening_preserves_unknown_inside_arrays() {
        let observed = vec![
            Type::Array(Some(Box::new(Type::LiteralString("known".into())))),
            Type::Array(Some(Box::new(Type::Untyped))),
        ];
        assert_eq!(
            Type::merge_param_arg_vec(observed.clone()).widen_param_type(),
            Type::Array(Some(Box::new(Type::Union(vec![
                Type::String,
                Type::Untyped,
            ])))),
        );
        assert_eq!(
            Type::from_type_vec(observed),
            Type::Array(Some(Box::new(Type::LiteralString("known".into())))),
            "ordinary value unions keep their existing behavior"
        );
    }

    #[test]
    fn parameter_argument_widening_preserves_unknown_union_members() {
        assert_eq!(
            Type::from_type_vec_preserve_untyped(vec![Type::LiteralInteger(1), Type::Untyped])
                .widen_arg_for_param(),
            Type::Union(vec![Type::Integer, Type::Untyped]),
        );
        assert_eq!(
            Type::from_type_vec(vec![Type::LiteralInteger(1), Type::Untyped]),
            Type::LiteralInteger(1),
            "ordinary expression unions keep their existing behavior"
        );
    }

    #[test]
    fn preserving_union_keeps_unknown_array_elements() {
        let observed = vec![
            Type::Array(Some(Box::new(Type::String))),
            Type::Array(Some(Box::new(Type::Untyped))),
        ];
        let expected = Type::Array(Some(Box::new(Type::Union(vec![
            Type::String,
            Type::Untyped,
        ]))));

        assert_eq!(
            Type::from_type_vec_preserve_untyped(observed.clone()),
            expected
        );
        assert_eq!(
            Type::from_type_vec(observed),
            Type::Array(Some(Box::new(Type::String))),
            "ordinary value unions keep their existing behavior"
        );
    }

    #[test]
    fn contains_untyped_leaf_recurses_through_structural_types() {
        let nested = Type::Record(vec![RecordField {
            key: RecordKey::Symbol("value".into()),
            value: Type::generic(
                Sym::new("Box"),
                [Type::Array(Some(Box::new(Type::Union(vec![
                    Type::String,
                    Type::Untyped,
                ]))))],
            ),
            optional: false,
        }]);

        assert!(Type::contains_untyped_leaf(&nested));
        assert!(Type::contains_untyped_leaf(&Type::ReceiverMethodRef(
            Box::new(Type::Array(Some(Box::new(Type::Untyped)))),
            Sym::new("value"),
        )));
        assert!(Type::contains_untyped_leaf(&Type::deferred_call_ref(
            Type::Class(Sym::new("Integer")),
            Sym::new("+"),
            vec![Type::Untyped],
        )));
        assert!(!Type::contains_untyped_leaf(&Type::Record(vec![
            RecordField {
                key: RecordKey::Symbol("value".into()),
                value: Type::Array(Some(Box::new(Type::String))),
                optional: false,
            }
        ])));
    }

    #[test]
    fn parameter_widening_keeps_intersection_behavior() {
        assert_eq!(
            Type::Intersection(vec![Type::LiteralString("known".into()), Type::Untyped])
                .widen_param_type(),
            Type::Intersection(vec![Type::String, Type::Untyped]),
        );
    }

    #[test]
    fn parameter_merge_keeps_regular_structural_unions_unchanged() {
        let hash_types = vec![
            Type::Hash(Some(Box::new(Type::Symbol)), Some(Box::new(Type::String))),
            Type::Hash(Some(Box::new(Type::Symbol)), Some(Box::new(Type::Integer))),
        ];
        assert_eq!(
            Type::merge_param_arg_vec(hash_types.clone()),
            Type::from_type_vec(hash_types)
        );

        let generic_types = vec![
            Type::generic(Sym::new("Box"), [Type::String]),
            Type::generic(Sym::new("Box"), [Type::Integer]),
        ];
        assert_eq!(
            Type::merge_param_arg_vec(generic_types.clone()),
            Type::from_type_vec(generic_types)
        );

        let record_types = vec![
            Type::Record(vec![RecordField {
                key: RecordKey::Symbol("value".into()),
                value: Type::String,
                optional: false,
            }]),
            Type::Record(vec![RecordField {
                key: RecordKey::Symbol("value".into()),
                value: Type::Integer,
                optional: false,
            }]),
        ];
        assert_eq!(
            Type::merge_param_arg_vec(record_types.clone()),
            Type::from_type_vec(record_types)
        );
    }

    #[test]
    fn parameter_merge_keeps_unknown_generic_arguments_as_union_arms() {
        let generic_types = vec![
            Type::generic(Sym::new("Box"), [Type::String]),
            Type::generic(Sym::new("Box"), [Type::Untyped]),
        ];
        let merged = Type::merge_param_arg_vec(generic_types.clone());

        assert_eq!(merged, Type::from_type_vec_preserve_untyped(generic_types));
        assert!(matches!(merged, Type::Union(parts) if parts.len() == 2));
    }

    #[test]
    fn parameter_merge_keeps_same_length_tuple_compaction_without_unknowns() {
        let tuples = vec![
            Type::Tuple(vec![Type::Integer, Type::String]),
            Type::Tuple(vec![Type::String, Type::Integer]),
        ];
        let expected = Type::Tuple(vec![
            Type::Union(vec![Type::Integer, Type::String]),
            Type::Union(vec![Type::Integer, Type::String]),
        ]);

        assert_eq!(Type::merge_param_arg_vec(tuples.clone()), expected);
        assert!(matches!(Type::from_type_vec(tuples), Type::Union(_)));
    }

    #[test]
    fn type_slot_stays_within_32_bytes() {
        assert!(
            std::mem::size_of::<Type>() <= 32,
            "size_of::<Type>() = {}",
            std::mem::size_of::<Type>()
        );
    }
}
