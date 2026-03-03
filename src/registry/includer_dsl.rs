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

    pub fn apply_includer_bound_dsl(&mut self) {
        if self.includer_bound_dsl_applied || !self.has_includer_bound_dsl {
            return;
        }
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
