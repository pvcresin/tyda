use super::*;
use crate::rails::classify;

const COLLECTION_PROXY: &str = "ActiveRecord::Associations::CollectionProxy";

impl TypeRegistry {
    pub fn push_includer_bound_dsl(&mut self, class_name: &str, dsl: IncluderBoundDsl) {
        let cold = self.class_data_mut(class_name).cold_mut();
        if cold.includer_bound_dsl.contains(&dsl) {
            return;
        }
        cold.includer_bound_dsl.push(dsl);
        self.has_includer_bound_dsl = true;
        self.includer_bound_dsl_applied = false;
    }

    /// Records a class-body call whose definition is not known yet, to resolve it later
    /// against a class method written in a concern or parent class.
    pub(crate) fn push_pending_class_macro(
        &mut self,
        class_name: &str,
        macro_name: &str,
        args: Vec<Sym>,
    ) {
        let entry = (Sym::new(macro_name), args);
        let cold = self.class_data_mut(class_name).cold_mut();
        if cold.pending_class_macros.contains(&entry) {
            return;
        }
        cold.pending_class_macros.push(entry);
        self.has_includer_bound_dsl = true;
        self.includer_bound_dsl_applied = false;
    }

    /// A macro written in Ruby source (`mount_file_store_uploader Uploader` with
    /// `file_field: :file`) can define methods named by its literal arguments and symbol
    /// parameter defaults, and replaces the column accessors its body passes on.
    fn apply_pending_class_macros(&mut self) {
        let mut jobs: Vec<(String, Vec<Sym>, Vec<Sym>)> = Vec::new();
        for (class_name, data) in &self.class_data {
            let Some(cold) = data.cold_opt() else {
                continue;
            };
            for (macro_name, args) in &cold.pending_class_macros {
                let Some((owner, true)) =
                    self.first_method_call_owner_cached(class_name, macro_name.as_str(), true)
                else {
                    continue;
                };
                if owner.as_ref() == class_name.as_ref() {
                    continue;
                }
                let Some(method) = self
                    .lookup_method_def(&owner, macro_name.as_str(), true)
                    .filter(|method| !method.rbs_file_source && method.loc.is_some())
                else {
                    continue;
                };
                let replaced = self.names_replaced_by_source_macro(
                    &owner,
                    method,
                    args.iter().map(Sym::as_str),
                );
                let mut names = args.clone();
                names.extend(method.param_infos.iter().filter_map(
                    |param| match &param.default_type {
                        Some(Type::LiteralSymbol(name)) => Some(*name),
                        _ => None,
                    },
                ));
                jobs.push((class_name.to_string(), names, replaced));
            }
        }
        for (class_name, names, replaced) in jobs {
            if !self.class_inherits_from_active_record(&class_name) {
                continue;
            }
            for name in names {
                self.mark_runtime_attribute(&class_name, name.as_str());
            }
            for name in replaced {
                if self
                    .schema_column_accessor_type(&class_name, name.as_str())
                    .is_some()
                {
                    self.mark_column_accessor_replaced(&class_name, name.as_str());
                }
            }
        }
    }

    pub fn apply_includer_bound_dsl(&mut self) {
        if self.includer_bound_dsl_applied || !self.has_includer_bound_dsl {
            return;
        }
        self.apply_pending_class_macros();
        let mut jobs: Vec<(String, IncluderBoundDsl)> = Vec::new();
        for (class_name, data) in &self.class_data {
            for mixin in &data.mixins {
                let owner = self.resolve_scoped_class_ref(class_name, mixin.module_name.as_ref());
                let Some(owner_data) = self.class_data.get(owner.as_str()) else {
                    continue;
                };
                for dsl in &owner_data.cold().includer_bound_dsl {
                    jobs.push((class_name.to_string(), dsl.clone()));
                }
            }
        }
        for (includer, dsl) in jobs {
            self.apply_includer_bound_dsl_to(&includer, &dsl);
        }
        self.includer_bound_dsl_applied = true;
    }

    pub(crate) fn apply_includer_bound_dsl_to(&mut self, includer: &str, dsl: &IncluderBoundDsl) {
        match dsl {
            IncluderBoundDsl::Devise { loc } => {
                if self.inherits_any(includer, &["ActiveRecord::Base", "ApplicationRecord"]) {
                    self.register_devise_controller_helpers(includer, *loc);
                }
            }
            IncluderBoundDsl::AmsBelongsTo { name, loc } => {
                if self.inherits_any(includer, &["ActiveModel::Serializer"]) {
                    let target = classify(name);
                    self.add_simple_reader(includer, name, Type::Class(Sym::new(target)), *loc);
                }
            }
            IncluderBoundDsl::AmsHasMany { name, loc } => {
                if self.inherits_any(includer, &["ActiveModel::Serializer"]) {
                    let target = classify(name);
                    let ty = Type::Generic {
                        base: Sym::new(COLLECTION_PROXY),
                        args: vec![Type::Class(Sym::new(target))].into(),
                    };
                    self.add_simple_reader(includer, name, ty, *loc);
                }
            }
            IncluderBoundDsl::AmsModelAttributes { names, loc } => {
                if self.inherits_any(includer, &["ActiveModelSerializers::Model"]) {
                    for name in names {
                        self.add_simple_reader(includer, name, Type::Untyped, *loc);
                    }
                }
            }
            IncluderBoundDsl::ActiveRecordEnumAttribute { name, loc } => {
                if self.inherits_any(includer, &["ActiveRecord::Base", "ApplicationRecord"]) {
                    self.add_enum_reader(includer, name, *loc);
                }
            }
            IncluderBoundDsl::ActiveRecordSerializedAttribute { name, ty, loc } => {
                if self.inherits_any(includer, &["ActiveRecord::Base", "ApplicationRecord"]) {
                    self.register_typed_attribute_accessors(includer, name, ty.clone(), *loc, true);
                }
            }
            IncluderBoundDsl::ActiveRecordEnumStatuses { name, loc } => {
                if self.inherits_any(includer, &["ActiveRecord::Base", "ApplicationRecord"]) {
                    self.add_simple_singleton_reader(
                        includer,
                        name,
                        Type::Hash(Some(Box::new(Type::String)), Some(Box::new(Type::Untyped))),
                        *loc,
                    );
                }
            }
        }
    }

    fn register_typed_attribute_accessors(
        &mut self,
        class_name: &str,
        attr_name: &str,
        ty: Type,
        loc: SourceLocation,
        preserve_explicit_methods: bool,
    ) {
        let getter_is_explicit = preserve_explicit_methods
            && self
                .lookup_method_def(class_name, attr_name, false)
                .is_some_and(|method| {
                    !method.synthetic_dsl_source
                        && (method.rbs_file_source
                            || method.rbs_inline_annotated
                            || method.sig_annotated
                            || !method.rbs_annotated)
                });
        if self.has_method_variant(class_name, attr_name, false) && !getter_is_explicit {
            self.update_instance_method_return_type(class_name, attr_name, ty.clone());
        } else if !getter_is_explicit {
            self.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(attr_name),
                    param_infos: Vec::new(),
                    raw_return_type: ty.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: Some(format!("@{attr_name}")),
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }

        let setter_name = format!("{attr_name}=");
        let setter_is_explicit = preserve_explicit_methods
            && self
                .lookup_method_def(class_name, &setter_name, false)
                .is_some_and(|method| {
                    !method.synthetic_dsl_source
                        && (method.rbs_file_source
                            || method.rbs_inline_annotated
                            || method.sig_annotated
                            || !method.rbs_annotated)
                });
        if self.has_method_variant(class_name, &setter_name, false) && !setter_is_explicit {
            self.update_instance_method_return_type(class_name, &setter_name, ty.clone());
            self.update_method_param_default_type(class_name, &setter_name, 0, ty);
        } else if !setter_is_explicit {
            self.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(setter_name),
                    param_infos: vec![ParamInfo {
                        name: attr_name.to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(ty.clone()),
                    }],
                    raw_return_type: ty,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: Some(format!("@{attr_name}")),
                    is_singleton: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    rbs_method_types: Default::default(),
                    extra_overloads: Vec::new(),
                    loc: Some(loc),
                },
            );
        }
    }

    pub(crate) fn register_devise_controller_helpers(
        &mut self,
        resource_class: &str,
        loc: SourceLocation,
    ) {
        let resource_name = underscore_tail(resource_class);
        let resource_type = Type::Class(Sym::new(resource_class));
        self.add_simple_reader(
            "ActionController::Base",
            &format!("current_{resource_name}"),
            resource_type,
            loc,
        );
        self.add_simple_reader(
            "ActionController::Base",
            &format!("{resource_name}_signed_in?"),
            Type::Bool,
            loc,
        );
        self.add_simple_reader(
            "ActionController::Base",
            &format!("authenticate_{resource_name}!"),
            Type::Void,
            loc,
        );
        self.add_simple_reader(
            "ActionController::Base",
            &format!("{resource_name}_session"),
            Type::Untyped,
            loc,
        );
    }

    fn add_simple_reader(
        &mut self,
        class_name: &str,
        name: &str,
        return_type: Type,
        loc: SourceLocation,
    ) {
        self.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(name),
                param_infos: Vec::new(),
                raw_return_type: return_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
    }

    fn add_enum_reader(&mut self, class_name: &str, name: &str, loc: SourceLocation) {
        self.add_method_def_overriding_synthetic_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(name),
                param_infos: Vec::new(),
                raw_return_type: Type::String,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: false,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: false,
                synthetic_dsl_source: true,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
    }

    fn add_simple_singleton_reader(
        &mut self,
        class_name: &str,
        name: &str,
        return_type: Type,
        loc: SourceLocation,
    ) {
        self.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new(name),
                param_infos: Vec::new(),
                raw_return_type: return_type,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: true,
                rbs_file_source: false,
                synthetic_dsl_source: false,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: Some(loc),
            },
        );
    }

    fn inherits_any(&self, class_name: &str, bases: &[&str]) -> bool {
        if bases.contains(&class_name) {
            return true;
        }
        let mut current = self
            .class_data
            .get(class_name)
            .and_then(|data| data.superclass.as_ref().map(|s| s.to_string()));
        let mut depth = 0;
        while let Some(name) = current {
            if depth >= MAX_RESOLVE_DEPTH {
                break;
            }
            depth += 1;
            if bases.contains(&name.as_str()) {
                return true;
            }
            current = self
                .class_data
                .get(name.as_str())
                .and_then(|data| data.superclass.as_ref().map(|s| s.to_string()));
        }
        false
    }
}

fn underscore_tail(class_name: &str) -> String {
    let tail = class_name.split("::").last().unwrap_or(class_name);
    let mut out = String::with_capacity(tail.len());
    for (idx, ch) in tail.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if idx > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}
