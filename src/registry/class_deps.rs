//! The symbols a class's declarations and types name, for the file dependency graph.

use super::{ClassData, TypeRegistry};
use crate::dep_graph::DepEdgeKind;
use crate::types::Type;
use std::sync::Arc;

/// What one class names. A file keeps the `references` and `edges` whose symbol it does not
/// define, and every `inheritance` edge.
#[derive(Debug, Default)]
pub(crate) struct ClassDepScan {
    pub(crate) references: Vec<Box<str>>,
    pub(crate) edges: Vec<(Box<str>, DepEdgeKind)>,
    pub(crate) inheritance: Vec<(Box<str>, DepEdgeKind)>,
}

impl ClassDepScan {
    fn of(data: &ClassData) -> Self {
        let mut references = Vec::new();
        let mut edges = Vec::new();
        let mut inheritance = Vec::new();
        if let Some(superclass) = &data.superclass {
            references.push(superclass.as_ref());
            inheritance.push((superclass.as_ref(), DepEdgeKind::Superclass));
        }
        for mixin in &data.mixins {
            references.push(mixin.module_name.as_ref());
            inheritance.push((mixin.module_name.as_ref(), DepEdgeKind::Mixin));
        }
        for method in &data.methods {
            type_references(&method.raw_return_type, &mut references);
            typed_edges(&method.raw_return_type, DepEdgeKind::MethodCall, &mut edges);
        }
        for ivar_types in data.ivars.values() {
            for ty in ivar_types {
                type_references(ty, &mut references);
                typed_edges(ty, DepEdgeKind::IvarFlow, &mut edges);
            }
        }
        for site in &data.call_sites {
            for ty in site.arg_types.iter().chain(site.keyword_arg_types.values()) {
                type_references(ty, &mut references);
            }
        }
        for constant in data.constants.values() {
            typed_edges(
                &constant.const_type,
                DepEdgeKind::ConstantLookup,
                &mut edges,
            );
        }
        references.sort_unstable();
        references.dedup();
        edges.sort_unstable();
        edges.dedup();
        inheritance.sort_unstable();
        inheritance.dedup();
        Self {
            references: references.into_iter().map(Box::from).collect(),
            edges: edges
                .into_iter()
                .map(|(symbol, kind)| (Box::from(symbol), kind))
                .collect(),
            inheritance: inheritance
                .into_iter()
                .map(|(symbol, kind)| (Box::from(symbol), kind))
                .collect(),
        }
    }
}

impl TypeRegistry {
    /// Calls `f` with what each class names. A class shared with another registry keeps its
    /// scan, since a write copies it out of sharing and drops the scan.
    pub(crate) fn for_each_class_dep_scan(&self, mut f: impl FnMut(&ClassDepScan)) {
        for data in self.class_data.values() {
            if Arc::strong_count(data) > 1 {
                f(data
                    .derived()
                    .dep_scan
                    .get_or_init(|| ClassDepScan::of(data)));
            } else {
                f(&ClassDepScan::of(data));
            }
        }
    }
}

fn base_name(name: &str) -> &str {
    name.find('[').map_or(name, |pos| &name[..pos])
}

fn type_references<'a>(ty: &'a Type, out: &mut Vec<&'a str>) {
    match ty {
        Type::Class(name) => out.push(base_name(name)),
        Type::Generic { base, args } => {
            out.push(base.as_str());
            for arg in args {
                type_references(arg, out);
            }
        }
        Type::Array(Some(inner)) => type_references(inner, out),
        Type::Hash(Some(k), Some(v)) => {
            type_references(k, out);
            type_references(v, out);
        }
        Type::Union(parts) | Type::Intersection(parts) => {
            for part in parts {
                type_references(part, out);
            }
        }
        Type::Tuple(elems) => {
            for elem in elems {
                type_references(elem, out);
            }
        }
        Type::Record(fields) => {
            for field in fields {
                type_references(&field.value, out);
            }
        }
        Type::Proc(proc_type) => type_references(&proc_type.return_type, out),
        Type::ReceiverMethodRef(recv, _) => type_references(recv, out),
        Type::MethodReturnRef(class, _) => out.push(class.as_str()),
        Type::DeferredSafeNavigation(receiver, result) => {
            type_references(receiver, out);
            type_references(result, out);
        }
        _ => {}
    }
}

fn typed_edges<'a>(ty: &'a Type, kind: DepEdgeKind, out: &mut Vec<(&'a str, DepEdgeKind)>) {
    match ty {
        Type::Class(name) => out.push((base_name(name), kind)),
        Type::Generic { base, args } => {
            out.push((base.as_str(), kind));
            for arg in args {
                typed_edges(arg, kind, out);
            }
        }
        Type::Array(Some(inner)) => typed_edges(inner, kind, out),
        Type::Hash(Some(k), Some(v)) => {
            typed_edges(k, kind, out);
            typed_edges(v, kind, out);
        }
        Type::Union(parts) | Type::Intersection(parts) => {
            for part in parts {
                typed_edges(part, kind, out);
            }
        }
        Type::DeferredSafeNavigation(receiver, result) => {
            typed_edges(receiver, kind, out);
            typed_edges(result, kind, out);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanned_references(registry: &TypeRegistry) -> Vec<String> {
        let mut names = Vec::new();
        registry.for_each_class_dep_scan(|scan| {
            names.extend(scan.references.iter().map(|name| name.to_string()));
        });
        names.sort();
        names
    }

    #[test]
    fn a_write_drops_the_kept_scan_of_a_shared_class() {
        let mut registry = TypeRegistry::new();
        registry.class_data_mut("A").superclass = Some(Arc::from("Base"));
        let shared = registry.clone();
        assert_eq!(scanned_references(&registry), ["Base"]);

        registry.class_data_mut("A").superclass = Some(Arc::from("Other"));
        assert_eq!(scanned_references(&registry), ["Other"]);
        assert_eq!(scanned_references(&shared), ["Base"]);

        // Once the other copy is gone, the class is written in place.
        let mut kept = shared.clone();
        drop(shared);
        assert_eq!(scanned_references(&kept), ["Base"]);
        kept.class_data_mut("A").superclass = Some(Arc::from("Last"));
        let again = kept.clone();
        assert_eq!(scanned_references(&again), ["Last"]);
    }
}
