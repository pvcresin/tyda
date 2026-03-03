use super::*;

impl<'a> InferenceEngine<'a> {
    fn class_needs_parameter_reference_resolution(data: &ClassData) -> bool {
        data.methods
            .iter()
            .any(|method| Self::method_needs_parameter_reference_resolution(method))
            || data
                .ivars
                .values()
                .flatten()
                .any(Self::type_contains_param_ref)
    }

    fn current_file_target_classes(&self) -> Option<HashSet<String>> {
        self.file_path.as_ref()?;
        let contributed = self.registry.file_contribution_names();
        if contributed.is_empty() {
            return None;
        }
        Some(self.expand_target_classes_with_ancestors(contributed.clone()))
    }

    fn is_stdlib_root_class(name: &str) -> bool {
        matches!(
            name,
            "Object" | "BasicObject" | "Kernel" | "Module" | "Class"
        )
    }

    fn expand_target_classes_with_ancestors(&self, mut names: HashSet<String>) -> HashSet<String> {
        let mut stack: Vec<String> = names.iter().cloned().collect();
        while let Some(name) = stack.pop() {
            if Self::is_stdlib_root_class(&name) {
                continue;
            }
            let Some(data) = self.registry.class_data_for(&name) else {
                continue;
            };
            let mut push = |related: String| {
                if names.insert(related.clone()) {
                    stack.push(related);
                }
            };
            if let Some(superclass) = data.superclass.as_ref() {
                push(superclass.to_string());
            }
            for mixin in &data.mixins {
                let mixin_name = mixin.module_name.to_string();
                let class_methods =
                    crate::sym::join_scope(mixin_name.trim_scope_prefix(), "ClassMethods");
                push(mixin_name);
                if self.registry.class_data_for(&class_methods).is_some() {
                    push(class_methods);
                }
            }
            if let Some(hooks) = data.hook_mixins() {
                for hook in hooks
                    .included
                    .iter()
                    .chain(&hooks.extended)
                    .chain(&hooks.prepended)
                {
                    push(hook.module_name.to_string());
                }
            }
            for ancestor in &data.cold().required_ancestors {
                push(ancestor.to_string());
            }
        }
        names
    }

    fn import_external_inbound_call_sites(&mut self) {
        let Some(external) = self.external_rbs else {
            return;
        };
        let class_names: Vec<String> = self
            .registry
            .file_contribution_names()
            .iter()
            .filter(|name| !Self::is_stdlib_root_class(name))
            .cloned()
            .collect();
        let inbound: Vec<(String, Vec<CallSite>)> = class_names
            .into_iter()
            .filter_map(|class_name| {
                let data = external.class_data_for(&class_name)?;
                if data.call_sites.is_empty() {
                    return None;
                }
                Some((
                    class_name,
                    data.call_sites.iter().cloned().collect::<Vec<_>>(),
                ))
            })
            .collect();
        for (class_name, sites) in inbound {
            for site in sites {
                self.registry.add_call_site(&class_name, site);
            }
        }
    }

    fn overlay_class_data(&self, class_name: &str) -> Option<&ClassData> {
        self.registry.class_data_for(class_name).or_else(|| {
            self.external_rbs
                .and_then(|external| external.class_data_for(class_name))
        })
    }

    fn overlay_resolve_call_owners(
        &self,
        class_name: &str,
        method_name: &str,
        method_is_singleton: bool,
        target_classes: Option<&HashSet<String>>,
    ) -> Vec<(String, bool)> {
        let mut owners =
            self.registry
                .resolve_method_call_owners(class_name, method_name, method_is_singleton);
        if owners.is_empty()
            && let Some(external) = self.external_rbs
        {
            owners =
                external.resolve_method_call_owners(class_name, method_name, method_is_singleton);
        }
        let Some(targets) = target_classes else {
            return owners;
        };
        let Some(data) = self.overlay_class_data(class_name) else {
            return owners;
        };
        let mut push_owner = |owner: String, owner_is_singleton: bool| {
            if !targets.contains(&owner) {
                return;
            }
            if !self
                .registry
                .has_method_variant(&owner, method_name, owner_is_singleton)
            {
                return;
            }
            if !owners.iter().any(|(existing, existing_singleton)| {
                existing == &owner && *existing_singleton == owner_is_singleton
            }) {
                owners.push((owner, owner_is_singleton));
            }
        };
        for mixin in &data.mixins {
            let mixin_name = mixin.module_name.to_string();
            match mixin.kind {
                crate::registry::MixinKind::Extend => {
                    if method_is_singleton {
                        push_owner(mixin_name.clone(), false);
                    }
                }
                crate::registry::MixinKind::Include | crate::registry::MixinKind::Prepend => {
                    push_owner(mixin_name.clone(), method_is_singleton);
                    if method_is_singleton {
                        push_owner(crate::sym::join_scope(&mixin_name, "ClassMethods"), false);
                    }
                }
            }
        }
        if let Some(superclass) = data.superclass.as_ref() {
            push_owner(superclass.to_string(), method_is_singleton);
        }
        owners
    }

    fn resolution_class_names(&self) -> Vec<Sym> {
        if let Some(target_classes) = self.current_file_target_classes() {
            target_classes.iter().map(Sym::new).collect()
        } else {
            self.registry.class_names_unsorted()
        }
    }

    pub fn needs_parameter_reference_resolution(&self) -> bool {
        if let Some(target_classes) = self.current_file_target_classes() {
            self.registry
                .iter_class_data()
                .filter(|(name, _)| target_classes.contains(name.as_str()))
                .any(|(_, data)| Self::class_needs_parameter_reference_resolution(data))
        } else {
            self.registry
                .iter_class_data()
                .any(|(_, data)| Self::class_needs_parameter_reference_resolution(data))
        }
    }

    pub fn resolve_parameter_references_from_calls(&mut self) {
        self.import_external_inbound_call_sites();
        let target_classes = self.current_file_target_classes();
        self.propagate_call_sites_to_object(target_classes.as_ref());

        let class_names = self.resolution_class_names();
        for class_name in &class_names {
            let Some(data) = self.registry.class_data_for(class_name) else {
                continue;
            };
            let grouped_call_sites = Self::group_call_sites_by_method(&data.call_sites);
            let mut all_param_types: Vec<Vec<Type>> = Vec::with_capacity(data.methods.len());
            for method in &data.methods {
                let empty: [&CallSite; 0] = [];
                let method_call_sites = grouped_call_sites
                    .get(&(method.name.as_str(), method.is_singleton))
                    .map(Vec::as_slice)
                    .unwrap_or(&empty);
                all_param_types.push(Self::resolve_param_types_from_grouped_call_sites(
                    &method.param_infos,
                    method_call_sites,
                ));
            }

            let init_updates = data
                .methods
                .iter()
                .position(|method| method.name == "initialize" && !method.is_singleton)
                .and_then(|init_idx| {
                    let init_param_types = all_param_types.get(init_idx)?;
                    init_param_types
                        .iter()
                        .any(|ty| !matches!(ty, Type::Untyped))
                        .then(|| {
                            (
                                data.ivars.keys().cloned().collect::<Vec<_>>(),
                                init_param_types.clone(),
                            )
                        })
                });

            let method_updates: Vec<(Sym, bool, Type)> = data
                .methods
                .iter()
                .enumerate()
                .filter_map(|(idx, method)| {
                    if !Self::method_needs_parameter_reference_resolution(method) {
                        return None;
                    }
                    let mut param_types_with_defaults = all_param_types[idx].clone();
                    let mut positional_index = 0;
                    for param in &method.param_infos {
                        if matches!(
                            param.kind,
                            ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                        ) {
                            if positional_index < param_types_with_defaults.len()
                                && let Some(ref default_ty) = param.default_type
                            {
                                let current = std::mem::replace(
                                    &mut param_types_with_defaults[positional_index],
                                    Type::Untyped,
                                );
                                param_types_with_defaults[positional_index] =
                                    if current == Type::Untyped {
                                        default_ty.clone()
                                    } else if matches!(param.kind, ParamKind::Optional) {
                                        current.union_with(default_ty.clone())
                                    } else {
                                        current
                                    };
                            }
                            positional_index += 1;
                        }
                    }
                    let param_types_with_defaults: Vec<Type> = param_types_with_defaults
                        .into_iter()
                        .map(Type::widen_arg_for_param)
                        .collect();
                    let keyword_types = Self::resolve_keyword_param_types_from_call_sites(
                        &method.param_infos,
                        grouped_call_sites
                            .get(&(method.name.as_str(), method.is_singleton))
                            .map(Vec::as_slice)
                            .unwrap_or(&[]),
                    );
                    let keyword_types: std::collections::HashMap<String, Type> = keyword_types
                        .into_iter()
                        .map(|(name, ty)| (name, ty.widen_arg_for_param()))
                        .collect();
                    if param_types_with_defaults
                        .iter()
                        .all(|ty| matches!(ty, Type::Untyped))
                        && keyword_types.values().all(|ty| matches!(ty, Type::Untyped))
                    {
                        return None;
                    }
                    let resolved = Self::substitute_param_refs_with_keywords(
                        &method.raw_return_type,
                        &param_types_with_defaults,
                        &keyword_types,
                    );
                    (resolved != method.raw_return_type).then_some((
                        method.name,
                        method.is_singleton,
                        resolved,
                    ))
                })
                .collect();

            if let Some((ivar_names, init_param_types)) = init_updates {
                for ivar_name in &ivar_names {
                    self.registry.resolve_ivar_param_refs(
                        class_name,
                        ivar_name.as_str(),
                        &init_param_types,
                    );
                }
            }
            for (method_name, is_singleton, resolved) in method_updates {
                self.registry.update_method_return_type_variant(
                    class_name,
                    &method_name,
                    is_singleton,
                    resolved,
                );
            }
        }
    }

    pub fn resolve_method_return_refs(&mut self) {
        self.merge_external_rbs_for_resolution();
        const MAX_ITERATIONS: usize = 8;
        let class_names: Vec<Sym> = if let Some(ref fp) = self.file_path {
            self.registry
                .user_defined_class_names_unsorted()
                .into_iter()
                .filter(|name| {
                    self.registry
                        .class_data_for(name)
                        .and_then(|d| d.file_path.as_deref())
                        == Some(fp.as_ref())
                })
                .collect()
        } else {
            self.registry.user_defined_class_names_unsorted()
        };
        if class_names.is_empty() {
            return;
        }
        for _ in 0..MAX_ITERATIONS {
            let mut changed = false;
            for class_name in &class_names {
                let Some(data) = self.registry.class_data_for(class_name) else {
                    continue;
                };
                let updates: Vec<(Sym, bool, Type)> = data
                    .methods
                    .iter()
                    .filter_map(|method| {
                        if !Self::type_contains_method_ref(&method.raw_return_type) {
                            return None;
                        }
                        let mut visiting = HashSet::new();
                        visiting.insert((class_name.to_string(), Sym::new(method.name)));
                        let resolved = Self::resolve_general_method_refs(
                            &self.registry,
                            class_name,
                            &method.raw_return_type,
                            &mut visiting,
                        );
                        (resolved != method.raw_return_type).then_some((
                            method.name,
                            method.is_singleton,
                            resolved,
                        ))
                    })
                    .collect();
                if !updates.is_empty() {
                    changed = true;
                }
                for (method_name, is_singleton, resolved) in updates {
                    self.registry.update_method_return_type_variant(
                        class_name,
                        &method_name,
                        is_singleton,
                        resolved,
                    );
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn resolve_general_method_refs(
        registry: &TypeRegistry,
        context_class: &str,
        ty: &Type,
        visiting: &mut HashSet<(String, Sym)>,
    ) -> Type {
        Self::resolve_general_method_refs_depth(registry, context_class, ty, visiting, 0)
    }

    fn resolve_general_method_refs_depth(
        registry: &TypeRegistry,
        context_class: &str,
        ty: &Type,
        visiting: &mut HashSet<(String, Sym)>,
        depth: usize,
    ) -> Type {
        if depth >= 12 {
            return ty.clone();
        }
        match ty {
            Type::IvarRef(ivar_name) => {
                let key = (context_class.to_string(), *ivar_name);
                if visiting.contains(&key) {
                    return ty.clone();
                }
                visiting.insert(key.clone());
                let ivar_type = registry.lookup_ivar_type(context_class, ivar_name);
                let result = match ivar_type {
                    Some(Type::Untyped)
                    | None
                    | Some(Type::ParamRef(_))
                    | Some(Type::KeywordParamRef(_)) => registry
                        .infer_attr_type_from_initialize(context_class, ivar_name)
                        .unwrap_or_else(|| ty.clone()),
                    Some(resolved) if Self::is_concrete_for_method_ref_resolve(&resolved) => {
                        resolved
                    }
                    Some(resolved) => Self::resolve_general_method_refs_depth(
                        registry,
                        context_class,
                        &resolved,
                        visiting,
                        depth + 1,
                    ),
                };
                visiting.remove(&key);
                result
            }
            Type::MethodReturnRef(class_name, method_name) => {
                if visiting.contains(&(class_name.to_string(), *method_name)) {
                    return ty.clone();
                }
                let ret = registry
                    .lookup_method_return_type_direct(class_name, method_name)
                    .or_else(|| registry.lookup_method_return_type(class_name, method_name));
                if let Some(ret) = ret
                    && Self::is_concrete_for_method_ref_resolve(&ret)
                {
                    visiting.insert((class_name.to_string(), *method_name));
                    let resolved = Self::resolve_general_method_refs_depth(
                        registry,
                        class_name,
                        &ret,
                        visiting,
                        depth + 1,
                    );
                    visiting.remove(&(class_name.to_string(), *method_name));
                    return resolved;
                }
                ty.clone()
            }
            Type::ReceiverMethodRef(receiver_type, method_name) => {
                let resolved_receiver = Self::resolve_general_method_refs_depth(
                    registry,
                    context_class,
                    receiver_type,
                    visiting,
                    depth + 1,
                );
                if let Some(receiver_class) = Self::static_type_to_class_name(&resolved_receiver) {
                    let ret = registry
                        .lookup_method_return_type_direct(&receiver_class, method_name)
                        .or_else(|| {
                            registry.lookup_method_return_type(&receiver_class, method_name)
                        });
                    if let Some(ret) = ret
                        && Self::is_concrete_for_method_ref_resolve(&ret)
                    {
                        let resolved = Self::resolve_general_method_refs_depth(
                            registry,
                            &receiver_class,
                            &ret,
                            visiting,
                            depth + 1,
                        );
                        return resolved;
                    }
                }
                Type::ReceiverMethodRef(Box::new(resolved_receiver), *method_name)
            }
            Type::Union(parts) => Type::from_type_vec(
                parts
                    .iter()
                    .map(|t| {
                        Self::resolve_general_method_refs_depth(
                            registry,
                            context_class,
                            t,
                            visiting,
                            depth + 1,
                        )
                    })
                    .collect(),
            ),
            Type::Array(Some(inner)) => {
                Type::Array(Some(Box::new(Self::resolve_general_method_refs_depth(
                    registry,
                    context_class,
                    inner,
                    visiting,
                    depth + 1,
                ))))
            }
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|t| {
                        Self::resolve_general_method_refs_depth(
                            registry,
                            context_class,
                            t,
                            visiting,
                            depth + 1,
                        )
                    })
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    fn type_contains_method_ref(ty: &Type) -> bool {
        match ty {
            Type::MethodReturnRef(..) | Type::ReceiverMethodRef(..) | Type::IvarRef(..) => true,
            Type::Union(parts) | Type::Intersection(parts) => {
                parts.iter().any(Self::type_contains_method_ref)
            }
            Type::Array(Some(inner)) => Self::type_contains_method_ref(inner),
            Type::Tuple(elems) => elems.iter().any(Self::type_contains_method_ref),
            Type::Hash(Some(k), Some(v)) => {
                Self::type_contains_method_ref(k) || Self::type_contains_method_ref(v)
            }
            _ => false,
        }
    }

    fn is_concrete_for_method_ref_resolve(ty: &Type) -> bool {
        match ty {
            Type::Integer
            | Type::Float
            | Type::String
            | Type::Symbol
            | Type::Bool
            | Type::True
            | Type::False
            | Type::Nil
            | Type::Void
            | Type::Top
            | Type::Bot
            | Type::LiteralInteger(_)
            | Type::LiteralFloat(_)
            | Type::LiteralString(_)
            | Type::LiteralSymbol(_)
            | Type::Class(_)
            | Type::Singleton(_)
            | Type::SelfType
            | Type::InstanceType => true,
            Type::Generic { .. } => true,
            Type::Array(None) | Type::Hash(None, None) => true,
            Type::Array(Some(inner)) => Self::is_concrete_for_method_ref_resolve(inner),
            Type::Hash(Some(k), Some(v)) => {
                Self::is_concrete_for_method_ref_resolve(k)
                    && Self::is_concrete_for_method_ref_resolve(v)
            }
            Type::Hash(Some(k), None) => Self::is_concrete_for_method_ref_resolve(k),
            Type::Hash(None, Some(v)) => Self::is_concrete_for_method_ref_resolve(v),
            Type::Union(parts) => parts.iter().all(Self::is_concrete_for_method_ref_resolve),
            Type::Intersection(parts) => parts.iter().all(Self::is_concrete_for_method_ref_resolve),
            Type::Tuple(elems) => elems.iter().all(Self::is_concrete_for_method_ref_resolve),
            Type::Record(fields) => fields
                .iter()
                .all(|f| Self::is_concrete_for_method_ref_resolve(&f.value)),
            Type::Proc {
                return_type,
                param_count: _,
            } => Self::is_concrete_for_method_ref_resolve(return_type),
            _ => false,
        }
    }

    fn merge_external_rbs_for_resolution(&mut self) {
        let Some(ext) = self.external_rbs else {
            return;
        };
        let refs = self.collect_referenced_class_names();
        for class_name in refs {
            self.registry.merge_rbs_class_from(ext, &class_name);
        }
        self.registry.build_subclass_index();
    }

    fn collect_referenced_class_names(&self) -> Vec<String> {
        let mut names = std::collections::HashSet::new();
        let class_names = self.registry.user_defined_class_names_unsorted();
        for class_name in &class_names {
            if let Some(data) = self.registry.class_data_for(class_name) {
                if let Some(ref sc) = data.superclass {
                    names.insert(sc.to_string());
                }
                for mixin in &data.mixins {
                    names.insert(mixin.module_name.to_string());
                }
            }
            let methods = self.registry.get_methods(class_name);
            for method in methods {
                Self::collect_class_names_from_type(&method.raw_return_type, &mut names);
            }
        }
        names.into_iter().collect()
    }

    fn collect_class_names_from_type(ty: &Type, names: &mut std::collections::HashSet<String>) {
        match ty {
            Type::Class(name) | Type::Singleton(name) => {
                names.insert((name.clone()).to_string());
            }
            Type::MethodReturnRef(class_name, _) => {
                names.insert((class_name.clone()).to_string());
            }
            Type::ReceiverMethodRef(inner, _) => {
                Self::collect_class_names_from_type(inner, names);
            }
            Type::Union(parts) | Type::Intersection(parts) => {
                for part in parts {
                    Self::collect_class_names_from_type(part, names);
                }
            }
            Type::Array(Some(inner)) => {
                Self::collect_class_names_from_type(inner, names);
            }
            Type::Tuple(elems) => {
                for elem in elems {
                    Self::collect_class_names_from_type(elem, names);
                }
            }
            _ => {}
        }
    }

    pub fn resolve_subclass_method_refs(&mut self) {
        let class_names = self.resolution_class_names();
        for class_name in &class_names {
            let Some(data) = self.registry.class_data_for(class_name) else {
                continue;
            };
            let updates: Vec<(Sym, bool, Type)> = data
                .methods
                .iter()
                .filter_map(|method| {
                    if !Self::type_contains_subclass_ref_candidate(&method.raw_return_type) {
                        return None;
                    }
                    let resolved = Self::resolve_subclass_refs_in_type(
                        &self.registry,
                        &method.raw_return_type,
                    );
                    (resolved != method.raw_return_type).then_some((
                        method.name,
                        method.is_singleton,
                        resolved,
                    ))
                })
                .collect();
            for (method_name, is_singleton, resolved) in updates {
                self.registry.update_method_return_type_variant(
                    class_name,
                    &method_name,
                    is_singleton,
                    resolved,
                );
            }
        }
    }

    fn type_contains_subclass_ref_candidate(ty: &Type) -> bool {
        match ty {
            Type::MethodReturnRef(..) | Type::ReceiverMethodRef(..) => true,
            Type::Union(parts) | Type::Intersection(parts) => {
                parts.iter().any(Self::type_contains_subclass_ref_candidate)
            }
            Type::Array(Some(inner))
            | Type::PatternRestRef(inner)
            | Type::PatternIndexRef(inner, _)
            | Type::PatternKeyRef(inner, _)
            | Type::PatternKeyRestRef(inner, _) => {
                Self::type_contains_subclass_ref_candidate(inner)
            }
            Type::Tuple(elems) => elems.iter().any(Self::type_contains_subclass_ref_candidate),
            Type::Hash(Some(key), Some(value)) => {
                Self::type_contains_subclass_ref_candidate(key)
                    || Self::type_contains_subclass_ref_candidate(value)
            }
            Type::Hash(Some(key), None) => Self::type_contains_subclass_ref_candidate(key),
            Type::Hash(None, Some(value)) => Self::type_contains_subclass_ref_candidate(value),
            Type::Record(fields) => fields
                .iter()
                .any(|field| Self::type_contains_subclass_ref_candidate(&field.value)),
            Type::Proc { return_type, .. } => {
                Self::type_contains_subclass_ref_candidate(return_type)
            }
            _ => false,
        }
    }

    fn resolve_subclass_refs_in_type(registry: &TypeRegistry, ty: &Type) -> Type {
        match ty {
            Type::MethodReturnRef(class_name, method_name) => {
                if registry
                    .resolve_method_call_owners(class_name, method_name, false)
                    .is_empty()
                    && registry
                        .resolve_method_call_owners(class_name, method_name, true)
                        .is_empty()
                    && let Some(ret) =
                        registry.lookup_method_return_type_in_subclasses(class_name, method_name)
                    && !matches!(ret, Type::MethodReturnRef(..))
                    && !Self::type_contains_param_ref(&ret)
                {
                    return ret;
                }
                ty.clone()
            }
            Type::ReceiverMethodRef(receiver_type, method_name) => {
                let resolved_receiver =
                    Self::resolve_subclass_refs_in_type(registry, receiver_type);
                if let Some(receiver_class) = Self::static_type_to_class_name(&resolved_receiver)
                    && let Some(ret) =
                        registry.lookup_method_return_type(&receiver_class, method_name)
                    && !matches!(ret, Type::MethodReturnRef(..) | Type::ReceiverMethodRef(..))
                    && !Self::type_contains_param_ref(&ret)
                {
                    return ret;
                }
                Type::ReceiverMethodRef(Box::new(resolved_receiver), *method_name)
            }
            Type::Union(parts) => Type::from_type_vec(
                parts
                    .iter()
                    .map(|t| Self::resolve_subclass_refs_in_type(registry, t))
                    .collect(),
            ),
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(
                Self::resolve_subclass_refs_in_type(registry, inner),
            ))),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|t| Self::resolve_subclass_refs_in_type(registry, t))
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    fn method_needs_parameter_reference_resolution(method: &MethodDef) -> bool {
        Self::type_contains_param_ref(&method.raw_return_type)
    }

    fn type_contains_param_ref(ty: &Type) -> bool {
        match ty {
            Type::ParamRef(_) | Type::KeywordParamRef(_) => true,
            Type::Union(parts) | Type::Intersection(parts) => {
                parts.iter().any(Self::type_contains_param_ref)
            }
            Type::Array(Some(inner)) => Self::type_contains_param_ref(inner),
            Type::Hash(Some(key), Some(value)) => {
                Self::type_contains_param_ref(key) || Self::type_contains_param_ref(value)
            }
            Type::Hash(Some(key), None) => Self::type_contains_param_ref(key),
            Type::Hash(None, Some(value)) => Self::type_contains_param_ref(value),
            Type::Record(fields) => fields
                .iter()
                .any(|field| Self::type_contains_param_ref(&field.value)),
            Type::PatternIndexRef(subject, _) | Type::PatternRestRef(subject) => {
                Self::type_contains_param_ref(subject)
            }
            Type::PatternKeyRef(subject, _) | Type::PatternKeyRestRef(subject, _) => {
                Self::type_contains_param_ref(subject)
            }
            Type::ReceiverMethodRef(receiver_type, _) => {
                Self::type_contains_param_ref(receiver_type)
            }
            Type::Proc { return_type, .. } => Self::type_contains_param_ref(return_type),
            Type::Tuple(elems) => elems.iter().any(Self::type_contains_param_ref),
            _ => false,
        }
    }

    pub(crate) fn collect_top_level_hover(
        &mut self,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &mut Scope,
    ) {
        match node {
            Node::ClassNode { .. } => {
                let class_node = node.as_class_node().expect("must be ClassNode");
                let class_name =
                    self.resolve_constant_path(&class_node.constant_path(), parse_result);
                if class_name != "Unknown"
                    && let Some(body) = class_node.body()
                {
                    self.collect_class_body_hover(&class_name, &body, parse_result);
                }
            }
            Node::ModuleNode { .. } => {
                let module_node = node.as_module_node().expect("must be ModuleNode");
                let module_name =
                    self.resolve_constant_path(&module_node.constant_path(), parse_result);
                if module_name != "Unknown"
                    && let Some(body) = module_node.body()
                {
                    self.collect_class_body_hover(&module_name, &body, parse_result);
                }
            }
            Node::DefNode { .. } => {}
            _ => {
                let ty = self.infer_node_type("Object", node, parse_result, scope);
                match node {
                    Node::LocalVariableWriteNode { .. } => {
                        let write_node = node
                            .as_local_variable_write_node()
                            .expect("must be LocalVariableWriteNode");
                        let var_name =
                            String::from_utf8_lossy(write_node.name().as_slice()).to_string();
                        scope.set(&var_name, ty);
                    }
                    Node::LocalVariableOrWriteNode { .. } => {
                        let write_node = node
                            .as_local_variable_or_write_node()
                            .expect("must be LocalVariableOrWriteNode");
                        let var_name =
                            String::from_utf8_lossy(write_node.name().as_slice()).to_string();
                        scope.set(&var_name, ty);
                    }
                    Node::LocalVariableAndWriteNode { .. } => {
                        let write_node = node
                            .as_local_variable_and_write_node()
                            .expect("must be LocalVariableAndWriteNode");
                        let var_name =
                            String::from_utf8_lossy(write_node.name().as_slice()).to_string();
                        scope.set(&var_name, ty);
                    }
                    Node::LocalVariableOperatorWriteNode { .. } => {
                        let write_node = node
                            .as_local_variable_operator_write_node()
                            .expect("must be LocalVariableOperatorWriteNode");
                        let var_name =
                            String::from_utf8_lossy(write_node.name().as_slice()).to_string();
                        scope.set(&var_name, ty);
                    }
                    _ => {}
                }
            }
        }
    }

    fn collect_class_body_hover(
        &mut self,
        class_name: &str,
        body: &Node<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        let Some(statements) = body.as_statements_node() else {
            return;
        };
        let scope = Scope::default();
        for node in statements.body().iter() {
            match &node {
                Node::ClassNode { .. } | Node::ModuleNode { .. } => {
                    continue;
                }
                Node::DefNode { .. } => {
                    let def_node = node.as_def_node().expect("must be DefNode");
                    let method_name =
                        String::from_utf8_lossy(def_node.name().as_slice()).to_string();
                    let is_singleton = def_node
                        .receiver()
                        .is_some_and(|r| matches!(r, Node::SelfNode { .. }));
                    let (param_names, _) =
                        self.collect_param_info(&def_node, class_name, parse_result);
                    if let Some(body) = def_node.body() {
                        let mut method_scope = Scope {
                            method_name: Some(method_name),
                            singleton_dispatch: is_singleton,
                            ..Default::default()
                        };
                        for (i, name) in param_names.iter().enumerate() {
                            if !name.is_empty() {
                                method_scope.set(name, Type::ParamRef(i));
                            }
                        }
                        self.collect_static_method_name_definition_literals(
                            class_name,
                            &body,
                            parse_result,
                            &method_scope,
                        );
                    }
                    continue;
                }
                _ => {}
            }
            // An association's scope lambda (`has_many :x, ->{ scope }, class_name: 'Y'`) is instance_exec'd by Rails on the target class's relation, so bare calls in the block resolve against the target class, not the declaring self. We infer with self switched to that class so association scopes / class methods resolve correctly (an unresolved owner is suppressed by the diagnostic gate).
            if self.infer_association_scope_hover(class_name, &node, parse_result, &scope) {
                continue;
            }
            let _ = self.infer_node_type(class_name, &node, parse_result, &scope);
        }
    }

    fn infer_association_scope_hover(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
    ) -> bool {
        if !self.dsl_enabled(DslLibrary::ActiveRecordAssociations) {
            return false;
        }
        let Node::CallNode { .. } = node else {
            return false;
        };
        let call_node = node.as_call_node().expect("must be CallNode");
        if call_node.receiver().is_some() {
            return false;
        }
        let method_name = String::from_utf8_lossy(call_node.name().as_slice());
        if !matches!(
            method_name.as_ref(),
            "has_many" | "has_one" | "has_and_belongs_to_many" | "belongs_to"
        ) {
            return false;
        }
        let Some(args) = call_node.arguments() else {
            return false;
        };
        if !args
            .arguments()
            .iter()
            .any(|arg| matches!(arg, Node::LambdaNode { .. }))
        {
            return false;
        }
        let names = Self::extract_symbol_args(&call_node);
        let Some(assoc_name) = names.first() else {
            return false;
        };
        let mut options = Self::extract_association_options(&call_node, parse_result);
        self.apply_with_options_fallback(&mut options);
        let collection = method_name != "has_one" && method_name != "belongs_to";
        // Switch self to the target class's class object, using its FQN with the leading `::` stripped.
        // If the target class is unresolved, the owner is unknown and the diagnostic gate suppresses it.
        let target = self
            .infer_association_target_class(class_name, assoc_name, &options, collection)
            .trim_scope_prefix()
            .to_string();
        let lambda_scope = Scope {
            self_override: Some(Type::Singleton(Sym::new(target))),
            singleton_dispatch: true,
            ..Default::default()
        };
        for arg in args.arguments().iter() {
            if matches!(arg, Node::LambdaNode { .. }) {
                let _ = self.infer_node_type(class_name, &arg, parse_result, &lambda_scope);
            } else {
                let _ = self.infer_node_type(class_name, &arg, parse_result, scope);
            }
        }
        true
    }

    fn collect_static_method_name_definition_literals(
        &mut self,
        class_name: &str,
        node: &Node<'_>,
        parse_result: &ParseResult<'_>,
        scope: &Scope,
    ) {
        match node {
            Node::StatementsNode { .. } => {
                let statements = node.as_statements_node().expect("must be StatementsNode");
                for child in statements.body().iter() {
                    self.collect_static_method_name_definition_literals(
                        class_name,
                        &child,
                        parse_result,
                        scope,
                    );
                }
            }
            Node::CallNode { .. } => {
                let call_node = node.as_call_node().expect("must be CallNode");
                let method_name = String::from_utf8_lossy(call_node.name().as_slice());
                if matches!(
                    method_name.as_ref(),
                    "send" | "public_send" | "__send__" | "try" | "try!"
                ) && let Some(args) = call_node.arguments()
                {
                    let arg_list: Vec<_> = args.arguments().iter().collect();
                    if let Some(first_arg) = arg_list.first()
                        && let Some(target_method) = Self::extract_symbol_literal_name(first_arg)
                    {
                        let receiver_type = if let Some(receiver) = call_node.receiver() {
                            self.infer_node_type(class_name, &receiver, parse_result, scope)
                        } else {
                            scope.current_self_type(class_name)
                        };
                        self.push_method_name_argument_definition_snapshot(
                            parse_result.source(),
                            first_arg,
                            receiver_type,
                            &target_method,
                        );
                    }
                }
                if let Some(block) = call_node.block()
                    && let Some(block_node) = block.as_block_node()
                    && let Some(body) = block_node.body()
                {
                    self.collect_static_method_name_definition_literals(
                        class_name,
                        &body,
                        parse_result,
                        scope,
                    );
                }
            }
            _ => {}
        }
    }

    pub fn preload_receiver_reference_types(&mut self) {
        if self.lazy_loader.is_none() && self.lazy_rbi_loader.is_none() {
            let mut pending: Vec<String> = self
                .registry
                .class_names_unsorted()
                .into_iter()
                .map(|name| name.to_string())
                .collect();
            let mut seen = HashSet::new();

            while let Some(class_name) = pending.pop() {
                if !seen.insert(class_name.clone()) {
                    continue;
                }

                self.ensure_class_available(&class_name);
                let Some(data) = self.registry.class_data_for(&class_name) else {
                    continue;
                };

                if let Some(superclass) = &data.superclass {
                    pending.push(superclass.to_string());
                }
                for mixin in &data.mixins {
                    pending.push(mixin.module_name.to_string());
                }
                for method in &data.methods {
                    self.collect_receiver_reference_classes(&method.raw_return_type, &mut pending);
                }
            }
            return;
        }

        let mut pending = Vec::new();
        let mut seen = HashSet::new();

        for class_name in self.registry.user_defined_class_names_unsorted() {
            let Some(data) = self.registry.class_data_for(&class_name) else {
                continue;
            };
            for method in &data.methods {
                if !method.rbs_file_source {
                    self.collect_receiver_reference_classes(&method.raw_return_type, &mut pending);
                }
            }
        }

        while let Some(class_name) = pending.pop() {
            if seen.insert(class_name.clone()) {
                self.ensure_class_available(&class_name);
            }
        }
    }

    fn collect_receiver_reference_classes(&self, ty: &Type, classes: &mut Vec<String>) {
        match ty {
            Type::MethodReturnRef(class_name, method_name) => {
                if self.registry.has_method_named(class_name, method_name) {
                    return;
                }
                if Self::should_preload_method_return_owner(class_name) {
                    classes.push((class_name.clone()).to_string());
                } else {
                    classes.push("Object".to_string());
                }
            }
            _ => Self::collect_receiver_classes(ty, classes),
        }
    }

    fn should_preload_method_return_owner(class_name: &str) -> bool {
        matches!(
            class_name,
            "Object"
                | "BasicObject"
                | "Kernel"
                | "String"
                | "Array"
                | "Hash"
                | "Integer"
                | "Float"
                | "Symbol"
                | "Range"
                | "Enumerator"
                | "Enumerable"
                | "Proc"
                | "Class"
                | "Module"
                | "NilClass"
                | "TrueClass"
                | "FalseClass"
        )
    }

    pub(super) fn collect_receiver_classes(ty: &Type, classes: &mut Vec<String>) {
        match ty {
            Type::ReceiverMethodRef(receiver_type, _) => {
                if let Some(cls) = Self::static_type_to_class_name(receiver_type) {
                    classes.push(cls);
                }
                Self::collect_receiver_classes(receiver_type, classes);
            }
            Type::Union(parts) => {
                for t in parts {
                    Self::collect_receiver_classes(t, classes);
                }
            }
            Type::Array(Some(inner)) => Self::collect_receiver_classes(inner, classes),
            Type::Tuple(elems) => {
                for t in elems {
                    Self::collect_receiver_classes(t, classes);
                }
            }
            Type::Hash(Some(k), Some(v)) => {
                Self::collect_receiver_classes(k, classes);
                Self::collect_receiver_classes(v, classes);
            }
            _ => {}
        }
    }

    pub(super) fn static_type_to_class_name(ty: &Type) -> Option<String> {
        match ty {
            Type::Integer | Type::LiteralInteger(_) => Some("Integer".to_string()),
            Type::Float | Type::LiteralFloat(_) => Some("Float".to_string()),
            Type::String | Type::LiteralString(_) => Some("String".to_string()),
            Type::Symbol | Type::LiteralSymbol(_) => Some("Symbol".to_string()),
            Type::Bool => Some("bool".to_string()),
            Type::True => Some("TrueClass".to_string()),
            Type::False => Some("FalseClass".to_string()),
            Type::Array(_) | Type::Tuple(_) => Some("Array".to_string()),
            Type::Hash(_, _) | Type::Record(_) => Some("Hash".to_string()),
            Type::Class(name) => Some((name.clone()).to_string()),
            _ => None,
        }
    }

    pub(super) fn propagate_call_sites_to_object(
        &mut self,
        target_classes: Option<&HashSet<String>>,
    ) {
        let include_object = target_classes.is_none_or(|targets| targets.contains("Object"));
        let target_method_names: Option<HashSet<Sym>> = target_classes.map(|targets| {
            let mut names = HashSet::new();
            for class_name in targets {
                if Self::is_stdlib_root_class(class_name) {
                    continue;
                }
                let Some(data) = self.registry.class_data_for(class_name) else {
                    continue;
                };
                for (method_name, slots) in data.method_index.iter() {
                    if slots.instance.is_some() || slots.singleton.is_some() {
                        names.insert(method_name);
                    }
                }
            }
            names.extend(
                self.registry
                    .file_contribution_method_names()
                    .iter()
                    .copied(),
            );
            names
        });
        if target_method_names
            .as_ref()
            .is_some_and(|names| names.is_empty())
            && !include_object
        {
            return;
        }

        let class_names = self.registry.class_names();
        let mut propagated_to_owners: Vec<(String, CallSite)> = Vec::new();
        let mut propagated_to_object: Vec<CallSite> = Vec::new();
        let mut mro_cache: rustc_hash::FxHashMap<
            (String, crate::types::SharedName, bool),
            Vec<(String, bool)>,
        > = rustc_hash::FxHashMap::default();

        for class_name in &class_names {
            if class_name == "Object" {
                continue;
            }
            let call_sites = self.registry.get_call_sites(class_name);

            for site in call_sites {
                if site.method_name.as_ref() == "initialize" {
                    continue;
                }
                if let Some(target_method_names) = target_method_names.as_ref()
                    && !target_method_names.contains(&Sym::new(site.method_name.as_ref()))
                {
                    continue;
                }
                if !self.registry.has_method_variant(
                    class_name,
                    site.method_name.as_ref(),
                    site.method_is_singleton,
                ) {
                    let cache_key = (
                        class_name.clone(),
                        site.method_name.clone(),
                        site.method_is_singleton,
                    );
                    let owners = mro_cache
                        .entry(cache_key)
                        .or_insert_with(|| {
                            self.overlay_resolve_call_owners(
                                class_name,
                                site.method_name.as_ref(),
                                site.method_is_singleton,
                                target_classes,
                            )
                        })
                        .clone();
                    if owners.is_empty() {
                        if include_object && !site.method_is_singleton {
                            propagated_to_object.push(site.clone());
                        }
                    } else {
                        for (owner, owner_method_is_singleton) in owners {
                            if let Some(targets) = target_classes
                                && !targets.contains(&owner)
                            {
                                continue;
                            }
                            if owner != *class_name
                                || owner_method_is_singleton != site.method_is_singleton
                            {
                                let mut propagated = site.clone();
                                propagated.method_is_singleton = owner_method_is_singleton;
                                propagated_to_owners.push((owner, propagated));
                            }
                        }
                    }
                }
            }
        }

        let external_callers: Vec<(String, Vec<CallSite>)> = self
            .external_rbs
            .map(|external| {
                external
                    .class_names_unsorted()
                    .into_iter()
                    .filter_map(|class_name| {
                        if class_name.as_str() == "Object" || self.registry.has_class(&class_name) {
                            return None;
                        }
                        let sites: Vec<CallSite> = external
                            .get_call_sites(&class_name)
                            .iter()
                            .filter(|site| {
                                site.method_name.as_ref() != "initialize"
                                    && target_method_names.as_ref().is_none_or(|names| {
                                        names.contains(&Sym::new(site.method_name.as_ref()))
                                    })
                            })
                            .cloned()
                            .collect();
                        (!sites.is_empty()).then_some((class_name.to_string(), sites))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (class_name, sites) in external_callers {
            for site in sites {
                if self.registry.has_method_variant(
                    &class_name,
                    site.method_name.as_ref(),
                    site.method_is_singleton,
                ) {
                    continue;
                }
                let cache_key = (
                    class_name.clone(),
                    site.method_name.clone(),
                    site.method_is_singleton,
                );
                let owners = mro_cache
                    .entry(cache_key)
                    .or_insert_with(|| {
                        self.overlay_resolve_call_owners(
                            &class_name,
                            site.method_name.as_ref(),
                            site.method_is_singleton,
                            target_classes,
                        )
                    })
                    .clone();
                if owners.is_empty() {
                    if include_object && !site.method_is_singleton {
                        propagated_to_object.push(site);
                    }
                } else {
                    for (owner, owner_method_is_singleton) in owners {
                        if let Some(targets) = target_classes
                            && !targets.contains(&owner)
                        {
                            continue;
                        }
                        if owner != class_name
                            || owner_method_is_singleton != site.method_is_singleton
                        {
                            let mut propagated = site.clone();
                            propagated.method_is_singleton = owner_method_is_singleton;
                            propagated_to_owners.push((owner, propagated));
                        }
                    }
                }
            }
        }

        for (owner, site) in propagated_to_owners {
            self.registry.add_call_site(&owner, site);
        }
        for site in propagated_to_object {
            self.registry.add_call_site("Object", site);
        }
    }

    fn group_call_sites_by_method(
        call_sites: &crate::registry::CallSiteStore,
    ) -> std::collections::HashMap<(&str, bool), Vec<&CallSite>> {
        let mut grouped = std::collections::HashMap::new();
        for call_site in call_sites {
            grouped
                .entry((
                    call_site.method_name.as_ref(),
                    call_site.method_is_singleton,
                ))
                .or_insert_with(Vec::new)
                .push(call_site);
        }
        grouped
    }

    fn resolve_keyword_param_types_from_call_sites(
        param_infos: &[ParamInfo],
        call_sites: &[&CallSite],
    ) -> std::collections::HashMap<String, Type> {
        let mut kw_types: std::collections::HashMap<String, Vec<Type>> =
            std::collections::HashMap::new();
        let kw_params: Vec<(&str, Option<&Type>)> = param_infos
            .iter()
            .filter(|pi| {
                matches!(
                    pi.kind,
                    ParamKind::KeywordRequired | ParamKind::KeywordOptional
                )
            })
            .map(|pi| (pi.name.as_str(), pi.default_type.as_ref()))
            .collect();
        if kw_params.is_empty() {
            return std::collections::HashMap::new();
        }
        for site in call_sites {
            for &(kw_name, _) in &kw_params {
                if let Some(ty) = site.keyword_arg_types.get(kw_name) {
                    kw_types
                        .entry(kw_name.to_string())
                        .or_default()
                        .push(ty.clone());
                }
            }
        }
        let mut result = std::collections::HashMap::new();
        for (kw_name, default_type) in kw_params {
            if let Some(types) = kw_types.remove(kw_name) {
                let mut merged = types;
                if let Some(dt) = default_type {
                    merged.push(dt.clone());
                }
                result.insert(kw_name.to_string(), Type::from_type_vec(merged));
            } else if let Some(dt) = default_type {
                result.insert(kw_name.to_string(), dt.clone());
            }
        }
        result
    }

    pub(super) fn resolve_param_types_from_grouped_call_sites(
        param_infos: &[ParamInfo],
        call_sites: &[&CallSite],
    ) -> Vec<Type> {
        let param_count = param_infos
            .iter()
            .filter(|pi| {
                matches!(
                    pi.kind,
                    ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                )
            })
            .count();
        let mut param_types: Vec<Vec<Type>> = vec![Vec::new(); param_count];
        for site in call_sites {
            TypeRegistry::merge_call_site_positional_types(&mut param_types, site, param_infos);
        }

        param_types
            .into_iter()
            .map(Type::merge_param_arg_vec)
            .collect()
    }

    pub(super) fn substitute_param_refs(ty: &Type, param_types: &[Type]) -> Type {
        Self::substitute_param_refs_with_keywords(
            ty,
            param_types,
            &std::collections::HashMap::new(),
        )
    }

    pub(super) fn substitute_param_refs_with_keywords(
        ty: &Type,
        param_types: &[Type],
        keyword_types: &std::collections::HashMap<String, Type>,
    ) -> Type {
        match ty {
            Type::ParamRef(idx) => param_types.get(*idx).cloned().unwrap_or(Type::Untyped),
            Type::KeywordParamRef(name) => keyword_types
                .get(name.as_str())
                .cloned()
                .unwrap_or(Type::Untyped),
            Type::Union(parts) => {
                let resolved: Vec<Type> = parts
                    .iter()
                    .map(|t| {
                        Self::substitute_param_refs_with_keywords(t, param_types, keyword_types)
                    })
                    .collect();
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(
                Self::substitute_param_refs_with_keywords(inner, param_types, keyword_types),
            ))),
            Type::Hash(Some(k), Some(v)) => Type::Hash(
                Some(Box::new(Self::substitute_param_refs_with_keywords(
                    k,
                    param_types,
                    keyword_types,
                ))),
                Some(Box::new(Self::substitute_param_refs_with_keywords(
                    v,
                    param_types,
                    keyword_types,
                ))),
            ),
            Type::Record(fields) => {
                let resolved: Vec<RecordField> = fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: Self::substitute_param_refs_with_keywords(
                            &field.value,
                            param_types,
                            keyword_types,
                        ),
                        optional: field.optional,
                    })
                    .collect();
                Type::Record(resolved)
            }
            Type::ReceiverMethodRef(receiver_type, method_name) => {
                let resolved_receiver = Self::substitute_param_refs_with_keywords(
                    receiver_type,
                    param_types,
                    keyword_types,
                );
                Type::ReceiverMethodRef(Box::new(resolved_receiver), *method_name)
            }
            Type::PatternIndexRef(subject, index) => {
                let resolved_subject =
                    Self::substitute_param_refs_with_keywords(subject, param_types, keyword_types);
                if Self::type_contains_param_ref(&resolved_subject) {
                    Type::PatternIndexRef(Box::new(resolved_subject), *index)
                } else {
                    Self::resolve_pattern_index_ref(&resolved_subject, *index)
                }
            }
            Type::PatternRestRef(subject) => {
                let resolved_subject =
                    Self::substitute_param_refs_with_keywords(subject, param_types, keyword_types);
                if Self::type_contains_param_ref(&resolved_subject) {
                    Type::PatternRestRef(Box::new(resolved_subject))
                } else {
                    Self::resolve_pattern_rest_ref(&resolved_subject)
                }
            }
            Type::PatternKeyRef(subject, key) => {
                let resolved_subject =
                    Self::substitute_param_refs_with_keywords(subject, param_types, keyword_types);
                if Self::type_contains_param_ref(&resolved_subject) {
                    Type::PatternKeyRef(Box::new(resolved_subject), key.clone())
                } else {
                    Self::resolve_pattern_key_ref(&resolved_subject, key)
                }
            }
            Type::PatternKeyRestRef(subject, matched_keys) => {
                let resolved_subject =
                    Self::substitute_param_refs_with_keywords(subject, param_types, keyword_types);
                if Self::type_contains_param_ref(&resolved_subject) {
                    Type::PatternKeyRestRef(Box::new(resolved_subject), matched_keys.clone())
                } else {
                    Self::resolve_pattern_key_rest_ref(&resolved_subject, matched_keys)
                }
            }
            Type::Proc {
                return_type,
                param_count,
            } => Type::Proc {
                return_type: Box::new(Self::substitute_param_refs_with_keywords(
                    return_type,
                    param_types,
                    keyword_types,
                )),
                param_count: *param_count,
            },
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|t| {
                        Self::substitute_param_refs_with_keywords(t, param_types, keyword_types)
                    })
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    fn resolve_pattern_index_ref(subject: &Type, index: usize) -> Type {
        match subject {
            Type::Union(parts) => {
                let mut resolved: Vec<Type> = parts
                    .iter()
                    .map(|part| Self::resolve_pattern_index_ref(part, index))
                    .collect();
                if resolved.iter().any(|ty| *ty != Type::Untyped) {
                    resolved.retain(|ty| *ty != Type::Untyped);
                }
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Tuple(elems) => elems.get(index).cloned().unwrap_or(Type::Untyped),
            Type::Array(Some(elem)) => *elem.clone(),
            _ => Type::Untyped,
        }
    }

    fn resolve_pattern_rest_ref(subject: &Type) -> Type {
        match subject {
            Type::Union(parts) => {
                let mut resolved: Vec<Type> =
                    parts.iter().map(Self::resolve_pattern_rest_ref).collect();
                if resolved
                    .iter()
                    .any(|ty| !Self::is_generic_pattern_placeholder(ty))
                {
                    resolved.retain(|ty| !Self::is_generic_pattern_placeholder(ty));
                }
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Tuple(elems) => Type::Array(Some(Box::new(Type::from_type_vec(elems.clone())))),
            Type::Array(Some(elem)) => Type::Array(Some(Box::new(*elem.clone()))),
            _ => Type::Array(Some(Box::new(Type::Untyped))),
        }
    }

    fn resolve_pattern_key_ref(subject: &Type, key: &RecordKey) -> Type {
        match subject {
            Type::Union(parts) => {
                let mut resolved: Vec<Type> = parts
                    .iter()
                    .map(|part| Self::resolve_pattern_key_ref(part, key))
                    .collect();
                if resolved.iter().any(|ty| *ty != Type::Untyped) {
                    resolved.retain(|ty| *ty != Type::Untyped);
                }
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Record(fields) => fields
                .iter()
                .find(|field| field.key == *key)
                .map(|field| field.value.clone())
                .unwrap_or(Type::Untyped),
            Type::Hash(key_type, Some(value_type))
                if key_type
                    .as_deref()
                    .map(|key_type| Self::pattern_hash_key_type_matches_for_pass(key_type, key))
                    .unwrap_or(true) =>
            {
                *value_type.clone()
            }
            _ => Type::Untyped,
        }
    }

    fn resolve_pattern_key_rest_ref(subject: &Type, matched_keys: &[RecordKey]) -> Type {
        match subject {
            Type::Union(parts) => {
                let mut resolved: Vec<Type> = parts
                    .iter()
                    .map(|part| Self::resolve_pattern_key_rest_ref(part, matched_keys))
                    .collect();
                if resolved
                    .iter()
                    .any(|ty| !Self::is_generic_pattern_placeholder(ty))
                {
                    resolved.retain(|ty| !Self::is_generic_pattern_placeholder(ty));
                }
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .filter(|field| !matched_keys.contains(&field.key))
                    .cloned()
                    .collect(),
            ),
            Type::Hash(Some(key), Some(value)) => {
                Type::Hash(Some(Box::new(*key.clone())), Some(Box::new(*value.clone())))
            }
            _ => Type::Hash(Some(Box::new(Type::Untyped)), Some(Box::new(Type::Untyped))),
        }
    }

    fn is_generic_pattern_placeholder(ty: &Type) -> bool {
        matches!(ty, Type::Untyped)
            || matches!(ty, Type::Array(Some(inner)) if **inner == Type::Untyped)
            || matches!(
                ty,
                Type::Hash(Some(key), Some(value))
                    if **key == Type::Untyped && **value == Type::Untyped
            )
    }

    fn pattern_hash_key_type_matches_for_pass(key_type: &Type, key: &RecordKey) -> bool {
        match (key_type, key) {
            (Type::Untyped | Type::Top, _) => true,
            (Type::Symbol, RecordKey::Symbol(_)) => true,
            (Type::String, RecordKey::String(_)) => true,
            (Type::LiteralSymbol(expected), RecordKey::Symbol(actual)) => expected == actual,
            (Type::LiteralString(expected), RecordKey::String(actual)) => expected == actual,
            (Type::Union(parts), _) => parts
                .iter()
                .any(|part| Self::pattern_hash_key_type_matches_for_pass(part, key)),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_needs_parameter_reference_resolution_only_for_param_refs() {
        let concrete = MethodDef {
            name: Sym::new("literal"),
            param_infos: Vec::new(),
            raw_return_type: Type::Integer,
            sorbet_modifier_comments: Vec::new(),
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            attr_ivar: None,
            is_singleton: false,
            rbs_file_source: false,
            synthetic_dsl_source: false,
            rbs_method_types: Default::default(),
            extra_overloads: Vec::new(),
            loc: None,
        };
        assert!(!InferenceEngine::method_needs_parameter_reference_resolution(&concrete));

        let receiver_ref = MethodDef {
            raw_return_type: Type::ReceiverMethodRef(
                Box::new(Type::Class(Sym::new("Array"))),
                Sym::new("each"),
            ),
            ..concrete.clone()
        };
        assert!(!InferenceEngine::method_needs_parameter_reference_resolution(&receiver_ref));

        let param_ref = MethodDef {
            raw_return_type: Type::Array(Some(Box::new(Type::ParamRef(0)))),
            ..concrete
        };
        assert!(InferenceEngine::method_needs_parameter_reference_resolution(&param_ref));
    }

    #[test]
    fn needs_parameter_reference_resolution_skips_files_without_param_ref_candidates() {
        let mut engine = InferenceEngine::new();
        engine.registry.mark_user_defined("A");
        engine.registry.add_method_def(
            "A",
            MethodDef {
                name: Sym::new("literal"),
                param_infos: Vec::new(),
                raw_return_type: Type::Integer,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: false,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: None,
            },
        );
        assert!(!engine.needs_parameter_reference_resolution());

        engine
            .registry
            .add_ivar_type("A", "@value", Type::ParamRef(0));
        assert!(engine.needs_parameter_reference_resolution());
    }

    #[test]
    fn collect_receiver_classes_skips_method_return_ref_owner() {
        let mut classes = Vec::new();
        InferenceEngine::collect_receiver_classes(
            &Type::MethodReturnRef("ApplicationController".into(), "error_view".into()),
            &mut classes,
        );
        assert!(classes.is_empty());
    }

    #[test]
    fn collect_receiver_reference_classes_preloads_object_for_unknown_self_call() {
        let engine = InferenceEngine::new();
        let mut classes = Vec::new();
        engine.collect_receiver_reference_classes(
            &Type::MethodReturnRef("Meta".into(), "to_s".into()),
            &mut classes,
        );
        assert_eq!(classes, vec!["Object".to_string()]);
    }

    #[test]
    fn collect_receiver_reference_classes_keeps_core_owner_for_stdlib_method() {
        let engine = InferenceEngine::new();
        let mut classes = Vec::new();
        engine.collect_receiver_reference_classes(
            &Type::MethodReturnRef("String".into(), "upcase".into()),
            &mut classes,
        );
        assert_eq!(classes, vec!["String".to_string()]);
    }

    #[test]
    fn collect_receiver_classes_keeps_receiver_method_refs() {
        let mut classes = Vec::new();
        InferenceEngine::collect_receiver_classes(
            &Type::ReceiverMethodRef(Box::new(Type::Class(Sym::new("Array"))), Sym::new("each")),
            &mut classes,
        );
        assert_eq!(classes, vec!["Array".to_string()]);
    }
}
