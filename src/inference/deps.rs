use super::*;
use crate::dep_graph::{DepEdge, DepEdgeKind};

impl<'a> InferenceEngine<'a> {
    /// Scan all types in the registry to build the complete set of
    /// referenced symbols. Call after parameter resolution and receiver preload.
    pub fn finalize_deps(&mut self) {
        let mut types_to_scan: Vec<Type> = Vec::new();
        let mut direct_refs: Vec<String> = Vec::new();

        for cn in &self.registry.class_names() {
            if let Some(data) = self.registry.class_data_for(cn) {
                if let Some(ref sc) = data.superclass {
                    direct_refs.push(sc.to_string());
                    self.file_deps.edges.push(DepEdge {
                        symbol: sc.to_string(),
                        kind: DepEdgeKind::Superclass,
                    });
                }
                for mixin in &data.mixins {
                    direct_refs.push(mixin.module_name.to_string());
                    self.file_deps.edges.push(DepEdge {
                        symbol: mixin.module_name.to_string(),
                        kind: DepEdgeKind::Mixin,
                    });
                }
                for method in &data.methods {
                    types_to_scan.push(method.raw_return_type.clone());
                }
                for ivar_types in data.ivars.values() {
                    for ty in ivar_types {
                        types_to_scan.push(ty.clone());
                        Self::collect_typed_edges_from_type(
                            ty,
                            &self.file_deps.defined_symbols,
                            DepEdgeKind::IvarFlow,
                            &mut self.file_deps.edges,
                        );
                    }
                }
                for cs in &data.call_sites {
                    types_to_scan.extend(cs.arg_types.iter().cloned());
                    types_to_scan.extend(cs.keyword_arg_types.values().cloned());
                }
            }
        }

        for r in direct_refs {
            self.record_reference(&r);
        }
        for ty in &types_to_scan {
            Self::collect_type_references_into(
                ty,
                &self.file_deps.defined_symbols,
                &mut self.file_deps.referenced_symbols,
            );
        }

        Self::collect_method_call_edges(
            &self.registry,
            &self.file_deps.defined_symbols,
            &mut self.file_deps.edges,
        );
        Self::collect_constant_edges(
            &self.registry,
            &self.file_deps.defined_symbols,
            &mut self.file_deps.edges,
        );
    }

    fn collect_typed_edges_from_type(
        ty: &Type,
        defined: &std::collections::HashSet<String>,
        kind: DepEdgeKind,
        edges: &mut Vec<DepEdge>,
    ) {
        match ty {
            Type::Class(name) => {
                let base = if let Some(pos) = name.find('[') {
                    &name[..pos]
                } else {
                    name.as_str()
                };
                if !defined.contains(base) {
                    edges.push(DepEdge {
                        symbol: base.to_string(),
                        kind,
                    });
                }
            }
            Type::Generic { base, args } => {
                if !defined.contains(base.as_str()) {
                    edges.push(DepEdge {
                        symbol: base.as_str().to_string(),
                        kind,
                    });
                }
                for arg in args {
                    Self::collect_typed_edges_from_type(arg, defined, kind, edges);
                }
            }
            Type::Array(Some(inner)) => {
                Self::collect_typed_edges_from_type(inner, defined, kind, edges);
            }
            Type::Hash(Some(k), Some(v)) => {
                Self::collect_typed_edges_from_type(k, defined, kind, edges);
                Self::collect_typed_edges_from_type(v, defined, kind, edges);
            }
            Type::Union(parts) | Type::Intersection(parts) => {
                for p in parts {
                    Self::collect_typed_edges_from_type(p, defined, kind, edges);
                }
            }
            _ => {}
        }
    }

    fn collect_method_call_edges(
        registry: &crate::registry::TypeRegistry,
        defined: &std::collections::HashSet<String>,
        edges: &mut Vec<DepEdge>,
    ) {
        for (_, data) in registry.iter_class_data() {
            for method in &data.methods {
                Self::collect_typed_edges_from_type(
                    &method.raw_return_type,
                    defined,
                    DepEdgeKind::MethodCall,
                    edges,
                );
            }
        }
    }

    fn collect_constant_edges(
        registry: &crate::registry::TypeRegistry,
        defined: &std::collections::HashSet<String>,
        edges: &mut Vec<DepEdge>,
    ) {
        for (_, data) in registry.iter_class_data() {
            for const_def in data.constants.values() {
                Self::collect_typed_edges_from_type(
                    &const_def.const_type,
                    defined,
                    DepEdgeKind::ConstantLookup,
                    edges,
                );
            }
        }
    }

    fn collect_type_references_into(
        ty: &Type,
        defined: &std::collections::HashSet<String>,
        refs: &mut std::collections::HashSet<String>,
    ) {
        match ty {
            Type::Class(name) => {
                let base = if let Some(pos) = name.find('[') {
                    &name[..pos]
                } else {
                    name.as_str()
                };
                if !defined.contains(base) {
                    refs.insert(base.to_string());
                }
            }
            Type::Generic { base, args } => {
                if !defined.contains(base.as_str()) {
                    refs.insert(base.as_str().to_string());
                }
                for arg in args {
                    Self::collect_type_references_into(arg, defined, refs);
                }
            }
            Type::Array(Some(inner)) => Self::collect_type_references_into(inner, defined, refs),
            Type::Hash(Some(k), Some(v)) => {
                Self::collect_type_references_into(k, defined, refs);
                Self::collect_type_references_into(v, defined, refs);
            }
            Type::Union(parts) | Type::Intersection(parts) => {
                for p in parts {
                    Self::collect_type_references_into(p, defined, refs);
                }
            }
            Type::Tuple(elems) => {
                for e in elems {
                    Self::collect_type_references_into(e, defined, refs);
                }
            }
            Type::Record(fields) => {
                for field in fields {
                    Self::collect_type_references_into(&field.value, defined, refs);
                }
            }
            Type::Proc {
                return_type,
                param_count: _,
            } => {
                Self::collect_type_references_into(return_type, defined, refs);
            }
            Type::ReceiverMethodRef(recv, _) => {
                Self::collect_type_references_into(recv, defined, refs);
            }
            Type::MethodReturnRef(class, _) => {
                if !defined.contains(class.as_str()) {
                    refs.insert((class.clone()).to_string());
                }
            }
            _ => {}
        }
    }
}
