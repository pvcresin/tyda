use super::*;
use crate::rbs::ir as rbs_ir;
use crate::types::TypeParamSig;

impl TypeRegistry {
    fn attr_writer_call_site_value_type(
        &self,
        class_name: &str,
        writer: &MethodDef,
    ) -> Option<Type> {
        if !matches!(
            self.method_visibility(class_name, writer.name.as_str(), false),
            Some(Visibility::Private | Visibility::Protected)
        ) {
            return None;
        }
        let _visit_guard = AttrWriterCallSiteVisitGuard::enter(class_name, writer.name.as_str())?;

        let data = self.class_data.get(class_name)?;
        let indexed =
            self.matching_call_site_indices(class_name, data, writer.name.as_str(), false);
        let mut values = Vec::new();
        let mut collect = |call_site: &CallSite| {
            if call_site.method_name.as_ref() != writer.name.as_str()
                || call_site.method_is_singleton
            {
                return true;
            }
            let Some(arg_type) = call_site.arg_types.first() else {
                return false;
            };
            if call_site.caller_context.is_some()
                && (Self::type_contains_param_ref(arg_type)
                    || Self::type_needs_deferred_resolution(arg_type))
            {
                return false;
            }
            let mut visiting = FxHashSet::default();
            let mut value_type =
                self.resolve_call_site_type_from_caller_context(call_site, arg_type, &mut visiting);
            if value_type == Type::Untyped || !Self::is_concrete_for_global_resolve(&value_type) {
                return false;
            }
            if call_site.caller_context.is_none() {
                value_type = value_type.widen_arg_for_param();
            }
            values.push(value_type);
            true
        };

        if let Some(indices) = indexed {
            for index in indices {
                let Ok(index) = usize::try_from(index) else {
                    return None;
                };
                if !collect(data.call_sites.get(index)) {
                    return None;
                }
            }
        } else {
            for call_site in &data.call_sites {
                if !collect(call_site) {
                    return None;
                }
            }
        }
        (!values.is_empty()).then(|| Type::from_type_vec(values))
    }

    pub(super) fn build_method_sigs_for_class(
        &self,
        class_name: &str,
        data: &ClassData,
    ) -> Vec<MethodSig> {
        let mut all_methods: Vec<MethodSig> = data
            .methods
            .iter()
            .map(|method| {
                let mut sig = self.build_method_sig(class_name, method);
                self.apply_declared_type_params(&mut sig, class_name, method);
                // look up the visibility override from the sparse map. Both `private` and `protected` are
                // stored in the map (since RBS has no `protected`, the renderer collapses it to `private`).
                sig.is_private = data
                    .cold()
                    .method_visibility
                    .contains_key(&(method.name, method.is_singleton));
                sig
            })
            .collect();

        let external_rbs_sig_indexes: HashMap<(String, bool), usize> = all_methods
            .iter()
            .enumerate()
            .filter(|(_, method_sig)| method_sig.is_external_rbs_source())
            .map(|(index, method_sig)| ((method_sig.name.clone(), method_sig.is_singleton), index))
            .collect();
        let locally_annotated: HashSet<(&str, bool)> = data
            .methods
            .iter()
            .filter(|method| method.has_annotation() && !method.rbs_file_source)
            .map(|method| (method.name.as_str(), method.is_singleton))
            .collect();
        for method_index in 0..all_methods.len() {
            let Some(&rbs_sig_index) = external_rbs_sig_indexes.get(&(
                all_methods[method_index].name.clone(),
                all_methods[method_index].is_singleton,
            )) else {
                continue;
            };
            if all_methods[method_index].rbs_file_source
                || locally_annotated.contains(&(
                    all_methods[method_index].name.as_str(),
                    all_methods[method_index].is_singleton,
                ))
                || method_index == rbs_sig_index
            {
                continue;
            }
            let (method_sig, rbs_sig) = if method_index < rbs_sig_index {
                let (before_rbs, rbs_and_after) = all_methods.split_at_mut(rbs_sig_index);
                (&mut before_rbs[method_index], &rbs_and_after[0])
            } else {
                let (before_method, method_and_after) = all_methods.split_at_mut(method_index);
                (&mut method_and_after[0], &before_method[rbs_sig_index])
            };
            if method_sig.return_type != Type::Untyped {
                method_sig.return_type = rbs_sig.return_type.clone();
            }
            for (param_index, param) in method_sig.params.iter_mut().enumerate() {
                if param.param_type == Type::Untyped
                    && let Some(rbs_param) = rbs_sig.params.get(param_index)
                    && rbs_param.param_type != Type::Untyped
                {
                    param.param_type = rbs_param.param_type.clone();
                }
            }
        }

        all_methods
    }

    pub fn build_classes(&self) -> Vec<ClassInfo> {
        let mut classes: Vec<ClassInfo> = self
            .class_data
            .iter()
            .map(|(name, data)| self.build_class_info(name, data, None))
            .collect();

        classes.sort_by(|a, b| a.name.cmp(&b.name));
        classes
    }

    pub fn build_output_classes(&self) -> Vec<ClassInfo> {
        let mut classes: Vec<ClassInfo> = self
            .class_data
            .iter()
            .filter(|(_, data)| data.user_defined)
            .map(|(name, data)| self.build_class_info(name, data, None))
            .collect();

        classes.sort_by(|a, b| a.name.cmp(&b.name));
        classes
    }

    pub fn build_output_top_level_constants(&self) -> Vec<ConstantSig> {
        let mut constants: Vec<ConstantSig> = self
            .class_data
            .get("Object")
            .map(|data| {
                data.constants
                    .values()
                    // lazy stdlib constants are lookup-only and are never emitted in the output.
                    .filter(|constant| !constant.external_source)
                    .map(|constant| self.output_constant_sig(constant))
                    .collect()
            })
            .unwrap_or_default();
        constants.sort_by(|a, b| {
            a.loc
                .map(|loc| (loc.line, loc.column))
                .cmp(&b.loc.map(|loc| (loc.line, loc.column)))
                .then(a.name.cmp(&b.name))
        });
        constants
    }

    fn output_constant_sig(&self, constant: &ConstantDef) -> ConstantSig {
        ConstantSig {
            name: constant.name.to_string(),
            const_type: constant.const_type.clone(),
            alias_kind: self.namespace_alias_kind(constant),
            loc: constant.loc,
            file_path: constant.file_path.as_deref().map(str::to_string),
        }
    }

    /// The keyword must match the target's declaration; an alias whose target kind is
    /// unknown stays a `singleton(...)` constant, which is still valid RBS.
    fn namespace_alias_kind(&self, constant: &ConstantDef) -> Option<NamespaceAliasKind> {
        let recorded = match constant.alias {
            ConstantAlias::None => return None,
            ConstantAlias::Namespace => None,
            ConstantAlias::NamespaceOf(kind) => Some(kind),
        };
        let Type::Singleton(target) = &constant.const_type else {
            return None;
        };
        self.class_data
            .get(target.as_str())
            .map(|data| NamespaceAliasKind::of(data.is_module))
            .or(recorded)
    }

    /// `Sym: Ord` compares by string content, so this keeps the byte-stable
    /// render order of the previous `Vec<String>`.
    pub fn output_class_names(&self) -> Vec<Sym> {
        let mut names: Vec<Sym> = self
            .class_data
            .iter()
            .filter(|(_, data)| data.user_defined)
            .map(|(name, _)| *name)
            .collect();
        names.sort();
        names
    }

    pub fn build_output_class_info(&self, name: &str) -> Option<ClassInfo> {
        let data = self.class_data.get(name)?;
        data.user_defined
            .then(|| self.build_class_info(name, data, None))
    }

    pub(super) fn build_class_info(
        &self,
        name: &str,
        data: &ClassData,
        file_filter: Option<&str>,
    ) -> ClassInfo {
        let mut all_methods = self.build_method_sigs_for_class(name, data);
        // synthesize dirty-family methods from the skeleton pattern to byte-match the old materialized version.
        self.splice_dirty_pattern_methods(name, data, &mut all_methods);

        // emit an RBS `alias` (and skip the `def`) when the alias target is rendered in the same class.
        let mut alias_sigs: Vec<MethodAliasSig> = Vec::new();
        let mut aliased_out: std::collections::HashSet<(String, bool)> =
            std::collections::HashSet::new();
        for alias in &data.cold().method_aliases {
            let canon = self.canonical_method_name(name, &alias.old_name, alias.is_singleton);
            if canon == alias.new_name {
                continue;
            }
            let target_renderable = all_methods
                .iter()
                .any(|m| m.name == canon && m.is_singleton == alias.is_singleton);
            let new_present = all_methods
                .iter()
                .any(|m| m.name == alias.new_name && m.is_singleton == alias.is_singleton);
            if target_renderable
                && new_present
                && aliased_out.insert((alias.new_name.clone(), alias.is_singleton))
            {
                alias_sigs.push(MethodAliasSig {
                    new_name: alias.new_name.clone(),
                    old_name: canon,
                    is_singleton: alias.is_singleton,
                    loc: alias.loc,
                });
            }
        }
        let all_methods: Vec<MethodSig> = all_methods
            .into_iter()
            .filter(|m| !aliased_out.contains(&(m.name.clone(), m.is_singleton)))
            .collect();
        alias_sigs.sort_by(|a, b| {
            a.loc
                .map(|loc| (loc.line, loc.column))
                .cmp(&b.loc.map(|loc| (loc.line, loc.column)))
                .then(a.new_name.cmp(&b.new_name))
        });

        let mixins: Vec<(String, String)> = data
            .mixins
            .iter()
            // The `[Elem]` header already declares the generic; Sorbet's runtime module adds nothing.
            .filter(|m| {
                !m.external_source
                    && !(m.kind == MixinKind::Extend && m.module_name.as_ref() == "T::Generic")
            })
            .map(|m| {
                let kind_str = match m.kind {
                    MixinKind::Include => "include",
                    MixinKind::Extend => "extend",
                    MixinKind::Prepend => "prepend",
                };
                (kind_str.to_string(), m.module_name.to_string())
            })
            .collect();

        let mut constants: Vec<ConstantSig> = data
            .constants
            .values()
            .filter(|constant| !constant.external_source)
            .filter(|constant| {
                file_filter.is_none_or(|file_path| constant.file_path.as_deref() == Some(file_path))
            })
            .map(|constant| self.output_constant_sig(constant))
            .collect();
        constants.sort_by(|a, b| {
            a.loc
                .map(|loc| (loc.line, loc.column))
                .cmp(&b.loc.map(|loc| (loc.line, loc.column)))
                .then(a.name.cmp(&b.name))
        });

        ClassInfo {
            name: name.to_string(),
            type_params: self.class_type_param_decls(name, data),
            methods: all_methods,
            aliases: alias_sigs,
            constants,
            sorbet_modifier_comments: data.cold().sorbet_modifier_comments.clone(),
            superclass: data.superclass.as_ref().map(ToString::to_string),
            mixins,
            is_module: data.is_module,
            loc: data.loc,
            file_path: data.file_path.as_deref().map(str::to_string),
        }
    }

    fn splice_dirty_pattern_methods(
        &self,
        class_name: &str,
        data: &ClassData,
        all_methods: &mut Vec<MethodSig>,
    ) {
        let Some(pattern) = &data.cold().dirty_method_pattern else {
            return;
        };
        // set of real method names (instance side). Skip any synthesized name that collides.
        let present: std::collections::HashSet<&str> = all_methods
            .iter()
            .filter(|m| !m.is_singleton)
            .map(|m| m.name.as_str())
            .collect();
        let by_column = pattern.enumerate_methods_by_column(&|name| present.contains(name));
        if by_column.iter().all(|(_, methods)| methods.is_empty()) {
            return;
        }
        // prepare the synthesized sig and candidate anchor name for each column.
        let mut rebuilt: Vec<MethodSig> = Vec::with_capacity(
            all_methods.len() + by_column.iter().map(|(_, m)| m.len()).sum::<usize>(),
        );
        // anchor name -> that column's synthesized sig list. The anchor is uniquely derived from the column name.
        let mut pending: FxHashMap<String, Vec<MethodSig>> = FxHashMap::default();
        // fallback for columns with no anchor method at all (placed at the end of the schema block).
        let mut orphan: Vec<MethodSig> = Vec::new();
        for (col, methods) in by_column {
            if methods.is_empty() {
                continue;
            }
            let sigs: Vec<MethodSig> = methods
                .iter()
                .map(|m| self.build_method_sig(class_name, m))
                .collect();
            let col = col.as_str();
            let predicate = format!("{col}?");
            let writer = format!("{col}=");
            let anchor = if present.contains(predicate.as_str()) {
                Some(predicate)
            } else if present.contains(writer.as_str()) {
                Some(writer)
            } else if present.contains(col) {
                Some(col.to_string())
            } else {
                None
            };
            match anchor {
                Some(anchor) => {
                    pending.entry(anchor).or_default().extend(sigs);
                }
                None => orphan.extend(sigs),
            }
        }
        // detect the position of the last real method (reader/writer/predicate) in the
        // schema block, and emit orphans right after it.
        let last_schema_anchor: Option<usize> = all_methods
            .iter()
            .rposition(|m| pending.contains_key(m.name.as_str()));
        for (idx, method) in all_methods.drain(..).enumerate() {
            let anchored = pending.remove(method.name.as_str());
            rebuilt.push(method);
            if let Some(sigs) = anchored {
                rebuilt.extend(sigs);
            }
            if Some(idx) == last_schema_anchor && !orphan.is_empty() {
                rebuilt.append(&mut orphan);
            }
        }
        // if there's no `last_schema_anchor` (no anchor methods at all), put orphans at the end.
        if !orphan.is_empty() {
            rebuilt.append(&mut orphan);
        }
        *all_methods = rebuilt;
    }

    /// `[unchecked out T < Bound = Default]` entries for the class header. RBS rejects a class
    /// redeclared with different type parameters, so these follow the declaration exactly.
    fn class_type_param_decls(&self, class_name: &str, data: &ClassData) -> Vec<String> {
        let cold = data.cold();
        cold.class_type_params
            .iter()
            .map(|name| {
                let mut decl = String::new();
                if let Some((_, modifiers)) = cold
                    .class_type_param_modifiers
                    .iter()
                    .find(|(param, _)| param == name)
                {
                    if modifiers.unchecked {
                        decl.push_str("unchecked ");
                    }
                    match modifiers.variance {
                        Some(rbs_sys::Variance::Covariant) => decl.push_str("out "),
                        Some(rbs_sys::Variance::Contravariant) => decl.push_str("in "),
                        None => {}
                    }
                }
                decl.push_str(name);
                if let Some((_, bound)) = cold
                    .class_type_param_bounds
                    .iter()
                    .find(|(param, _)| param == name)
                {
                    let bound = crate::rbs::import::convert_imported_rbs_type(
                        bound,
                        self.type_aliases(),
                        Some(class_name),
                    );
                    decl.push_str(&format!(" < {bound}"));
                }
                if let Some((_, default)) = cold
                    .class_type_param_defaults
                    .iter()
                    .find(|(param, _)| param == name)
                {
                    decl.push_str(&format!(" = {default}"));
                }
                decl
            })
            .collect()
    }

    /// Inference reads a declared method type parameter as `untyped`; the rendered signature
    /// keeps it by name, with its bounds.
    fn apply_declared_type_params(
        &self,
        sig: &mut MethodSig,
        owner_class: &str,
        method: &MethodDef,
    ) {
        let Some(primary) = method.rbs_method_types.first() else {
            return;
        };
        self.apply_method_type_params(
            primary,
            owner_class,
            &mut sig.type_params,
            &mut sig.params,
            &mut sig.return_type,
            sig.block.as_mut(),
        );
        if !method.extra_overloads.is_empty() {
            return;
        }
        for (overload, method_type) in sig
            .overloads
            .iter_mut()
            .zip(method.rbs_method_types.iter().skip(1))
        {
            self.apply_method_type_params(
                method_type,
                owner_class,
                &mut overload.type_params,
                &mut overload.params,
                &mut overload.return_type,
                overload.block.as_mut(),
            );
        }
    }

    fn apply_method_type_params(
        &self,
        method_type: &rbs_ir::MethodType,
        owner_class: &str,
        type_params: &mut Vec<TypeParamSig>,
        params: &mut [Param],
        return_type: &mut Type,
        block: Option<&mut HoverBlockSig>,
    ) {
        if method_type.type_params.is_empty() {
            return;
        }
        let bindings: HashMap<String, Type> = method_type
            .type_params
            .iter()
            .map(|name| (name.to_string(), Type::Class(*name)))
            .collect();
        let aliases = self.type_aliases();
        let convert = |ty: &rbs_ir::RbsType| {
            crate::rbs::import::convert_imported_rbs_type_with_bindings(
                ty,
                aliases,
                Some(owner_class),
                &bindings,
            )
        };
        let bound_of = |bounds: &[(Sym, rbs_ir::RbsType)], name: &Sym| {
            bounds
                .iter()
                .find(|(bound_name, _)| bound_name == name)
                .map(|(_, bound)| convert(bound))
        };
        *type_params = method_type
            .type_params
            .iter()
            .map(|name| TypeParamSig {
                name: name.to_string(),
                upper_bound: bound_of(&method_type.type_param_bounds, name),
                lower_bound: bound_of(&method_type.type_param_lower_bounds, name),
            })
            .collect();
        let declared = build_params_from_function_type(
            &method_type.function_type,
            &[],
            aliases,
            Some(owner_class),
            &bindings,
        );
        let mut positional: Vec<&mut Param> = params
            .iter_mut()
            .filter(|param| param.kind != ParamKind::Block)
            .collect();
        if positional.len() == declared.len() {
            for (param, declared) in positional.iter_mut().zip(declared) {
                param.param_type = declared.param_type;
            }
        }
        *return_type = convert(&method_type.function_type.return_type);
        if let (Some(block), Some(declared_block)) = (block, method_type.block.as_ref()) {
            let declared = build_params_from_function_type(
                &declared_block.function_type,
                &[],
                aliases,
                Some(owner_class),
                &bindings,
            );
            if block.params.len() == declared.len() {
                for (param, declared) in block.params.iter_mut().zip(declared) {
                    param.param_type = declared.param_type;
                }
            }
            block.return_type = convert(&declared_block.function_type.return_type);
        }
    }

    pub(super) fn build_method_sig(&self, class_name: &str, method: &MethodDef) -> MethodSig {
        self.build_method_sig_for_receiver(class_name, class_name, method)
    }

    pub(super) fn build_method_sig_for_receiver(
        &self,
        receiver_class: &str,
        owner_class: &str,
        method: &MethodDef,
    ) -> MethodSig {
        let mut params = self.resolve_params(receiver_class, method);
        if method
            .rbs_method_types
            .iter()
            .any(|method_type| method_type.block.is_some())
        {
            params.retain(|param| param.kind != ParamKind::Block);
        }
        let block = self.resolve_method_block(owner_class, method, &params);
        let mut return_type = if method.attr_ivar.is_some()
            && method.has_annotation()
            && method.raw_return_type != Type::Untyped
            && Self::is_concrete_for_global_resolve(&method.raw_return_type)
        {
            method.raw_return_type.clone()
        } else if let Some(ref ivar) = method.attr_ivar {
            let ivar_type = if method.is_singleton {
                self.lookup_singleton_ivar_type(receiver_class, ivar)
            } else {
                self.lookup_ivar_type(receiver_class, ivar)
            };
            match ivar_type {
                Some(Type::Untyped)
                | None
                | Some(Type::ParamRef(_))
                | Some(Type::KeywordParamRef(_)) => {
                    let from_init = if method.is_singleton {
                        None
                    } else {
                        self.infer_attr_type_from_initialize(receiver_class, ivar)
                    };
                    match from_init {
                        Some(ref ty) if Self::is_concrete_for_global_resolve(ty) => ty.clone(),
                        _ => method.raw_return_type.clone(),
                    }
                }
                Some(
                    ref deferred @ (Type::IvarRef(_)
                    | Type::MethodReturnRef(..)
                    | Type::ReceiverMethodRef(..)
                    | Type::DeferredSafeNavigation(..)),
                ) => {
                    let resolved = self.resolve_deferred_refs_for_context(
                        receiver_class,
                        method.is_singleton,
                        deferred,
                    );
                    if Self::is_concrete_for_global_resolve(&resolved) && resolved != Type::Untyped
                    {
                        resolved
                    } else {
                        let from_init = if method.is_singleton {
                            None
                        } else {
                            self.infer_attr_type_from_initialize(receiver_class, ivar)
                        };
                        match from_init {
                            Some(ref ty) if Self::is_concrete_for_global_resolve(ty) => ty.clone(),
                            _ => {
                                if resolved != Type::Untyped
                                    && !matches!(
                                        resolved,
                                        Type::IvarRef(_)
                                            | Type::MethodReturnRef(..)
                                            | Type::ReceiverMethodRef(..)
                                            | Type::DeferredSafeNavigation(..)
                                    )
                                {
                                    resolved
                                } else {
                                    method.raw_return_type.clone()
                                }
                            }
                        }
                    }
                }
                Some(ty) => ty,
            }
        } else {
            let _hop_memo = super::DeferredHopMemoScope::enter();
            let raw_return_type =
                self.resolve_block_return_refs(owner_class, method, &method.raw_return_type);
            let substituted = if Self::type_contains_param_ref(&raw_return_type) {
                self.resolve_param_refs_from_resolved(&raw_return_type, &params)
            } else {
                raw_return_type
            };
            let resolved = self.resolve_deferred_refs_for_context_owned(
                owner_class,
                method.is_singleton,
                substituted,
            );
            if Self::type_contains_param_ref(&resolved) {
                self.resolve_param_refs_from_resolved(&resolved, &params)
            } else {
                resolved
            }
        };
        // detect attr getters by unioning in external setter assignment types too (prevents widen explosion within the same class).
        if !method.is_singleton
            && !method.name.ends_with('=')
            && method.attr_ivar.is_some()
            && let Some(setter) =
                self.lookup_method_def(receiver_class, &format!("{}=", method.name), false)
            && setter.attr_ivar == method.attr_ivar
            && !method.has_annotation()
            && !setter.has_annotation()
        {
            if let Some(value_type) = self.attr_writer_call_site_value_type(receiver_class, setter)
                && Self::is_concrete_for_global_resolve(&return_type)
            {
                return_type = return_type.union_with(value_type);
            } else {
                let setter_params = self.resolve_params(receiver_class, setter);
                if let Some(value) = setter_params.first()
                    && Self::is_concrete_for_global_resolve(&value.param_type)
                {
                    return_type = if Self::is_concrete_for_global_resolve(&return_type) {
                        return_type.union_with(value.param_type.clone()).widen()
                    } else {
                        value.param_type.clone()
                    };
                }
            }
        }
        if method.name.ends_with('=')
            && method.attr_ivar.is_some()
            && let [param] = params.as_mut_slice()
        {
            if !method.has_annotation() && Self::is_concrete_for_global_resolve(&param.param_type) {
                return_type = param.param_type.clone().widen_arg_for_param();
            } else if !Self::is_concrete_for_global_resolve(&param.param_type)
                && Self::is_concrete_for_global_resolve(&return_type)
            {
                param.param_type = return_type.clone().widen_arg_for_param();
                if !method.has_annotation() {
                    return_type = param.param_type.clone();
                }
            }
        }

        let overloads: Vec<OverloadSig> = if !method.extra_overloads.is_empty() {
            method
                .extra_overloads
                .iter()
                .map(|o| {
                    let ol_params: Vec<Param> = o
                        .param_types
                        .iter()
                        .enumerate()
                        .map(|(i, (ty, kind))| {
                            let pname =
                                method.param_name_at(i).unwrap_or_else(|| format!("arg{i}"));
                            Param {
                                name: pname,
                                param_type: ty.clone(),
                                kind: *kind,
                            }
                        })
                        .collect();
                    OverloadSig {
                        type_params: Vec::new(),
                        params: ol_params,
                        return_type: o.return_type.clone(),
                        block: None,
                    }
                })
                .collect()
        } else if method.rbs_method_types.len() > 1 {
            method.rbs_method_types[1..]
                .iter()
                .map(|mt| {
                    let ft = &mt.function_type;
                    let param_names = method.effective_param_names();
                    let ol_params = build_params_from_function_type(
                        ft,
                        &param_names,
                        self.type_aliases(),
                        Some(owner_class),
                        &HashMap::new(),
                    );
                    OverloadSig {
                        type_params: Vec::new(),
                        params: ol_params,
                        return_type: crate::rbs::import::convert_imported_rbs_type(
                            &ft.return_type,
                            self.type_aliases(),
                            Some(owner_class),
                        ),
                        block: mt.block.as_ref().map(|block| HoverBlockSig {
                            params: build_params_from_function_type(
                                &block.function_type,
                                &[],
                                self.type_aliases(),
                                Some(owner_class),
                                &HashMap::new(),
                            ),
                            return_type: crate::rbs::import::convert_imported_rbs_type(
                                &block.function_type.return_type,
                                self.type_aliases(),
                                Some(owner_class),
                            ),
                            required: block.required,
                        }),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut sig = MethodSig {
            type_params: Vec::new(),
            name: method.name.to_string(),
            params,
            return_type,
            block,
            sorbet_modifier_comments: method.sorbet_modifier_comments.clone(),
            is_singleton: method.is_singleton,
            rbs_annotated: method.rbs_annotated,
            rbs_inline_annotated: method.rbs_inline_annotated,
            sig_annotated: method.sig_annotated,
            rbs_file_source: method.rbs_file_source,
            synthetic_dsl_source: method.synthetic_dsl_source,
            overloads,
            loc: method.loc,
            is_private: false,
        };
        // when rendering, collapse `instance` down to the owner's instance (a separate path from call-site substitution).
        Self::resolve_instance_type_in_sig(&mut sig, owner_class);
        sig
    }

    /// `replace_instance_type` is an identity deep clone for a signature with no `instance`
    /// in it, which is the overwhelming majority, so scan first and rewrite nothing.
    fn sig_contains_instance_type(sig: &MethodSig) -> bool {
        let block_has_instance = |block: &HoverBlockSig| {
            block.return_type.contains_instance_type()
                || block
                    .params
                    .iter()
                    .any(|param| param.param_type.contains_instance_type())
        };
        sig.return_type.contains_instance_type()
            || sig
                .params
                .iter()
                .any(|param| param.param_type.contains_instance_type())
            || sig.block.as_ref().is_some_and(block_has_instance)
            || sig.overloads.iter().any(|overload| {
                overload.return_type.contains_instance_type()
                    || overload
                        .params
                        .iter()
                        .any(|param| param.param_type.contains_instance_type())
                    || overload.block.as_ref().is_some_and(block_has_instance)
            })
    }

    fn resolve_instance_type_in_sig(sig: &mut MethodSig, owner_class: &str) {
        if !Self::sig_contains_instance_type(sig) {
            return;
        }
        let owner_instance = Type::Class(Sym::new(owner_class));
        let resolve_block = |block: &mut HoverBlockSig| {
            for param in &mut block.params {
                param.param_type = param.param_type.replace_instance_type(&owner_instance);
            }
            block.return_type = block.return_type.replace_instance_type(&owner_instance);
        };
        sig.return_type = sig.return_type.replace_instance_type(&owner_instance);
        for param in &mut sig.params {
            param.param_type = param.param_type.replace_instance_type(&owner_instance);
        }
        if let Some(block) = sig.block.as_mut() {
            resolve_block(block);
        }
        for overload in &mut sig.overloads {
            overload.return_type = overload.return_type.replace_instance_type(&owner_instance);
            for param in &mut overload.params {
                param.param_type = param.param_type.replace_instance_type(&owner_instance);
            }
            if let Some(block) = overload.block.as_mut() {
                resolve_block(block);
            }
        }
    }

    fn resolve_method_block(
        &self,
        owner_class: &str,
        method: &MethodDef,
        method_params: &[Param],
    ) -> Option<HoverBlockSig> {
        let has_explicit_block_param = method
            .param_infos
            .iter()
            .any(|param| param.kind == ParamKind::Block);
        if let Some(meta) =
            self.lookup_method_block_meta(owner_class, &method.name, method.is_singleton)
            && let Some((forwarded_name, forwarded_singleton)) = &meta.forwarded_block
            && let Some(target) =
                self.lookup_method_def(owner_class, forwarded_name, *forwarded_singleton)
        {
            let target_params = self.resolve_params(owner_class, target);
            return self.resolve_method_block(owner_class, target, &target_params);
        }

        if let Some(meta) =
            self.lookup_method_block_meta(owner_class, &method.name, method.is_singleton)
            && (!meta.yield_param_types.is_empty()
                || meta.return_type.is_some()
                || (meta.yield_requires_block.is_some() && !has_explicit_block_param))
        {
            let block_return_type =
                self.resolve_block_return_type(owner_class, &method.name, method.is_singleton);
            let block_return_type = if matches!(block_return_type, Type::MethodReturnRef(..)) {
                self.resolve_deferred_refs_for_context(
                    owner_class,
                    method.is_singleton,
                    &block_return_type,
                )
            } else {
                block_return_type
            };
            let params = meta
                .yield_param_types
                .iter()
                .enumerate()
                .map(|(idx, ty)| Param {
                    name: format!("arg{idx}"),
                    param_type: self.resolve_param_refs_from_resolved(ty, method_params),
                    kind: ParamKind::Required,
                })
                .collect();
            let declared_required = method
                .rbs_method_types
                .iter()
                .find_map(|method_type| method_type.block.as_ref().map(|block| block.required));
            let inferred_required = if self.method_call_has_user_source_definition(
                owner_class,
                "block_given?",
                method.is_singleton,
            ) {
                None
            } else {
                meta.yield_requires_block
            };
            let required = declared_required.or(inferred_required).unwrap_or(true);
            return Some(HoverBlockSig {
                params,
                return_type: block_return_type,
                required,
            });
        }

        method.rbs_method_types.first().and_then(|method_type| {
            method_type.block.as_ref().map(|block| HoverBlockSig {
                params: build_params_from_function_type(
                    &block.function_type,
                    &[],
                    self.type_aliases(),
                    Some(owner_class),
                    &HashMap::new(),
                ),
                return_type: crate::rbs::import::convert_imported_rbs_type(
                    &block.function_type.return_type,
                    self.type_aliases(),
                    Some(owner_class),
                ),
                required: block.required,
            })
        })
    }

    fn resolve_block_return_type(
        &self,
        class_name: &str,
        method_name: &str,
        method_is_singleton: bool,
    ) -> Type {
        if let Some(meta) =
            self.lookup_method_block_meta(class_name, method_name, method_is_singleton)
            && let Some(return_type) = &meta.return_type
        {
            return return_type.clone();
        }
        let mut types = Vec::new();
        if let Some(data) = self.class_data.get(class_name) {
            for call_site in &data.call_sites {
                if call_site.method_name.as_ref() == method_name
                    && call_site.method_is_singleton == method_is_singleton
                    && let Some(block) = &call_site.block
                {
                    Type::merge_into_vec(&mut types, block.return_type.clone());
                }
            }
        }
        if types.is_empty() {
            Type::Untyped
        } else {
            Type::from_type_vec(types)
        }
    }

    pub(super) fn resolve_block_return_refs(
        &self,
        owner_class: &str,
        method: &MethodDef,
        ty: &Type,
    ) -> Type {
        match ty {
            Type::BlockReturnRef => {
                self.resolve_block_return_type(owner_class, &method.name, method.is_singleton)
            }
            Type::DeferredSafeNavigation(receiver, result) => {
                let receiver = self.resolve_block_return_refs(owner_class, method, receiver);
                let result = self.resolve_block_return_refs(owner_class, method, result);
                Type::resolve_deferred_safe_navigation(receiver, result)
            }
            Type::Union(parts) => {
                let resolved: Vec<Type> = parts
                    .iter()
                    .map(|part| self.resolve_block_return_refs(owner_class, method, part))
                    .collect();
                if Type::is_short_circuit_array_shape_union(ty) {
                    Type::from_type_vec_preserve_short_circuit_array_shape(resolved)
                } else {
                    Type::from_type_vec_preserve_untyped(resolved)
                }
            }
            Type::Intersection(parts) => Type::Intersection(
                parts
                    .iter()
                    .map(|part| self.resolve_block_return_refs(owner_class, method, part))
                    .collect(),
            ),
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(
                self.resolve_block_return_refs(owner_class, method, inner),
            ))),
            Type::Hash(Some(key), Some(value)) => Type::Hash(
                Some(Box::new(self.resolve_block_return_refs(
                    owner_class,
                    method,
                    key,
                ))),
                Some(Box::new(self.resolve_block_return_refs(
                    owner_class,
                    method,
                    value,
                ))),
            ),
            Type::Hash(Some(key), None) => Type::Hash(
                Some(Box::new(self.resolve_block_return_refs(
                    owner_class,
                    method,
                    key,
                ))),
                None,
            ),
            Type::Hash(None, Some(value)) => Type::Hash(
                None,
                Some(Box::new(self.resolve_block_return_refs(
                    owner_class,
                    method,
                    value,
                ))),
            ),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: self.resolve_block_return_refs(owner_class, method, &field.value),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Tuple(elems) => Type::Tuple(
                elems
                    .iter()
                    .map(|elem| self.resolve_block_return_refs(owner_class, method, elem))
                    .collect(),
            ),
            Type::Proc {
                return_type,
                param_count,
                literal,
            } => Type::Proc {
                return_type: Box::new(self.resolve_block_return_refs(
                    owner_class,
                    method,
                    return_type,
                )),
                param_count: *param_count,
                literal: *literal,
            },
            _ => ty.clone(),
        }
    }
}

fn build_params_from_function_type(
    ft: &rbs_ir::FunctionType,
    fallback_param_names: &[String],
    type_aliases: &HashMap<String, Type>,
    current_scope: Option<&str>,
    type_bindings: &HashMap<String, Type>,
) -> Vec<Param> {
    let convert_imported_rbs_type = |ty: &rbs_ir::RbsType, aliases, scope| {
        crate::rbs::import::convert_imported_rbs_type_with_bindings(
            ty,
            aliases,
            scope,
            type_bindings,
        )
    };

    let mut params = Vec::new();
    let mut idx = 0;
    for p in &ft.required_positionals {
        let name = p
            .name
            .map(String::from)
            .or_else(|| fallback_param_names.get(idx).cloned())
            .unwrap_or_else(|| format!("arg{idx}"));
        params.push(Param {
            name,
            param_type: convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            kind: ParamKind::Required,
        });
        idx += 1;
    }
    for p in &ft.optional_positionals {
        let name = p
            .name
            .map(String::from)
            .or_else(|| fallback_param_names.get(idx).cloned())
            .unwrap_or_else(|| format!("arg{idx}"));
        params.push(Param {
            name,
            param_type: convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            kind: ParamKind::Optional,
        });
        idx += 1;
    }
    if let Some(ref p) = ft.rest_positionals {
        let name = p
            .name
            .map(String::from)
            .or_else(|| fallback_param_names.get(idx).cloned())
            .unwrap_or_else(|| format!("arg{idx}"));
        params.push(Param {
            name,
            param_type: convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            kind: ParamKind::Rest,
        });
        idx += 1;
    }
    for p in &ft.trailing_positionals {
        let name = p
            .name
            .map(String::from)
            .or_else(|| fallback_param_names.get(idx).cloned())
            .unwrap_or_else(|| format!("arg{idx}"));
        params.push(Param {
            name,
            param_type: convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            kind: ParamKind::Required,
        });
        idx += 1;
    }
    for (kw_name, p) in &ft.required_keywords {
        params.push(Param {
            name: kw_name.to_string(),
            param_type: convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            kind: ParamKind::KeywordRequired,
        });
    }
    for (kw_name, p) in &ft.optional_keywords {
        params.push(Param {
            name: kw_name.to_string(),
            param_type: convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            kind: ParamKind::KeywordOptional,
        });
    }
    if let Some(ref p) = ft.rest_keywords {
        let name = p
            .name
            .map(String::from)
            .unwrap_or_else(|| format!("arg{idx}"));
        params.push(Param {
            name,
            param_type: convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            kind: ParamKind::DoubleRest,
        });
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    fn method_def(
        name: &str,
        return_type: Type,
        is_singleton: bool,
        rbs_file_source: bool,
    ) -> MethodDef {
        MethodDef {
            name: Sym::new(name),
            param_infos: Vec::new(),
            raw_return_type: return_type,
            sorbet_modifier_comments: Vec::new(),
            rbs_annotated: false,
            rbs_inline_annotated: false,
            sig_annotated: false,
            attr_ivar: None,
            is_singleton,
            rbs_file_source,
            synthetic_dsl_source: false,
            rbs_method_types: Arc::new(Vec::new()),
            extra_overloads: Vec::new(),
            loc: None,
        }
    }

    #[test]
    fn external_instance_signature_does_not_widen_same_named_singleton_method() {
        let mut registry = TypeRegistry::new();
        registry.add_method_def("String", method_def("===", Type::Bool, false, true));
        registry.add_method_def("String", method_def("===", Type::True, true, false));

        let class = registry.class_data.get("String").expect("String class");
        let methods = registry.build_method_sigs_for_class("String", class);
        let instance = methods
            .iter()
            .find(|method| method.name == "===" && !method.is_singleton)
            .expect("instance ===");
        let singleton = methods
            .iter()
            .find(|method| method.name == "===" && method.is_singleton)
            .expect("singleton ===");

        assert_eq!(instance.return_type, Type::Bool);
        assert_eq!(singleton.return_type, Type::True);
    }

    fn top_level_alias_kind(registry: &TypeRegistry, name: &str) -> Option<NamespaceAliasKind> {
        registry
            .build_output_top_level_constants()
            .into_iter()
            .find(|constant| constant.name == name)
            .expect("constant")
            .alias_kind
    }

    fn set_alias(registry: &mut TypeRegistry, name: &str, target: &str, alias: ConstantAlias) {
        let ty = Type::Singleton(Sym::new(target));
        registry.set_constant_entry("Object", name, ty, None, None, alias);
    }

    #[test]
    fn namespace_alias_kind_prefers_output_declaration_over_recorded_kind() {
        let mut registry = TypeRegistry::new();
        registry.set_is_module("Target", true);
        let recorded = ConstantAlias::NamespaceOf(NamespaceAliasKind::Class);
        set_alias(&mut registry, "A", "Target", recorded);
        set_alias(&mut registry, "Ext", "External", recorded);
        set_alias(
            &mut registry,
            "Unknown",
            "External",
            ConstantAlias::Namespace,
        );
        set_alias(&mut registry, "Value", "Target", ConstantAlias::None);

        assert_eq!(
            top_level_alias_kind(&registry, "A"),
            Some(NamespaceAliasKind::Module)
        );
        assert_eq!(
            top_level_alias_kind(&registry, "Ext"),
            Some(NamespaceAliasKind::Class)
        );
        assert_eq!(top_level_alias_kind(&registry, "Unknown"), None);
        assert_eq!(top_level_alias_kind(&registry, "Value"), None);
    }
}
