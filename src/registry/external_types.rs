use super::knowledge_precedence::{DeclKind, MergeRule, SourceKind, merge_rule};
use super::*;
use crate::rbs::ir as rbs_ir;
use crate::types::MethodOrigin;

/// What `merge_stdlib_rbs_registry` makes of each class of a shared declaration in a registry
/// that does not have the class yet, built once so each merge can share it.
#[derive(Debug, Default)]
pub struct PreparedStdlibMerge {
    /// In the declaration registry's class order.
    classes: Vec<FreshClass>,
}

#[derive(Debug)]
pub(super) struct FreshClass {
    name: Sym,
    data: Arc<ClassData>,
    sets_mixin_hook_methods: bool,
    sets_dirty_patterns: bool,
    sets_mixin_hook_mixins: bool,
    sets_includer_bound_dsl: bool,
    /// Whether merging the declaration again into exactly `data` leaves it as it is, found by
    /// doing it the first time a registry still holding `data` merges the declaration again.
    remerge_is_noop: std::sync::OnceLock<bool>,
}

impl FreshClass {
    fn remerge_is_noop(&self, rbs_data: &ClassData) -> bool {
        *self.remerge_is_noop.get_or_init(|| {
            let mut again = TypeRegistry::new();
            again
                .class_data
                .insert_shared(self.name, Arc::clone(&self.data));
            again.merge_external_type_class(&self.name, rbs_data, false, true);
            again.class_data.get(&self.name).is_some_and(|merged| {
                content_fingerprint(merged) == content_fingerprint(&self.data)
            })
        })
    }
}

/// A 128-bit hash of the value's `Debug` text, streamed so the text is never built. `Debug`
/// lists every field in iteration order, so equal fingerprints mean the same content laid out
/// the same way. The keys are random per process; fingerprints are only compared within it.
fn content_fingerprint(value: &impl std::fmt::Debug) -> u128 {
    use std::hash::{BuildHasher, Hasher};
    static KEYS: std::sync::LazyLock<(std::hash::RandomState, std::hash::RandomState)> =
        std::sync::LazyLock::new(|| (std::hash::RandomState::new(), std::hash::RandomState::new()));
    const CHUNK: usize = 1024;
    /// `Debug` writes many short pieces; the hashers take them in chunks, which hashes the
    /// same bytes for less work per piece.
    struct Stream {
        hashers: (std::hash::DefaultHasher, std::hash::DefaultHasher),
        chunk: [u8; CHUNK],
        len: usize,
    }
    impl Stream {
        fn flush(&mut self) {
            self.hashers.0.write(&self.chunk[..self.len]);
            self.hashers.1.write(&self.chunk[..self.len]);
            self.len = 0;
        }
    }
    impl std::fmt::Write for Stream {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            let bytes = text.as_bytes();
            if self.len + bytes.len() > CHUNK {
                self.flush();
            }
            if bytes.len() > CHUNK {
                self.hashers.0.write(bytes);
                self.hashers.1.write(bytes);
            } else {
                self.chunk[self.len..self.len + bytes.len()].copy_from_slice(bytes);
                self.len += bytes.len();
            }
            Ok(())
        }
    }
    let mut stream = Stream {
        hashers: (KEYS.0.build_hasher(), KEYS.1.build_hasher()),
        chunk: [0; CHUNK],
        len: 0,
    };
    let _ = std::fmt::write(&mut stream, format_args!("{value:?}"));
    stream.flush();
    (u128::from(stream.hashers.0.finish()) << 64) | u128::from(stream.hashers.1.finish())
}

/// Prepared classes by content, so declarations that hold the same class share one copy and a
/// merge can tell that a registry still holds exactly that class.
#[derive(Debug, Default)]
pub struct PreparedClassPool {
    /// Fingerprints are taken only once a second class of the same name arrives; most names
    /// come once.
    classes: FxHashMap<Sym, Vec<PooledClass>>,
}

/// A pooled class and its fingerprint, taken when first needed.
type PooledClass = (std::cell::OnceCell<u128>, Arc<ClassData>);

impl PreparedClassPool {
    fn share(&mut self, name: Sym, data: Arc<ClassData>) -> Arc<ClassData> {
        let same = self.classes.entry(name).or_default();
        if same.is_empty() {
            same.push((std::cell::OnceCell::new(), Arc::clone(&data)));
            return data;
        }
        let key = content_fingerprint(&data);
        if let Some((_, existing)) = same.iter().find(|(fingerprint, existing)| {
            *fingerprint.get_or_init(|| content_fingerprint(existing)) == key
        }) {
            return Arc::clone(existing);
        }
        same.push((std::cell::OnceCell::from(key), Arc::clone(&data)));
        data
    }
}

impl PreparedStdlibMerge {
    /// Merges each class of `shape` into an empty registry, as `merge_stdlib_rbs_registry`
    /// would into a registry without that class. `shape` must not change afterwards.
    pub fn new(shape: &TypeRegistry, pool: &mut PreparedClassPool) -> Self {
        let classes = shape
            .class_data
            .iter()
            .map(|(name, rbs_data)| FreshClass::prepare(*name, rbs_data, true, Some(&mut *pool)))
            .collect();
        Self { classes }
    }
}

impl FreshClass {
    /// What `merge_external_type_class` makes of `rbs_data` in a registry without the class
    /// and without a name pool.
    fn prepare(
        name: Sym,
        rbs_data: &ClassData,
        constants_external: bool,
        pool: Option<&mut PreparedClassPool>,
    ) -> Self {
        let mut scratch = TypeRegistry::new();
        scratch.merge_external_type_class(&name, rbs_data, false, constants_external);
        debug_assert_eq!(scratch.class_data.len(), 1);
        let data = scratch
            .class_data
            .remove(&name)
            .expect("merging a class creates it");
        let data = match pool {
            Some(pool) => pool.share(name, data),
            None => data,
        };
        FreshClass {
            name,
            data,
            remerge_is_noop: std::sync::OnceLock::new(),
            sets_mixin_hook_methods: scratch.has_mixin_hook_methods,
            sets_dirty_patterns: scratch.has_dirty_patterns,
            sets_mixin_hook_mixins: scratch.has_mixin_hook_mixins,
            sets_includer_bound_dsl: scratch.has_includer_bound_dsl,
        }
    }
}

impl TypeRegistry {
    pub fn merge_rbs_registry(&mut self, rbs_registry: &TypeRegistry) {
        self.merge_external_type_registry_inner(rbs_registry, false, false, None);
    }

    // lazy stdlib constants are lookup-only and are never emitted in RBS output.
    pub fn merge_stdlib_rbs_registry(&mut self, rbs_registry: &TypeRegistry) {
        self.merge_external_type_registry_inner(rbs_registry, false, true, None);
    }

    /// `merge_stdlib_rbs_registry` that shares the classes this registry does not have yet
    /// with `prepared` instead of building them again.
    pub fn merge_prepared_stdlib_rbs_registry(
        &mut self,
        rbs_registry: &TypeRegistry,
        prepared: &PreparedStdlibMerge,
    ) {
        self.merge_external_type_registry_inner(rbs_registry, false, true, Some(prepared));
    }

    pub fn merge_rbs_registry_preserving_user_defined(&mut self, rbs_registry: &TypeRegistry) {
        self.merge_external_type_registry_inner(rbs_registry, true, false, None);
    }

    fn merge_external_type_registry_inner(
        &mut self,
        rbs_registry: &TypeRegistry,
        propagate_user_defined: bool,
        constants_external: bool,
        prepared: Option<&PreparedStdlibMerge>,
    ) {
        self.has_source_base_case_equality_override |=
            rbs_registry.has_source_base_case_equality_override;
        self.has_source_numeric_case_equality_override |=
            rbs_registry.has_source_numeric_case_equality_override;
        self.has_source_block_given_override |= rbs_registry.has_source_block_given_override;
        self.carry_source_namespaces(rbs_registry);
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
        // Without a name pool, merging a class this registry lacks builds exactly the prepared
        // class, so it is shared instead.
        let prepared = prepared.filter(|prepared| {
            !self.name_pool_enabled
                && !propagate_user_defined
                && constants_external
                && prepared.classes.len() == rbs_registry.class_data.len()
        });
        for (index, (class_name, rbs_data)) in rbs_registry.class_data.iter().enumerate() {
            if let Some(fresh) = prepared.map(|prepared| &prepared.classes[index])
                && fresh.name == *class_name
            {
                match self.class_data.get(class_name.as_str()) {
                    None => {
                        self.insert_fresh_external_class(fresh);
                        continue;
                    }
                    // Still the shared class, which merging again would not change.
                    Some(existing)
                        if Arc::ptr_eq(existing, &fresh.data)
                            && fresh.remerge_is_noop(rbs_data) =>
                    {
                        self.apply_fresh_merge_effects(fresh);
                        continue;
                    }
                    Some(_) => {}
                }
            }
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
        self.has_source_base_case_equality_override |=
            rbs_registry.has_source_base_case_equality_override;
        self.has_source_numeric_case_equality_override |=
            rbs_registry.has_source_numeric_case_equality_override;
        self.has_source_block_given_override |= rbs_registry.has_source_block_given_override;
        self.carry_source_namespaces(rbs_registry);
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
        let Some((name, rbs_data)) = rbs_registry.class_data.get_key_value(class_name) else {
            return false;
        };
        // Merging a class this registry lacks builds the same class every time, so the
        // declaration keeps the result and each registry shares it.
        if !self.name_pool_enabled && !self.class_data.contains_key(class_name) {
            let fresh = rbs_data
                .derived()
                .fresh_external_merge
                .get_or_init(|| FreshClass::prepare(*name, rbs_data, false, None));
            if fresh.name == *name {
                self.insert_fresh_external_class(fresh);
                self.invalidate_reverse_indexes();
                self.mixin_hook_mixins_applied = false;
                self.includer_bound_dsl_applied = false;
                return true;
            }
        }
        self.merge_external_type_class(class_name, rbs_data, false, false);
        self.invalidate_reverse_indexes();
        self.mixin_hook_mixins_applied = false;
        self.includer_bound_dsl_applied = false;
        true
    }

    /// The registry-wide effects of `merge_external_type_class` on a class this registry lacks,
    /// with the class itself taken from `fresh`.
    fn insert_fresh_external_class(&mut self, fresh: &FreshClass) {
        self.apply_fresh_merge_effects(fresh);
        self.class_data
            .insert_shared(fresh.name, Arc::clone(&fresh.data));
    }

    fn apply_fresh_merge_effects(&mut self, fresh: &FreshClass) {
        self.invalidate_owner_walks();
        if fresh.sets_mixin_hook_methods {
            self.has_mixin_hook_methods = true;
        }
        if fresh.sets_dirty_patterns {
            self.mark_has_dirty_patterns();
        }
        if fresh.sets_mixin_hook_mixins {
            self.has_mixin_hook_mixins = true;
        }
        if fresh.sets_includer_bound_dsl {
            self.has_includer_bound_dsl = true;
            self.includer_bound_dsl_applied = false;
        }
    }

    fn merge_external_type_class(
        &mut self,
        class_name: &str,
        rbs_data: &ClassData,
        from_user_source: bool,
        constants_external: bool,
    ) {
        self.invalidate_owner_walks();
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
                untyped_type_arity: mixin.untyped_type_arity,
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
                untyped_type_arity: mixin.untyped_type_arity,
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
                untyped_type_arity: mixin.untyped_type_arity,
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
                untyped_type_arity: mixin.untyped_type_arity,
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
        let method_aliases = rbs_data.cold().method_aliases.clone();
        let is_user_defined = self
            .class_data
            .get(class_name)
            .is_some_and(|d| d.user_defined);
        if !rbs_data.cold().includer_bound_dsl.is_empty()
            || !rbs_data.cold().pending_class_macros.is_empty()
        {
            self.has_includer_bound_dsl = true;
            self.includer_bound_dsl_applied = false;
        }
        for alias in method_aliases {
            self.record_method_alias(
                class_name,
                alias.new_name,
                alias.old_name,
                alias.is_singleton,
                alias.loc,
            );
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
            data.superclass_is_absolute = rbs_data.superclass_is_absolute;
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
        if !rbs_data.cold().class_type_param_modifiers.is_empty()
            && data.cold().class_type_param_modifiers.is_empty()
        {
            data.cold_mut().class_type_param_modifiers =
                rbs_data.cold().class_type_param_modifiers.clone();
        }
        for (key, flows) in &rbs_data.cold().param_flows {
            let cold = data.cold_mut();
            cold.param_flows.retain(|(existing, _)| existing != key);
            cold.param_flows.push((*key, flows.clone()));
        }
        for text in &rbs_data.cold().embedded_rbs {
            if !data.cold().embedded_rbs.contains(text) {
                data.cold_mut().embedded_rbs.push(text.clone());
            }
        }
        for template in &rbs_data.cold().define_method_macros {
            if !data.cold().define_method_macros.contains(template) {
                data.cold_mut().define_method_macros.push(template.clone());
            }
        }
        for (id, arg_types, result) in &rbs_data.cold().proc_literal_calls {
            if !data
                .cold()
                .proc_literal_calls
                .iter()
                .any(|(existing, _, _)| existing == id)
            {
                data.cold_mut()
                    .proc_literal_calls
                    .push((*id, arg_types.clone(), result.clone()));
            }
        }
        for name in &rbs_data.cold().scoped_annotation_names {
            if !data.cold().scoped_annotation_names.contains(name) {
                data.cold_mut().scoped_annotation_names.push(*name);
            }
        }
        for key in &rbs_data.cold().user_rbs_generic_methods {
            if !data.cold().user_rbs_generic_methods.contains(key) {
                data.cold_mut().user_rbs_generic_methods.push(*key);
            }
        }
        if !rbs_data.cold().class_type_param_defaults.is_empty()
            && data.cold().class_type_param_defaults.is_empty()
        {
            data.cold_mut().class_type_param_defaults =
                rbs_data.cold().class_type_param_defaults.clone();
        }
        if rbs_data.cold().struct_constructor_allows_missing_members {
            data.cold_mut().struct_constructor_allows_missing_members = true;
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
        if let Some(src_cold) = rbs_data.cold_opt() {
            if src_cold.has_dynamic_method_surface {
                data.cold_mut().has_dynamic_method_surface = true;
            }
            for entry in &src_cold.macro_forwarded_params {
                if !data.cold().macro_forwarded_params.contains(entry) {
                    data.cold_mut().macro_forwarded_params.push(entry.clone());
                }
            }
            for entry in &src_cold.pending_class_macros {
                if !data.cold().pending_class_macros.contains(entry) {
                    data.cold_mut().pending_class_macros.push(entry.clone());
                }
            }
            for name in &src_cold.runtime_attributes {
                if !data.cold().runtime_attributes.contains(name) {
                    data.cold_mut().runtime_attributes.push(*name);
                }
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
            if method.origin == MethodOrigin::SyntheticDsl
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
            // A method from source (`belongs_to :user`, `enum :status`, a `def`) replaces the
            // schema column or foreign-key reader it redefines. A source method whose return is
            // unknown (`def name = super || ""` before the column is visible) keeps the column
            // reader, which describes the same attribute with a type.
            let generated_slot =
                data.method_index
                    .get(method.name.as_str())
                    .and_then(|slots| slots.get(method.is_singleton))
                    .filter(|&idx| {
                        !method.origin.is_rbs_file_source()
                            && !matches!(method.raw_return_type, Type::Untyped)
                            && data.methods.get(idx).is_some_and(|existing| {
                                existing.origin == MethodOrigin::SyntheticDsl
                            })
                    });
            if let Some(idx) = generated_slot {
                data.methods[idx] = method;
            } else if !variant_exists {
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
            let incoming_external = constants_external || const_def.external_source;
            match data.constants.entry(*const_name) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let mut const_def = const_def.clone();
                    const_def.external_source = incoming_external;
                    entry.insert(const_def);
                }
                // A Ruby definition of a constant that only an `.rbs` file declared so far: emit it,
                // keeping the declared type.
                std::collections::hash_map::Entry::Occupied(mut entry)
                    if entry.get().external_source && !incoming_external =>
                {
                    let existing = entry.get_mut();
                    existing.loc = const_def.loc;
                    existing.file_path = const_def.file_path.clone();
                    existing.external_source = false;
                }
                std::collections::hash_map::Entry::Occupied(_) => {}
            }
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
            let key = (method.name, method.is_singleton);
            if method
                .rbs_method_types
                .iter()
                .any(|method_type| !method_type.type_params.is_empty())
                && !data.cold().user_rbs_generic_methods.contains(&key)
            {
                data.cold_mut().user_rbs_generic_methods.push(key);
            }
        }

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
        let external_origin = |origin: MethodOrigin| {
            if origin.is_sig() {
                MethodOrigin::Rbi
            } else {
                MethodOrigin::RbsFile
            }
        };
        for mut data in self.class_data.values_mut() {
            let unchanged = !data.user_defined
                && data
                    .methods
                    .iter()
                    .all(|method| method.origin == external_origin(method.origin));
            if unchanged {
                continue;
            }
            let data = data.write();
            data.user_defined = false;
            for method in &mut data.methods {
                let method = Arc::make_mut(method);
                method.origin = external_origin(method.origin);
            }
        }
        self.refresh_mixin_hook_method_flag();
    }

    pub fn remove_external_methods_for_class(&mut self, class_name: &str) -> bool {
        self.invalidate_owner_walks();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharing_a_project_class_matches_merging_it() {
        let mut declarations = TypeRegistry::new();
        crate::rbs::import::load_rbs_string(
            "module Named\n  def name: () -> String\nend\n\
             class Base\nend\n\
             class User < Base\n  include Named\n  VERSION: Integer\n  \
             def age: () -> Integer\n  def self.find: (Integer) -> User\n  alias years age\nend\n",
            &mut declarations,
        );

        // The pooled registry merges method by method; the plain one shares the kept class.
        let mut merged = TypeRegistry::new_pooled();
        let mut shared = TypeRegistry::new();
        let mut again = TypeRegistry::new();
        for registry in [&mut merged, &mut shared, &mut again] {
            assert!(registry.merge_rbs_class_from(&declarations, "User"));
        }

        let fingerprint =
            |registry: &TypeRegistry| content_fingerprint(&registry.class_data["User"]);
        assert_eq!(fingerprint(&shared), fingerprint(&merged));
        assert!(Arc::ptr_eq(
            &shared.class_data["User"],
            &again.class_data["User"]
        ));
    }
}
