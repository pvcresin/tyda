//! Splat conversion during inference: `to_a` lookups for `Type::splat_concat`.

use super::InferenceEngine;
use crate::types::{SplatElements, SplatToA, Type};

impl InferenceEngine<'_> {
    pub(super) fn splat_to_a(&mut self, ty: &Type) -> SplatToA {
        match self.type_responds_to_method(ty, "to_a") {
            Some(false) => {
                let surface_complete = self
                    .type_to_class_name(ty)
                    .is_some_and(|class_name| self.member_surface_provably_complete(&class_name));
                if surface_complete {
                    SplatToA::Missing
                } else {
                    SplatToA::Unknown
                }
            }
            Some(true) => match self.splat_to_a_return(ty) {
                Some(Type::Untyped) | None => SplatToA::Unknown,
                Some(array) => SplatToA::Returns(array),
            },
            None => SplatToA::Unknown,
        }
    }

    /// A source-defined `to_a` reads its inferred return; a declared one binds the receiver's type arguments.
    fn splat_to_a_return(&mut self, ty: &Type) -> Option<Type> {
        let class_name = self.type_to_class_name(ty)?;
        let is_singleton = matches!(ty, Type::Singleton(_));
        let source_defined = self
            .registry
            .resolve_method_call_owners(&class_name, "to_a", is_singleton)
            .first()
            .is_some_and(|(owner, _)| self.registry.is_user_defined_class(owner));
        if source_defined {
            return Some(self.resolve_symbol_proc_return_type(ty, "to_a"));
        }
        self.resolve_rbs_no_arg_method_return_on_type(ty, "to_a", false)
            .or_else(|| Some(self.resolve_symbol_proc_return_type(ty, "to_a")))
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
