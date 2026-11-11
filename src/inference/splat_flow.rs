//! Splat conversion during inference: `to_a` lookups for `Type::splat_concat`.

use super::InferenceEngine;
use crate::registry::ToAResponse;
use crate::types::{SplatElements, SplatToA, Type};

impl InferenceEngine<'_> {
    /// Loads the value's class and ancestors, then decides as deferred resolution does
    /// (`TypeRegistry::splat_to_a_response`); only the return lookup binds type arguments here.
    pub(super) fn splat_to_a(&mut self, ty: &Type) -> SplatToA {
        if let Some(class_name) = self.type_to_class_name(ty) {
            self.ensure_class_available_with_ancestors(&class_name, 0);
        }
        match self.registry.splat_to_a_response(ty) {
            ToAResponse::Missing => SplatToA::Missing,
            ToAResponse::Unknown => SplatToA::Unknown,
            ToAResponse::Defined { source_defined, .. } => {
                let ret = if source_defined {
                    Some(self.resolve_symbol_proc_return_type(ty, "to_a"))
                } else {
                    self.resolve_rbs_no_arg_method_return_on_type(ty, "to_a", false)
                        .or_else(|| Some(self.resolve_symbol_proc_return_type(ty, "to_a")))
                };
                match ret {
                    Some(Type::Untyped) | None => SplatToA::Unknown,
                    Some(array) => SplatToA::Returns(array),
                }
            }
        }
    }

    /// `[*parts...]`, deferred while a part is known only later.
    pub(super) fn splat_array(&mut self, parts: Vec<Type>) -> Type {
        Type::splat_array(parts, &mut |ty| self.splat_to_a(ty))
    }

    /// The elements `*ty` gives; a value known only later counts as one element of itself.
    pub(super) fn splat_elements(&mut self, ty: &Type) -> SplatElements {
        Type::splat_elements(ty, &mut |ty| self.splat_to_a(ty))
            .unwrap_or_else(|| SplatElements::Exact(vec![ty.clone()]))
    }

    pub(super) fn resolve_splat_parts(&mut self, parts: &Type) -> Type {
        Type::resolve_splat_ref(parts, &mut |ty| self.splat_to_a(ty))
            .unwrap_or_else(|| Type::SplatRef(Box::new(parts.clone())))
    }
}
