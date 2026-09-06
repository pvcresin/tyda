use super::knowledge_precedence::{DeclKind, MergeRule, SourceKind, merge_rule};
use super::*;
use crate::rbs::ir as rbs_ir;

impl TypeRegistry {
    pub fn merge_rbs_registry(&mut self, rbs_registry: &TypeRegistry) {
        self.merge_external_type_registry_inner(rbs_registry, false, false);
    }

    // lazy stdlib constants are lookup-only and are never emitted in RBS output.
    pub fn merge_stdlib_rbs_registry(&mut self, rbs_registry: &TypeRegistry) {
        self.merge_external_type_registry_inner(rbs_registry, false, true);
    }

    pub fn merge_rbs_registry_preserving_user_defined(&mut self, rbs_registry: &TypeRegistry) {
        self.merge_external_type_registry_inner(rbs_registry, true, false);
    }

    fn merge_external_type_registry_inner(
        &mut self,
        rbs_registry: &TypeRegistry,
        propagate_user_defined: bool,
        constants_external: bool,
    ) {
        // carry over unresolved method aliases; finalize once ancestors are complete.
        for pending in &rbs_registry.pending_method_aliases {
            self.push_pending_method_alias(pending.clone());
        }
        // carry over forward references inside scoped types; finalize after merge.
        for pending in &rbs_registry.pending_scoped_type_refs {
            self.push_pending_scoped_type_ref(pending.clone());
        }
        for (alias_name, ty) in &rbs_registry.tail().type_aliases {
            self.tail_mut()
                .type_aliases
                .entry(alias_name.clone())
                .or_insert_with(|| ty.clone());
        }
        for (name, ty) in &rbs_registry.tail().global_variables {
            self.tail_mut()
                .global_variables
                .entry(name.clone())
                .or_insert_with(|| ty.clone());
        }
        for (class_name, class_meta) in &rbs_registry.tail().method_block_meta {
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
        for (class_name, rbs_data) in &rbs_registry.class_data {
            if propagate_user_defined && rbs_data.user_defined {
                let data = self.class_data_mut(class_name);
                data.user_defined = true;
            }
            self.merge_external_type_class(
                class_name,
                rbs_data,
                propagate_user_defined,
                constants_external,
            );
        }
        self.invalidate_reverse_indexes();
        self.mixin_hook_mixins_applied = false;
        self.includer_bound_dsl_applied = false;
    }

    pub fn merge_user_rbs_registry(&mut self, rbs_registry: &TypeRegistry) {
        for (alias_name, ty) in &rbs_registry.tail().type_aliases {
            self.tail_mut()
                .type_aliases
                .entry(alias_name.clone())
                .or_insert_with(|| ty.clone());
        }
        for (name, ty) in &rbs_registry.tail().global_variables {
            self.tail_mut()
                .global_variables
                .entry(name.clone())
                .or_insert_with(|| ty.clone());
        }
        for (class_name, rbs_data) in &rbs_registry.class_data {
            self.merge_user_external_type_class(class_name, rbs_data);
        }
        self.invalidate_reverse_indexes();
        self.mixin_hook_mixins_applied = false;
        self.includer_bound_dsl_applied = false;
    }

    pub fn merge_rbs_class_from(&mut self, rbs_registry: &TypeRegistry, class_name: &str) -> bool {
        let Some(rbs_data) = rbs_registry.class_data.get(class_name) else {
            return false;
        };
        self.merge_external_type_class(class_name, rbs_data, false, false);
        self.invalidate_reverse_indexes();
        self.mixin_hook_mixins_applied = false;
        self.includer_bound_dsl_applied = false;
        true
    }

    fn merge_external_type_class(
        &mut self,
        class_name: &str,
        rbs_data: &ClassData,
        from_user_source: bool,
        constants_external: bool,
    ) {
        if rbs_data
            .methods
            .iter()
            .any(|method| Self::method_needs_mixin_hook_call_site(method))
        {
            self.has_mixin_hook_methods = true;
        }
        // set the registry flag if this merge brings in a dirty pattern (a global gate for the synthesis
        // path; write to self before borrowing `data`).
        if rbs_data.cold().dirty_method_pattern.is_some() {
            self.mark_has_dirty_patterns();
        }
        let shared_superclass = rbs_data
            .superclass
            .as_ref()
            .map(|sc| self.intern_shared_name(sc));
        let shared_mixins: Vec<Mixin> = rbs_data
            .mixins
            .iter()
            .map(|mixin| Mixin {
                module_name: self.intern_shared_name(&mixin.module_name),
                type_args: mixin.type_args.clone(),
                kind: mixin.kind.clone(),
                external_source: mixin.external_source,
            })
            .collect();
        let shared_included_hook_mixins: Vec<Mixin> = rbs_data
            .hook_mixins_by_kind(&MixinKind::Include)
            .iter()
            .map(|mixin| Mixin {
                module_name: self.intern_shared_name(&mixin.module_name),
                type_args: mixin.type_args.clone(),
                kind: mixin.kind.clone(),
                external_source: mixin.external_source,
            })
            .collect();
        let shared_extended_hook_mixins: Vec<Mixin> = rbs_data
            .hook_mixins_by_kind(&MixinKind::Extend)
            .iter()
            .map(|mixin| Mixin {
                module_name: self.intern_shared_name(&mixin.module_name),
                type_args: mixin.type_args.clone(),
                kind: mixin.kind.clone(),
                external_source: mixin.external_source,
            })
            .collect();
        let shared_prepended_hook_mixins: Vec<Mixin> = rbs_data
            .hook_mixins_by_kind(&MixinKind::Prepend)
            .iter()
            .map(|mixin| Mixin {
                module_name: self.intern_shared_name(&mixin.module_name),
                type_args: mixin.type_args.clone(),
                kind: mixin.kind.clone(),
                external_source: mixin.external_source,
            })
            .collect();
        if !shared_included_hook_mixins.is_empty()
            || !shared_extended_hook_mixins.is_empty()
            || !shared_prepended_hook_mixins.is_empty()
        {
            self.has_mixin_hook_mixins = true;
        }
        let shared_required_ancestors: Vec<SharedName> = rbs_data
            .cold()
            .required_ancestors
            .iter()
            .map(|ancestor| self.intern_shared_name(ancestor))
            .collect();
        let shared_required_ancestor_type_args: Vec<(SharedName, Vec<rbs_ir::RbsType>)> = rbs_data
            .cold()
            .required_ancestor_type_args
            .iter()
            .map(|(ancestor, args)| (self.intern_shared_name(ancestor), args.clone()))
            .collect();
        let shared_sorbet_modifier_comments = rbs_data.cold().sorbet_modifier_comments.clone();
        // `Arc<MethodDef>` clone shares the pointer (reduces RSS/CPU for RBI shape merges).
        let shared_methods: Vec<Arc<MethodDef>> = rbs_data.methods.clone();
        let shared_method_file_paths: Vec<((Sym, bool), SharedPath)> = rbs_data
            .method_file_paths
            .iter()
            .map(|(key, path)| (key, path.clone()))
            .collect();
        let shared_annotated_params: Vec<((Sym, bool), HashMap<usize, Type>)> = rbs_data
            .cold()
            .annotated_params
            .iter()
            .map(|(key, params)| {
                (
                    *key,
                    params.iter().map(|(idx, ty)| (*idx, ty.clone())).collect(),
                )
            })
            .collect();
        let shared_call_sites: Vec<CallSite> = rbs_data
            .call_sites
            .iter()
            .cloned()
            .map(|mut call_site| {
                self.intern_call_site(&mut call_site);
                call_site
            })
            .collect();
        let is_user_defined = self
            .class_data
            .get(class_name)
            .is_some_and(|d| d.user_defined);
        if !rbs_data.cold().includer_bound_dsl.is_empty() {
            self.has_includer_bound_dsl = true;
            self.includer_bound_dsl_applied = false;
        }
        let data = self.class_data_mut(class_name);
        if data.loc.is_none() {
            data.loc = rbs_data.loc;
        }
        if data.file_path.is_none() {
            data.file_path = rbs_data.file_path.clone();
        }
        // knowledge source = Ruby source definition (`from_user_source`) vs external; use it to look up the superclass /
        // `is_module` merge rule from [`knowledge_precedence::merge_rule`].
        let source = if from_user_source {
            SourceKind::RubySource
        } else {
            SourceKind::RbsDecl
        };
        let superclass_rule = merge_rule(source, DeclKind::Superclass, is_user_defined);
        if let Some(sc) = shared_superclass
            && data.superclass.is_none()
            && superclass_rule == MergeRule::AddIfAbsent
        {
            data.superclass = Some(sc);
            // don't allocate the cold Box if both sides are empty, since the assignment would be a no-op.
            if !rbs_data.cold().superclass_type_args.is_empty()
                || !data.cold().superclass_type_args.is_empty()
            {
                data.cold_mut().superclass_type_args = rbs_data.cold().superclass_type_args.clone();
            }
        } else if data.cold().superclass_type_args.is_empty()
            && !rbs_data.cold().superclass_type_args.is_empty()
        {
            data.cold_mut().superclass_type_args = rbs_data.cold().superclass_type_args.clone();
        }
        let is_module_rule = merge_rule(source, DeclKind::IsModule, is_user_defined);
        if rbs_data.is_module && is_module_rule == MergeRule::Override {
            data.is_module = true;
        }
        // pin the single-value-slot merge rule with an assert (keeps it mechanically in sync with the inline implementation).
        debug_assert_eq!(
            merge_rule(source, DeclKind::TypeParams, is_user_defined),
            MergeRule::AddIfAbsent
        );
        if !rbs_data.cold().class_type_params.is_empty() && data.cold().class_type_params.is_empty()
        {
            data.cold_mut().class_type_params = rbs_data.cold().class_type_params.clone();
        }
        if !rbs_data.cold().class_type_param_bounds.is_empty()
            && data.cold().class_type_param_bounds.is_empty()
        {
            data.cold_mut().class_type_param_bounds =
                rbs_data.cold().class_type_param_bounds.clone();
        }
        if !rbs_data.cold().class_type_param_defaults.is_empty()
            && data.cold().class_type_param_defaults.is_empty()
        {
            data.cold_mut().class_type_param_defaults =
                rbs_data.cold().class_type_param_defaults.clone();
        }
        debug_assert_eq!(
            merge_rule(source, DeclKind::Mixin, is_user_defined),
            MergeRule::AppendDedup
        );
        // These `shared_*` buffers exist only to end the `rbs_data` borrow, so move out of them
        // instead of cloning again on the way in.
        for mixin in shared_mixins {
            if !data.mixins.iter().any(|existing| {
                existing.module_name == mixin.module_name
                    && existing.kind == mixin.kind
                    && existing.type_args == mixin.type_args
            }) {
                data.mixins.push(mixin);
            }
        }
        for mixin in shared_included_hook_mixins {
            let hooks = data.hook_mixins_mut();
            if !hooks.included.iter().any(|existing| {
                existing.module_name == mixin.module_name && existing.kind == mixin.kind
            }) {
                hooks.included.push(mixin);
            }
        }
        for mixin in shared_extended_hook_mixins {
            let hooks = data.hook_mixins_mut();
            if !hooks.extended.iter().any(|existing| {
                existing.module_name == mixin.module_name && existing.kind == mixin.kind
            }) {
                hooks.extended.push(mixin);
            }
        }
        for mixin in shared_prepended_hook_mixins {
            let hooks = data.hook_mixins_mut();
            if !hooks.prepended.iter().any(|existing| {
                existing.module_name == mixin.module_name && existing.kind == mixin.kind
            }) {
                hooks.prepended.push(mixin);
            }
        }
        for ancestor in shared_required_ancestors {
            let cold = data.cold_mut();
            if !cold.required_ancestors.contains(&ancestor) {
                cold.required_ancestors.push(ancestor);
            }
        }
        for (ancestor, type_args) in shared_required_ancestor_type_args {
            let cold = data.cold_mut();
            if !cold.required_ancestors.contains(&ancestor) {
                cold.required_ancestors.push(ancestor.clone());
            }
            if type_args.is_empty() {
                continue;
            }
            if let Some((_, existing_args)) = cold
                .required_ancestor_type_args
                .iter_mut()
                .find(|(existing, _)| *existing == ancestor)
            {
                if existing_args.is_empty() {
                    *existing_args = type_args;
                }
            } else {
                cold.required_ancestor_type_args.push((ancestor, type_args));
            }
        }
        for comment in shared_sorbet_modifier_comments {
            let cold = data.cold_mut();
            if !cold.sorbet_modifier_comments.contains(&comment) {
                cold.sorbet_modifier_comments.push(comment);
            }
        }
        debug_assert_eq!(
            merge_rule(source, DeclKind::Method, is_user_defined),
            MergeRule::AddIfAbsent
        );
        // merge the dirty skeleton pattern before the methods loop (suppresses per-file dirty materialization = byte-compatible render).
        debug_assert_eq!(
            merge_rule(source, DeclKind::DirtyPattern, is_user_defined),
            MergeRule::AddIfAbsent
        );
        if let Some(src_pattern) = &rbs_data.cold().dirty_method_pattern {
            data.merge_dirty_pattern(src_pattern);
        }
        for dsl in &rbs_data.cold().includer_bound_dsl {
            if !data.cold().includer_bound_dsl.contains(dsl) {
                data.cold_mut().includer_bound_dsl.push(dsl.clone());
            }
        }
        let target_has_pattern = data.cold().dirty_method_pattern.is_some();
        for (method_idx, method) in shared_methods.into_iter().enumerate() {
            let method_key = (method.name, method.is_singleton);
            // don't materialize dirty-family instance methods that the pattern can already synthesize.
            if target_has_pattern
                && !method.is_singleton
                && let Some(pattern) = &data.cold().dirty_method_pattern
                && pattern.synthesize(method.name.as_str()).is_some()
            {
                continue;
            }
            // when the same-named variant is duplicated, keep only the synthesis-source slot's variant (synthetic never shadows user).
            if method.rbs_file_source
                && method.synthetic_dsl_source
                && rbs_data
                    .method_index
                    .get(method.name.as_str())
                    .and_then(|slots| slots.get(method.is_singleton))
                    .is_some_and(|slot_idx| slot_idx != method_idx)
            {
                continue;
            }
            let method_file_path = rbs_data
                .method_file_paths
                .get(&method_key)
                .cloned()
                .or_else(|| method.loc.and_then(|_| rbs_data.file_path.clone()));
            let variant_exists = data
                .method_index
                .get(method.name.as_str())
                .is_some_and(|slots| slots.has(method.is_singleton));
            if !variant_exists {
                Self::index_method_if_absent(
                    data,
                    method.name,
                    method.is_singleton,
                    data.methods.len(),
                );
                if let Some(file_path) = method_file_path {
                    data.method_file_paths
                        .insert_if_absent(method_key, file_path);
                }
                data.methods.push(method);
            }
        }
        for (method_key, file_path) in shared_method_file_paths {
            data.method_file_paths
                .insert_if_absent(method_key, file_path);
        }
        // merge visibility overrides with AddIfAbsent too (never overwrite existing entries).
        for (key, visibility) in &rbs_data.cold().method_visibility {
            data.cold_mut()
                .method_visibility
                .entry(*key)
                .or_insert(*visibility);
        }
        for (method_key, params) in shared_annotated_params {
            let target = data
                .cold_mut()
                .annotated_params
                .entry(method_key)
                .or_default();
            for (idx, ty) in params {
                target.entry(idx).or_insert(ty);
            }
        }
        debug_assert_eq!(
            merge_rule(source, DeclKind::Ivar, is_user_defined),
            MergeRule::AppendDedup
        );
        for (ivar_name, rbs_types) in &rbs_data.ivars {
            let entry = data.ivars.entry(*ivar_name).or_default();
            for ty in rbs_types {
                if !entry.contains(ty) {
                    entry.push(ty.clone());
                }
            }
        }
        for (ivar_name, rbs_types) in &rbs_data.cold().singleton_ivars {
            let entry = data
                .cold_mut()
                .singleton_ivars
                .entry(*ivar_name)
                .or_default();
            for ty in rbs_types {
                if !entry.contains(ty) {
                    entry.push(ty.clone());
                }
            }
        }
        for (var_name, rbs_types) in &rbs_data.cold().class_variables {
            let entry = data
                .cold_mut()
                .class_variables
                .entry(*var_name)
                .or_default();
            for ty in rbs_types {
                if !entry.contains(ty) {
                    entry.push(ty.clone());
                }
            }
        }
        debug_assert_eq!(
            merge_rule(source, DeclKind::Constant, is_user_defined),
            MergeRule::AddIfAbsent
        );
        for (const_name, const_def) in &rbs_data.constants {
            data.constants.entry(*const_name).or_insert_with(|| {
                let mut const_def = const_def.clone();
                if constants_external {
                    const_def.external_source = true;
                }
                const_def
            });
        }
        if !shared_call_sites.is_empty() {
            // call site dedup uses a fingerprint (linear `contains` would be quadratic across thousands of sites).
            let mut existing: rustc_hash::FxHashSet<u64> =
                data.call_sites.iter().map(call_site_fingerprint).collect();
            for call_site in shared_call_sites {
                if existing.insert(call_site_fingerprint(&call_site)) {
                    data.call_sites.push(call_site);
                    data.call_sites_revision = data.call_sites_revision.wrapping_add(1);
                }
            }
        }
    }

    fn merge_user_external_type_class(&mut self, class_name: &str, rbs_data: &ClassData) {
        self.merge_external_type_class(class_name, rbs_data, false, false);

        // special case: pre-loaded user `.rbs` only overwrites the return type of user-source methods (outside the merge_rule table).
        let Some(data) = self.class_data.get_mut(class_name) else {
            return;
        };

        for method in &rbs_data.methods {
            let Some(existing_idx) = data
                .method_index
                .get(method.name.as_str())
                .and_then(|slots| slots.get(method.is_singleton))
            else {
                continue;
            };
            let Some(existing) = data.methods.get_mut(existing_idx) else {
                continue;
            };
            if existing.is_external_rbs_source() {
                continue;
            }

            let existing = Arc::make_mut(existing);
            existing.raw_return_type = method.raw_return_type.clone();
            existing.rbs_annotated = true;
            existing.rbs_method_types = method.rbs_method_types.clone();
            existing.extra_overloads = method.extra_overloads.clone();
        }
    }

    pub fn mark_all_methods_as_external_source(&mut self) {
        for data in self.class_data.values_mut() {
            data.user_defined = false;
            for method in &mut data.methods {
                let method = Arc::make_mut(method);
                method.rbs_file_source = true;
                method.synthetic_dsl_source = false;
            }
        }
        self.refresh_mixin_hook_method_flag();
    }

    pub fn remove_external_methods_for_class(&mut self, class_name: &str) -> bool {
        let removed = if let Some(data) = self.class_data.get_mut(class_name) {
            let before = data.methods.len();
            data.methods.retain(|m| !m.is_external_rbs_source());
            if data.methods.len() != before {
                Self::rebuild_method_index(data);
            }
            data.methods.len() != before
        } else {
            false
        };
        if removed {
            self.refresh_mixin_hook_method_flag();
            self.mixin_hook_mixins_applied = false;
        }
        removed
    }

    pub fn remove_class_if_external_only(&mut self, class_name: &str) {
        if let Some(data) = self.class_data.get(class_name)
            && !data.user_defined
            && data.methods.iter().all(|m| m.is_external_rbs_source())
        {
            self.class_data.remove(class_name);
            self.refresh_mixin_hook_method_flag();
            self.mixin_hook_mixins_applied = false;
        }
    }
}
