//! Ruby's splat conversion (`[*value]`, `Array(value)`, `call(*value)`).
//!
//! A splatted value gives its elements through `to_a`: an Array gives its own elements, `nil`
//! gives none, and a value that does not respond to `to_a` gives itself as the only element.
//! Callers supply the `to_a` lookup, so the same rule serves inference and deferred resolution.

use super::Type;

/// What `value.to_a` is known to give.
#[derive(Debug, Clone, PartialEq)]
pub enum SplatToA {
    /// The value does not respond to `to_a`.
    Missing,
    Returns(Type),
    Unknown,
}

/// The elements a splat contributes.
#[derive(Debug, Clone, PartialEq)]
pub enum SplatElements {
    Exact(Vec<Type>),
    /// Any number of elements of this type.
    Open(Type),
}

impl SplatElements {
    pub fn element_type(&self) -> Type {
        match self {
            SplatElements::Exact(elems) => Type::from_type_vec_preserve_untyped(elems.clone()),
            SplatElements::Open(elem) => elem.clone(),
        }
    }
}

impl Type {
    /// The elements `*ty` gives, or `None` while `ty` is a reference known only later.
    pub fn splat_elements(
        ty: &Type,
        to_a: &mut dyn FnMut(&Type) -> SplatToA,
    ) -> Option<SplatElements> {
        match ty {
            Type::Union(parts) => Self::union_splat_elements(parts, to_a),
            _ => match Self::structural_splat_elements(ty)? {
                Some(elements) => Some(elements),
                None => match to_a(ty) {
                    SplatToA::Missing => Some(Self::splat_of_value_without_to_a(ty)),
                    SplatToA::Unknown => Some(SplatElements::Open(Type::Untyped)),
                    SplatToA::Returns(array) => match Self::structural_splat_elements(&array)? {
                        Some(elements) => Some(elements),
                        None => Some(SplatElements::Open(Type::Untyped)),
                    },
                },
            },
        }
    }

    /// The value is the only element. A nominal type may stand for a subclass that adds `to_a`,
    /// so only a value fixed by its type keeps the exact count.
    fn splat_of_value_without_to_a(ty: &Type) -> SplatElements {
        match ty {
            Type::LiteralInteger(_)
            | Type::LiteralFloat(_)
            | Type::LiteralString(_)
            | Type::LiteralSymbol(_)
            | Type::True
            | Type::False
            | Type::Singleton(_) => SplatElements::Exact(vec![ty.clone()]),
            _ => SplatElements::Open(ty.clone()),
        }
    }

    /// The Array `[*parts[0], *parts[1], ...]` builds, or `None` while a part is known only later.
    pub fn splat_concat(parts: &[Type], to_a: &mut dyn FnMut(&Type) -> SplatToA) -> Option<Type> {
        let mut exact: Vec<Type> = Vec::new();
        let mut open: Option<Vec<Type>> = None;
        for part in parts {
            match Self::splat_elements(part, to_a)? {
                SplatElements::Exact(elems) => match open.as_mut() {
                    Some(members) => members.extend(elems),
                    None => exact.extend(elems),
                },
                SplatElements::Open(elem) => {
                    open.get_or_insert_with(|| std::mem::take(&mut exact))
                        .push(elem);
                }
            }
        }
        Some(match open {
            Some(members) => Type::Array(Some(Box::new(Type::from_type_vec_preserve_untyped(
                members,
            )))),
            None => Type::Tuple(exact),
        })
    }

    /// `[*parts...]` now when every part is known. Otherwise the shape is open and only the
    /// element type waits: `Array[SplatRef]`, so the value still acts as an Array meanwhile.
    pub fn splat_array(parts: Vec<Type>, to_a: &mut dyn FnMut(&Type) -> SplatToA) -> Type {
        Self::splat_concat(&parts, to_a).unwrap_or_else(|| {
            Type::Array(Some(Box::new(Type::SplatRef(Box::new(Type::Tuple(parts))))))
        })
    }

    /// The element type of `[*parts...]` (a `SplatRef`'s value); `None` keeps it deferred.
    pub fn resolve_splat_ref(
        inner: &Type,
        to_a: &mut dyn FnMut(&Type) -> SplatToA,
    ) -> Option<Type> {
        let parts = match inner {
            Type::Tuple(parts) => parts.as_slice(),
            _ => std::slice::from_ref(inner),
        };
        let mut members = Vec::new();
        for part in parts {
            match Self::splat_elements(part, to_a)? {
                SplatElements::Exact(elems) => members.extend(elems),
                SplatElements::Open(elem) => members.push(elem),
            }
        }
        Some(Self::from_type_vec_preserve_untyped(members))
    }

    /// A `SplatRef` that can no longer be resolved: its unknown parts give unknown elements.
    pub fn splat_fallback(inner: &Type) -> Type {
        let erased = match inner {
            Type::Tuple(parts) => {
                Type::Tuple(parts.iter().map(Self::erase_deferred_splat_part).collect())
            }
            other => Self::erase_deferred_splat_part(other),
        };
        Self::resolve_splat_ref(&erased, &mut |_| SplatToA::Unknown).unwrap_or(Type::Untyped)
    }

    fn erase_deferred_splat_part(part: &Type) -> Type {
        match part {
            Type::Union(members) => Type::from_type_vec_preserve_untyped(
                members
                    .iter()
                    .map(Self::erase_deferred_splat_part)
                    .collect(),
            ),
            _ if Self::structural_splat_elements(part).is_none() => Type::Untyped,
            _ => part.clone(),
        }
    }

    /// The outer `None` means deferred; the inner `None` means the value needs a `to_a` lookup.
    fn structural_splat_elements(ty: &Type) -> Option<Option<SplatElements>> {
        Some(Some(match ty {
            Type::Tuple(elems) => SplatElements::Exact(elems.clone()),
            Type::Array(elem) => {
                SplatElements::Open(elem.as_deref().cloned().unwrap_or(Type::Untyped))
            }
            Type::Hash(key, value) => SplatElements::Open(Type::Tuple(vec![
                key.as_deref().cloned().unwrap_or(Type::Untyped),
                value.as_deref().cloned().unwrap_or(Type::Untyped),
            ])),
            Type::Nil | Type::Bot => SplatElements::Exact(Vec::new()),
            Type::Untyped | Type::Top | Type::Void | Type::Todo => {
                SplatElements::Open(Type::Untyped)
            }
            Type::ParamRef(_)
            | Type::KeywordParamRef(_)
            | Type::IvarRef(_)
            | Type::GlobalVariableRef(_)
            | Type::MethodReturnRef(..)
            | Type::ReceiverMethodRef(..)
            | Type::DeferredSafeNavigation(..)
            | Type::BlockReturnRef
            | Type::PatternIndexRef(..)
            | Type::PatternRestRef(_)
            | Type::PatternTrailingRef(..)
            | Type::PatternKeyRef(..)
            | Type::PatternKeyRestRef(..)
            | Type::TruthyProjection(_)
            | Type::FalsyProjection(_)
            | Type::NonNilProjection(_)
            | Type::SplatRef(_)
            | Type::SelfType
            | Type::InstanceType => return None,
            _ => return Some(None),
        }))
    }

    fn union_splat_elements(
        parts: &[Type],
        to_a: &mut dyn FnMut(&Type) -> SplatToA,
    ) -> Option<SplatElements> {
        let mut expanded = Vec::with_capacity(parts.len());
        for part in parts {
            expanded.push(Self::splat_elements(part, to_a)?);
        }
        let exact_len = match expanded.first() {
            Some(SplatElements::Exact(first)) => Some(first.len()),
            _ => None,
        };
        if let Some(len) = exact_len
            && expanded.iter().all(
                |elements| matches!(elements, SplatElements::Exact(elems) if elems.len() == len),
            )
        {
            let slots = (0..len)
                .map(|index| {
                    Type::from_type_vec_preserve_untyped(
                        expanded
                            .iter()
                            .filter_map(|elements| match elements {
                                SplatElements::Exact(elems) => elems.get(index).cloned(),
                                SplatElements::Open(_) => None,
                            })
                            .collect(),
                    )
                })
                .collect();
            return Some(SplatElements::Exact(slots));
        }
        let mut members = Vec::new();
        for elements in expanded {
            match elements {
                SplatElements::Exact(elems) => members.extend(elems),
                SplatElements::Open(elem) => members.push(elem),
            }
        }
        Some(SplatElements::Open(Type::from_type_vec_preserve_untyped(
            members,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Sym;

    fn no_to_a(_: &Type) -> SplatToA {
        SplatToA::Missing
    }

    #[test]
    fn splat_of_tuple_keeps_exact_shape() {
        let parts = [
            Type::Tuple(vec![Type::LiteralInteger(0)]),
            Type::Tuple(vec![Type::LiteralInteger(1), Type::String]),
        ];
        assert_eq!(
            Type::splat_concat(&parts, &mut no_to_a),
            Some(Type::Tuple(vec![
                Type::LiteralInteger(0),
                Type::LiteralInteger(1),
                Type::String
            ]))
        );
    }

    #[test]
    fn splat_of_array_opens_the_element_count() {
        let parts = [
            Type::Tuple(vec![Type::LiteralInteger(0)]),
            Type::Array(Some(Box::new(Type::Integer))),
        ];
        assert_eq!(
            Type::splat_concat(&parts, &mut no_to_a),
            Some(Type::Array(Some(Box::new(Type::Integer))))
        );
    }

    #[test]
    fn splat_of_nil_gives_no_elements() {
        let optional = Type::Union(vec![Type::Array(Some(Box::new(Type::Integer))), Type::Nil]);
        assert_eq!(
            Type::splat_concat(&[optional], &mut no_to_a),
            Some(Type::Array(Some(Box::new(Type::Integer))))
        );
        assert_eq!(
            Type::splat_concat(&[Type::Nil], &mut no_to_a),
            Some(Type::Tuple(Vec::new()))
        );
    }

    #[test]
    fn splat_of_value_without_to_a_wraps_it() {
        let item = Type::Class(Sym::new("Item"));
        assert_eq!(
            Type::splat_concat(std::slice::from_ref(&item), &mut no_to_a),
            Some(Type::Array(Some(Box::new(item))))
        );
        let one = Type::LiteralInteger(1);
        assert_eq!(
            Type::splat_concat(std::slice::from_ref(&one), &mut no_to_a),
            Some(Type::Tuple(vec![one]))
        );
    }

    #[test]
    fn splat_uses_the_to_a_result() {
        let pair = Type::Class(Sym::new("Pair"));
        let tuple = Type::Tuple(vec![Type::LiteralInteger(1), Type::String]);
        let mut to_a = |_: &Type| SplatToA::Returns(tuple.clone());
        assert_eq!(Type::splat_concat(&[pair], &mut to_a), Some(tuple));
    }

    #[test]
    fn splat_of_unknown_to_a_is_open_untyped() {
        let mut to_a = |_: &Type| SplatToA::Unknown;
        assert_eq!(
            Type::splat_concat(&[Type::Class(Sym::new("Pair"))], &mut to_a),
            Some(Type::Array(Some(Box::new(Type::Untyped))))
        );
    }

    #[test]
    fn unresolved_splat_element_falls_back_to_untyped_parts() {
        let inner = Type::Tuple(vec![Type::Tuple(vec![Type::Integer]), Type::ParamRef(0)]);
        assert_eq!(
            Type::splat_fallback(&inner),
            Type::Union(vec![Type::Integer, Type::Untyped])
        );
    }

    #[test]
    fn splat_of_reference_defers() {
        let parts = vec![Type::Tuple(vec![Type::Integer]), Type::ParamRef(0)];
        assert_eq!(Type::splat_concat(&parts, &mut no_to_a), None);
        let element = Type::SplatRef(Box::new(Type::Tuple(parts.clone())));
        assert_eq!(
            Type::splat_array(parts, &mut no_to_a),
            Type::Array(Some(Box::new(element.clone())))
        );
        let resolved = Type::Tuple(vec![
            Type::Tuple(vec![Type::Integer]),
            Type::Array(Some(Box::new(Type::String))),
        ]);
        assert_eq!(
            Type::resolve_splat_ref(&resolved, &mut no_to_a),
            Some(Type::Union(vec![Type::Integer, Type::String]))
        );
    }
}
