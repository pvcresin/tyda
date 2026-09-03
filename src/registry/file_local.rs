use super::*;

pub type MethodBodyCallSitesByClass = Vec<(SharedName, Arc<[CallSite]>)>;
pub type MethodBodyIvarTypes = Vec<(Sym, Vec<Type>)>;
pub type MethodBodyIvarTypesByClass = Vec<(SharedName, MethodBodyIvarTypes)>;
pub type MethodBodyBlockMetaByClass = Vec<(SharedName, ClassMethodBlockMeta)>;

#[derive(Debug, Clone, Default)]
pub struct MethodBodySummary {
    pub call_sites_by_class: MethodBodyCallSitesByClass,
    pub ivar_types_by_class: MethodBodyIvarTypesByClass,
    pub method_block_meta_by_class: MethodBodyBlockMetaByClass,
}

impl MethodBodySummary {
    pub fn is_empty(&self) -> bool {
        self.call_sites_by_class.is_empty()
            && self.ivar_types_by_class.is_empty()
            && self.method_block_meta_by_class.is_empty()
    }

    pub fn accumulate_deep_bytes(
        &self,
        b: &mut RegistryDeepBytes,
        seen: &mut rustc_hash::FxHashSet<usize>,
    ) {
        for (name, call_sites) in &self.call_sites_by_class {
            b.container_bytes +=
                std::mem::size_of::<(SharedName, Arc<[CallSite]>)>() + name.len() + 16;
            if !seen.insert(call_sites.as_ptr() as usize) {
                continue;
            }
            for call_site in call_sites.iter() {
                b.call_site_count += 1;
                b.call_site_bytes += call_site.deep_bytes();
            }
        }
        for (class_name, ivars) in &self.ivar_types_by_class {
            b.container_bytes +=
                std::mem::size_of::<(SharedName, MethodBodyIvarTypes)>() + class_name.len();
            for (_ivar_name, types) in ivars {
                b.constant_ivar_bytes += std::mem::size_of::<(Sym, Vec<Type>)>();
                b.constant_ivar_bytes += types.len() * std::mem::size_of::<Type>();
                b.constant_ivar_bytes += types.iter().map(Type::deep_extra_bytes).sum::<usize>();
            }
        }
        for (_, meta) in &self.method_block_meta_by_class {
            b.container_bytes += meta.deep_bytes();
        }
        b.total_bytes =
            b.container_bytes + b.constant_ivar_bytes + b.call_site_bytes + b.method_body_bytes;
    }

    pub fn shrink_to_fit(&mut self) {
        self.call_sites_by_class.shrink_to_fit();
        self.ivar_types_by_class.shrink_to_fit();
        for (_, ivars) in self.ivar_types_by_class.iter_mut() {
            ivars.shrink_to_fit();
            for (_, types) in ivars.iter_mut() {
                types.shrink_to_fit();
            }
        }
        self.method_block_meta_by_class.shrink_to_fit();
        for (_, class_meta) in self.method_block_meta_by_class.iter_mut() {
            class_meta.instance.shrink_to_fit();
            for meta in class_meta.instance.values_mut() {
                meta.yield_param_types.shrink_to_fit();
            }
            class_meta.singleton.shrink_to_fit();
            for meta in class_meta.singleton.values_mut() {
                meta.yield_param_types.shrink_to_fit();
            }
        }
    }
}

impl TypeRegistry {
    pub fn apply_method_body_summary(&mut self, summary: &MethodBodySummary) {
        for (class_name, call_sites) in &summary.call_sites_by_class {
            if call_sites.is_empty() {
                continue;
            }
            // summary chunk/Arc names share the pointer (no per-site deep copy needed).
            let data = self.class_data_mut(class_name.as_ref());
            data.call_sites.push_chunk(Arc::clone(call_sites));
            data.call_sites_revision = data.call_sites_revision.wrapping_add(1);
        }
        for (class_name, ivar_types) in &summary.ivar_types_by_class {
            let data = self.class_data_mut(class_name.as_ref());
            for (ivar_name, types) in ivar_types {
                let entry = data.ivars.entry(*ivar_name).or_default();
                for ty in types {
                    if !entry.contains(ty) {
                        entry.push(ty.clone());
                    }
                }
            }
        }
        for (class_name, class_meta) in &summary.method_block_meta_by_class {
            let class_name = self.intern_shared_name(class_name);
            let instance_meta: Vec<(SharedName, MethodBlockMeta)> = class_meta
                .instance
                .iter()
                .map(|(method_name, meta)| {
                    let mut meta = meta.clone();
                    self.intern_method_block_meta_inner(&mut meta);
                    (self.intern_shared_name(method_name), meta)
                })
                .collect();
            let singleton_meta: Vec<(SharedName, MethodBlockMeta)> = class_meta
                .singleton
                .iter()
                .map(|(method_name, meta)| {
                    let mut meta = meta.clone();
                    self.intern_method_block_meta_inner(&mut meta);
                    (self.intern_shared_name(method_name), meta)
                })
                .collect();
            let target = self
                .tail_mut()
                .method_block_meta
                .entry(class_name)
                .or_default();
            for (method_name, meta) in instance_meta {
                target.get_or_insert(method_name, false, meta);
            }
            for (method_name, meta) in singleton_meta {
                target.get_or_insert(method_name, true, meta);
            }
        }
    }

    // a per-file snapshot references the external registry for accuracy, while keeping retained facts scoped to the file.
    pub fn retain_file_facts(&mut self, file_path: &str) {
        self.class_data.retain(|_, data| {
            data.constants
                .retain(|_, constant| constant.file_path.as_deref() == Some(file_path));
            data.method_file_paths
                .retain_paths(|path| path.as_ref() == file_path);
            if !data.methods.is_empty() {
                // every retained path already equals `file_path`, so presence is the filter.
                let method_file_paths = std::mem::take(&mut data.method_file_paths);
                data.methods.retain(|method| {
                    method_file_paths
                        .get(&(method.name, method.is_singleton))
                        .is_some()
                });
                data.method_file_paths = method_file_paths;
                Self::rebuild_method_index(data);
            }

            data.file_path.as_deref() == Some(file_path)
                || !data.constants.is_empty()
                || !data.cold().class_variables.is_empty()
                || !data.methods.is_empty()
        });
        if let Some(tail) = self.tail_opt_mut() {
            tail.method_block_meta.clear();
            tail.body_fact_class_names.clear();
            tail.type_aliases.clear();
        }
        self.invalidate_reverse_indexes();
        self.mixin_hook_mixins_applied = false;
    }

    fn is_stdlib_root_class(name: &str) -> bool {
        matches!(
            name,
            "Object" | "BasicObject" | "Kernel" | "Module" | "Class"
        )
    }

    /// Drop workspace preload copies, keeping only classes this analysis pass defined.
    /// `ClassData.file_path` is first-writer-wins (Object stays on stdlib), so path matching
    /// cannot identify this file's contributions.
    pub fn retain_current_pass_facts(&mut self) {
        let keep = std::mem::take(&mut self.file_contribution_names);
        let contributed_methods = std::mem::take(&mut self.file_contribution_method_names);
        if keep.is_empty() {
            return;
        }
        self.class_data.retain(|name, data| {
            if !keep.contains(name.as_str()) {
                return false;
            }
            if Self::is_stdlib_root_class(name) {
                data.methods
                    .retain(|method| contributed_methods.contains(&method.name));
                Self::rebuild_method_index(data);
                return !data.methods.is_empty() || !data.constants.is_empty();
            }
            true
        });
        if let Some(tail) = self.tail_opt_mut() {
            tail.method_block_meta.clear();
            tail.body_fact_class_names.clear();
            tail.type_aliases.clear();
        }
        self.invalidate_reverse_indexes();
        self.mixin_hook_mixins_applied = false;
    }

    pub fn take_method_body_summary(&mut self, file_path: &str) -> MethodBodySummary {
        let body_fact_class_names = self
            .tail_opt_mut()
            .map(|tail| std::mem::take(&mut tail.body_fact_class_names))
            .unwrap_or_default();
        let pooled_names: HashMap<String, SharedName> = body_fact_class_names
            .iter()
            .map(|class_name| (class_name.clone(), self.shared_name(class_name)))
            .collect();
        let mut summary = MethodBodySummary::default();
        for class_name in body_fact_class_names {
            let is_local_owner = if let Some(data) = self.class_data.get_mut(class_name.as_str()) {
                if !data.call_sites.is_empty() {
                    summary.call_sites_by_class.push((
                        pooled_names
                            .get(&class_name)
                            .cloned()
                            .unwrap_or_else(|| Arc::<str>::from(class_name.as_str())),
                        {
                            data.call_site_fingerprints = None;
                            data.call_sites_revision = data.call_sites_revision.wrapping_add(1);
                            // shed per-site excess capacity before freezing (once frozen, the
                            // `Arc<[_]>` chunk is immutable and can't be shrunk).
                            let mut sites = data.call_sites.take_all();
                            for site in &mut sites {
                                site.arg_types.shrink_to_fit();
                                site.keyword_arg_types.shrink_to_fit();
                            }
                            Arc::<[CallSite]>::from(sites)
                        },
                    ));
                }
                if !data.ivars.is_empty() {
                    summary.ivar_types_by_class.push((
                        pooled_names
                            .get(&class_name)
                            .cloned()
                            .unwrap_or_else(|| Arc::<str>::from(class_name.as_str())),
                        data.ivars.drain().collect(),
                    ));
                }
                data.has_pending_call_site_summary = false;
                data.file_path.as_deref() == Some(file_path)
            } else {
                false
            };
            if is_local_owner
                && let Some((class_name, class_meta)) = self
                    .tail_mut()
                    .method_block_meta
                    .remove_entry(class_name.as_str())
            {
                summary
                    .method_block_meta_by_class
                    .push((class_name, class_meta));
            }
        }

        summary
    }

    pub fn build_classes_for_file(&self, file_path: &str) -> Vec<ClassInfo> {
        let mut classes: Vec<ClassInfo> = self
            .class_data
            .iter()
            .filter_map(|(name, data)| {
                let info = self.build_class_info(name, data, Some(file_path));
                let has_constants = !info.constants.is_empty();
                if has_constants || info.file_path.as_deref() == Some(file_path) {
                    Some(info)
                } else {
                    None
                }
            })
            .collect();
        classes.sort_by(|a, b| a.name.cmp(&b.name));
        classes
    }

    pub fn methods_for_file(&self, file_path: &str) -> Vec<(String, MethodSig)> {
        type MethodKey = (String, String, bool, Option<(u32, u32)>);
        let mut methods_by_key: std::collections::HashMap<MethodKey, (String, MethodSig)> =
            std::collections::HashMap::new();
        for (class_name, data) in &self.class_data {
            let has_constants_in_file = data
                .constants
                .values()
                .any(|constant| constant.file_path.as_deref() == Some(file_path));
            if !has_constants_in_file && data.file_path.as_deref() != Some(file_path) {
                continue;
            }
            for method in self.build_method_sigs_for_class(class_name, data) {
                if method.rbs_file_source {
                    continue;
                }
                let key: MethodKey = (
                    class_name.to_string(),
                    method.name.clone(),
                    method.is_singleton,
                    method.loc.map(|loc| (loc.line, loc.column)),
                );
                methods_by_key.insert(key, (class_name.to_string(), method));
            }
        }
        let mut deduped: Vec<(String, MethodSig)> = methods_by_key.into_values().collect();
        deduped.sort_by(|(class_a, method_a), (class_b, method_b)| {
            let loc_a = method_a
                .loc
                .map(|loc| (loc.line, loc.column))
                .unwrap_or((u32::MAX, u32::MAX));
            let loc_b = method_b
                .loc
                .map(|loc| (loc.line, loc.column))
                .unwrap_or((u32::MAX, u32::MAX));
            loc_a
                .cmp(&loc_b)
                .then_with(|| class_a.cmp(class_b))
                .then_with(|| method_a.name.cmp(&method_b.name))
                .then_with(|| method_a.is_singleton.cmp(&method_b.is_singleton))
        });
        deduped
    }
}
