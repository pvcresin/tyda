use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use super::fingerprints::{combine_unordered_hashes, hash_u64};
use super::{FileId, WorkspaceFileEntry, WorkspaceState};
use crate::dep_graph::DepEdgeKind;
use crate::registry::TypeRegistry;
use crate::types::Type;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DisplayFileScopeKey {
    file_ids: Vec<FileId>,
    fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DisplayScopeKey {
    file_set: DisplayFileScopeKey,
    target_classes_fingerprint: u64,
}

#[derive(Debug, Clone)]
pub(super) struct DisplayBaseRegistryCache {
    key: DisplayFileScopeKey,
    registry: Arc<TypeRegistry>,
}

impl WorkspaceState {
    const DISPLAY_INCOMING_KINDS: [DepEdgeKind; 3] = [
        DepEdgeKind::MethodCall,
        DepEdgeKind::IvarFlow,
        DepEdgeKind::RBSOverride,
    ];

    fn type_needs_display_workspace(ty: &Type) -> bool {
        match ty {
            Type::ParamRef(_)
            | Type::KeywordParamRef(_)
            | Type::IvarRef(_)
            | Type::MethodReturnRef(..)
            | Type::ReceiverMethodRef(..)
            | Type::BlockReturnRef => true,
            Type::Array(Some(inner))
            | Type::PatternRestRef(inner)
            | Type::Proc {
                return_type: inner,
                param_count: _,
            } => Self::type_needs_display_workspace(inner),
            Type::Hash(Some(key), Some(value)) => {
                Self::type_needs_display_workspace(key) || Self::type_needs_display_workspace(value)
            }
            Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => {
                parts.iter().any(Self::type_needs_display_workspace)
            }
            Type::Record(fields) => fields
                .iter()
                .any(|field| Self::type_needs_display_workspace(&field.value)),
            Type::PatternIndexRef(subject, _)
            | Type::PatternKeyRef(subject, _)
            | Type::PatternKeyRestRef(subject, _) => Self::type_needs_display_workspace(subject),
            _ => false,
        }
    }

    fn analysis_needs_display_workspace(
        &self,
        file_path: &str,
        entry: &WorkspaceFileEntry,
    ) -> bool {
        if self.dep_graph.has_references(file_path) {
            return true;
        }

        entry
            .analysis
            .registry()
            .iter_class_data()
            .any(|(_, data)| {
                data.methods
                    .iter()
                    .any(|method| Self::type_needs_display_workspace(&method.raw_return_type))
                    || data
                        .constants
                        .values()
                        .any(|const_def| Self::type_needs_display_workspace(&const_def.const_type))
                    || data
                        .ivars
                        .values()
                        .flatten()
                        .any(Self::type_needs_display_workspace)
            })
    }

    fn file_is_namespace_only_reopen_for_symbol(&self, file_path: &str, symbol: &str) -> bool {
        if !self.dep_graph.has_definitions(file_path) {
            return false;
        }
        let nested_prefix = format!("{symbol}::");
        let mut saw_definition = false;
        let mut all_nested = true;
        self.dep_graph
            .for_each_definition_symbol(file_path, |defined_symbol| {
                saw_definition = true;
                if defined_symbol == symbol || !defined_symbol.starts_with(&nested_prefix) {
                    all_nested = false;
                }
            });
        saw_definition && all_nested
    }

    fn display_related_dependents(&self, symbols: &HashSet<String>) -> HashSet<FileId> {
        let mut related = HashSet::new();
        self.dep_graph.for_each_dependent_path_by_kinds_strict(
            symbols,
            &Self::DISPLAY_INCOMING_KINDS,
            |file_path| {
                if let Some(file_id) = self.workspace_file_id(file_path) {
                    related.insert(file_id);
                }
            },
        );
        self.dep_graph
            .for_each_dependent_path(symbols, |file_path| {
                let Some(file_id) = self.workspace_file_id(file_path) else {
                    return;
                };
                if related.contains(&file_id) {
                    return;
                }
                let mut should_include = false;
                self.dep_graph
                    .for_each_reference_symbol(file_path, |referenced_symbol| {
                        if should_include {
                            return;
                        }
                        should_include = symbols.contains(referenced_symbol)
                            && !self.file_is_namespace_only_reopen_for_symbol(
                                file_path,
                                referenced_symbol,
                            );
                    });
                if should_include {
                    related.insert(file_id);
                }
            });
        related
    }

    fn has_imprecise_dep_entries_except(&self, current_file: &str) -> bool {
        self.files.iter().any(|(file_id, entry)| {
            self.workspace_file_path(*file_id).is_some_and(|file_path| {
                file_path != current_file
                    && !self.dep_graph.has_definitions(file_path)
                    && !self.dep_graph.has_references(file_path)
                    && entry.analysis.registry().iter_class_data().next().is_some()
            })
        })
    }

    fn display_file_set_key(
        &self,
        exclude_file_id: Option<FileId>,
        related_files: Option<&HashSet<FileId>>,
    ) -> DisplayFileScopeKey {
        let mut file_ids: Vec<FileId> = match related_files {
            Some(related) => related.iter().copied().collect(),
            None => self.files.keys().copied().collect(),
        };
        if let Some(exclude_file_id) = exclude_file_id {
            file_ids.retain(|file_id| *file_id != exclude_file_id);
        }
        file_ids.sort_unstable();
        let fingerprint = hash_u64(&combine_unordered_hashes(file_ids.iter().filter_map(
            |file_id| {
                let entry = self.files.get(file_id)?;
                Some(hash_u64(&(*file_id, entry.registry_fingerprint_hash)))
            },
        )));
        DisplayFileScopeKey {
            file_ids,
            fingerprint,
        }
    }

    pub(crate) fn display_scope_key(&self, exclude_file: &str) -> DisplayScopeKey {
        let exclude_file_id = self.workspace_file_id(exclude_file);
        let target_classes: HashSet<String> = self
            .workspace_file(exclude_file)
            .map(|entry| {
                entry
                    .analysis
                    .registry()
                    .iter_class_data()
                    .map(|(name, _)| name.to_string())
                    .collect()
            })
            .unwrap_or_default();
        let related_files = self.display_related_files(exclude_file);
        DisplayScopeKey {
            file_set: self.display_file_set_key(exclude_file_id, related_files.as_ref()),
            target_classes_fingerprint: Self::target_classes_fingerprint(&target_classes),
        }
    }

    #[cfg(test)]
    pub(crate) fn display_scope_includes_file(
        &self,
        exclude_file: &str,
        candidate_file: &str,
    ) -> bool {
        let Some(candidate_id) = self.workspace_file_id(candidate_file) else {
            return false;
        };
        self.display_scope_key(exclude_file)
            .file_set
            .file_ids
            .contains(&candidate_id)
    }

    fn target_classes_fingerprint(target_classes: &HashSet<String>) -> u64 {
        hash_u64(&combine_unordered_hashes(
            target_classes.iter().map(hash_u64),
        ))
    }

    pub fn workspace_registry_excluding(
        &mut self,
        user_rbs: &TypeRegistry,
        exclude_file: &str,
        _excluding_fingerprint: u64,
    ) -> Arc<TypeRegistry> {
        let display_scope_key = self.display_scope_key(exclude_file);
        self.workspace_registry_excluding_with_key(user_rbs, exclude_file, &display_scope_key)
    }

    pub(crate) fn warm_display_base_registry(&mut self, user_rbs: &TypeRegistry) -> bool {
        let base_key = self.display_file_set_key(None, None);
        if self
            .cached_display_base_registry
            .as_ref()
            .is_some_and(|cached| cached.key == base_key)
        {
            return true;
        }

        let mut registry = TypeRegistry::new_pooled();
        registry.merge_rbs_registry(user_rbs);
        for file_id in &base_key.file_ids {
            if let Some(entry) = self.files.get(file_id) {
                entry.analysis.apply_to_registry(&mut registry);
            }
        }
        registry.build_subclass_index();
        registry.shrink_to_fit_after_compact();
        registry.drop_transient_collection_state();
        let arc = Arc::new(registry);
        self.cached_display_base_registry = Some(DisplayBaseRegistryCache {
            key: base_key,
            registry: arc,
        });
        false
    }

    pub(crate) fn workspace_registry_excluding_with_key(
        &mut self,
        user_rbs: &TypeRegistry,
        exclude_file: &str,
        _display_scope_key: &DisplayScopeKey,
    ) -> Arc<TypeRegistry> {
        let target_classes: HashSet<String> = self
            .workspace_file(exclude_file)
            .map(|entry| {
                entry
                    .analysis
                    .registry()
                    .iter_class_data()
                    .map(|(name, _)| name.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // One workspace-wide base is shared across current files. Per-file
        // exclusion made A→B a full remmerge because the key was "all except A"
        // vs "all except B". Current-file defs still win in the engine; this
        // clone is the OnDemand lookup source plus inbound call sites.
        let t0 = Instant::now();
        let display_base_cache_hit = self.warm_display_base_registry(user_rbs);
        let t_merge = t0.elapsed();
        let base_registry = self
            .cached_display_base_registry
            .as_ref()
            .map(|cached| Arc::clone(&cached.registry))
            .expect("warm_display_base_registry leaves cached_display_base_registry populated");

        let t_clone_start = Instant::now();
        let mut registry = (*base_registry).clone();
        registry.strip_methods_defined_in(exclude_file);
        let t_clone = t_clone_start.elapsed();
        let t1 = Instant::now();
        registry.apply_display_resolution_for_targets(&target_classes);
        let t_resolve = t1.elapsed();

        self.last_timings.registry_build = t_merge + t_clone + t_resolve;
        self.last_timings.propagate = t_resolve;
        #[cfg(test)]
        {
            self.last_timings.display_merge = t_merge;
            self.last_timings.display_clone = t_clone;
            self.last_timings.display_base_cache_hit = display_base_cache_hit;
        }
        #[cfg(not(test))]
        let _ = display_base_cache_hit;

        Arc::new(registry)
    }

    fn display_related_files(&self, _exclude_file: &str) -> Option<HashSet<FileId>> {
        // dep_graph pruning was unsound (insufficient depth, missing graph edges, duck-typed refs), so include all workspace files instead (`None` -> the full file set).
        None
    }

    #[cfg(test)]
    pub(super) fn cached_display_base_registry(&self) -> Option<Arc<TypeRegistry>> {
        self.cached_display_base_registry
            .as_ref()
            .map(|cached| Arc::clone(&cached.registry))
    }

    #[cfg(test)]
    pub fn has_cached_registry(&self) -> bool {
        self.cached_registry.is_some()
    }

    #[cfg(test)]
    pub fn clear_display_base_registry_cache(&mut self) {
        self.cached_display_base_registry = None;
    }

    pub fn display_can_skip_workspace_context(&self, file_path: &str) -> bool {
        let Some(entry) = self.workspace_file(file_path) else {
            return false;
        };
        if self.analysis_needs_display_workspace(file_path, entry) {
            return false;
        }

        if !self.dep_graph.has_definitions(file_path) {
            return entry.analysis.registry().iter_class_data().next().is_none();
        }
        if self.has_imprecise_dep_entries_except(file_path) {
            return false;
        }

        let mut defined_symbols = HashSet::new();
        self.dep_graph
            .for_each_definition_symbol(file_path, |symbol| {
                defined_symbols.insert(symbol.to_string());
            });
        self.display_related_dependents(&defined_symbols).is_empty()
    }
}
