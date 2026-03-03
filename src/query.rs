use crate::analysis::HoverResult;
use crate::inference::FileAnalysisSnapshot;
use crate::rbs::stdlib_loader::LazyRbsLoader;
use crate::registry::{ConstantCompletionCandidate, MethodCompletionCandidate, TypeRegistry};
use crate::types::Type;

/// Entry point for semantic queries over cached analysis + workspace type data.
pub struct TypeQueryEngine<'a> {
    analysis: &'a FileAnalysisSnapshot,
    source: &'a str,
    stdlib_loader: &'a LazyRbsLoader,
    external_registry: Option<&'a TypeRegistry>,
}

#[derive(Debug, Clone)]
pub struct MethodCompletionQueryResult {
    pub receiver_type: Type,
    pub candidates: Vec<MethodCompletionCandidate>,
}

impl<'a> TypeQueryEngine<'a> {
    pub fn new(
        analysis: &'a FileAnalysisSnapshot,
        source: &'a str,
        stdlib_loader: &'a LazyRbsLoader,
    ) -> Self {
        Self {
            analysis,
            source,
            stdlib_loader,
            external_registry: None,
        }
    }

    pub fn with_external_registry(mut self, registry: Option<&'a TypeRegistry>) -> Self {
        self.external_registry = registry;
        self
    }

    pub fn hover_at(&self, byte_offset: usize) -> Option<HoverResult> {
        self.analysis.hover_at(
            self.source,
            byte_offset,
            self.stdlib_loader,
            self.external_registry,
        )
    }

    pub fn method_completion_at_receiver(
        &self,
        receiver_offset: usize,
    ) -> Option<MethodCompletionQueryResult> {
        let hover = self.hover_at(receiver_offset)?;
        Some(self.method_completion_for_receiver_type(hover.ty))
    }

    pub fn method_completion_for_receiver_type(
        &self,
        receiver_type: Type,
    ) -> MethodCompletionQueryResult {
        let mut registry = self.file_projection_registry();
        ensure_completion_classes_available(
            &mut registry,
            self.stdlib_loader,
            self.external_registry,
            &receiver_type,
        );
        registry.apply_global_resolution();
        let candidates = registry.method_completion_candidates_for_type(&receiver_type);
        MethodCompletionQueryResult {
            receiver_type,
            candidates,
        }
    }

    pub fn constant_completion_candidates(
        &self,
        namespace: &str,
        class_context: &str,
        prefix: &str,
    ) -> Vec<ConstantCompletionCandidate> {
        let mut registry = self.registry_for_projection();
        registry.apply_global_resolution();
        registry
            .constant_completion_candidates_for_namespace(namespace, class_context)
            .into_iter()
            .filter(|candidate| candidate.name.starts_with(prefix))
            .collect()
    }

    pub fn registry_for_projection(&self) -> TypeRegistry {
        let mut registry = self
            .external_registry
            .cloned()
            .unwrap_or_else(TypeRegistry::new_pooled);
        self.analysis.apply_to_registry(&mut registry);
        registry
    }

    fn file_projection_registry(&self) -> TypeRegistry {
        let mut registry = TypeRegistry::new_pooled();
        self.analysis.apply_to_registry(&mut registry);
        registry
    }
}

fn ensure_completion_classes_available(
    registry: &mut TypeRegistry,
    loader: &LazyRbsLoader,
    external_registry: Option<&TypeRegistry>,
    receiver_type: &Type,
) {
    let mut pending = Vec::from([
        "Object".to_string(),
        "Kernel".to_string(),
        "BasicObject".to_string(),
        "Class".to_string(),
        "Module".to_string(),
        "Comparable".to_string(),
    ]);
    collect_completion_class_names(receiver_type, &mut pending);

    let mut seen = std::collections::HashSet::new();
    while let Some(class_name) = pending.pop() {
        if !seen.insert(class_name.clone()) {
            continue;
        }
        if let Some(external_registry) = external_registry {
            registry.merge_rbs_class_from(external_registry, &class_name);
        }
        loader.merge_class_into(&class_name, registry);
        let Some(data) = registry.class_data_for(&class_name) else {
            continue;
        };
        let related: Vec<String> = data
            .superclass
            .iter()
            .map(|name| name.as_ref().to_string())
            .chain(
                data.mixins
                    .iter()
                    .map(|mixin| mixin.module_name.as_ref().to_string()),
            )
            .chain(
                data.cold()
                    .required_ancestors
                    .iter()
                    .map(|name| name.as_ref().to_string()),
            )
            .collect();
        pending.extend(related);
    }
}

fn collect_completion_class_names(ty: &Type, names: &mut Vec<String>) {
    match ty {
        Type::Union(parts) | Type::Intersection(parts) => {
            for part in parts {
                collect_completion_class_names(part, names);
            }
        }
        _ => {
            if let Some(class_name) = TypeRegistry::type_to_class_name_pub(ty) {
                names.push(class_name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::analysis::{AnalysisOptions, analyze_cached_file_with_deps};
    use crate::rbs::import::load_rbs_string;

    fn loader() -> LazyRbsLoader {
        let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        LazyRbsLoader::new(core_dir)
    }

    #[test]
    fn hover_queries_cached_file_snapshot() {
        let source = r#"
class Box
  def value
    1
  end
end

box = Box.new
box.value
"#;
        let loader = loader();
        let (analysis, _) = analyze_cached_file_with_deps(
            source,
            None,
            Some(&loader),
            Some("query.rb"),
            AnalysisOptions::default(),
        );
        let offset = source.rfind("value").expect("call site");

        let hover = TypeQueryEngine::new(&analysis, source, &loader)
            .hover_at(offset)
            .expect("hover result");

        assert_eq!(hover.name, "value");
        assert_eq!(hover.ty.to_string(), "1");
    }

    #[test]
    fn method_completion_queries_receiver_type() {
        let source = r#"
class Box
  def value
    1
  end
end

box = Box.new
box
"#;
        let loader = loader();
        let (analysis, _) = analyze_cached_file_with_deps(
            source,
            None,
            Some(&loader),
            Some("query.rb"),
            AnalysisOptions::default(),
        );
        let offset = source.rfind("box").expect("receiver");

        let completion = TypeQueryEngine::new(&analysis, source, &loader)
            .method_completion_at_receiver(offset)
            .expect("method completion");

        assert_eq!(completion.receiver_type.to_string(), "Box");
        assert!(
            completion
                .candidates
                .iter()
                .any(|candidate| candidate.name == "value")
        );
    }

    #[test]
    fn method_completion_uses_external_classes_without_full_projection_clone() {
        let source = "client = ApiClient.new\nclient\n";
        let loader = loader();
        let mut external = TypeRegistry::new();
        load_rbs_string(
            r#"
class ApiClient
  def ping: () -> String
end
"#,
            &mut external,
        );
        let (analysis, _) = analyze_cached_file_with_deps(
            source,
            Some(&external),
            Some(&loader),
            Some("query_external.rb"),
            AnalysisOptions::default(),
        );
        let offset = source.rfind("client").expect("receiver");

        let completion = TypeQueryEngine::new(&analysis, source, &loader)
            .with_external_registry(Some(&external))
            .method_completion_at_receiver(offset)
            .expect("method completion");

        assert_eq!(completion.receiver_type.to_string(), "ApiClient");
        assert!(
            completion
                .candidates
                .iter()
                .any(|candidate| candidate.name == "ping")
        );
    }

    #[test]
    fn constant_completion_queries_projected_registry() {
        let source = r#"
module Outer
  class Inner
  end

  VALUE = 1
end
"#;
        let loader = loader();
        let (analysis, _) = analyze_cached_file_with_deps(
            source,
            None,
            Some(&loader),
            Some("query.rb"),
            AnalysisOptions::default(),
        );

        let candidates = TypeQueryEngine::new(&analysis, source, &loader)
            .constant_completion_candidates("Outer", "", "In");

        assert!(candidates.iter().any(|candidate| candidate.name == "Inner"));
        assert!(!candidates.iter().any(|candidate| candidate.name == "VALUE"));
    }
}
