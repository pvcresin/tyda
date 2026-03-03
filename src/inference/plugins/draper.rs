use super::{DslFeature, Plugin, PluginCx, PluginManifest};
use crate::project::DslLibrary;
use crate::types::SourceLocation;
use ruby_prism::{CallNode, ParseResult};

pub(super) struct Draper;

static MANIFEST: PluginManifest = PluginManifest {
    id: "draper",
    features: &[DslFeature {
        library: DslLibrary::Draper,
        gem_markers: &["draper"],
    }],
    base_classes: &[],
    rails_default: false,
};

impl Plugin for Draper {
    fn manifest(&self) -> &'static PluginManifest {
        &MANIFEST
    }

    fn collect_class_body_call(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        method_name: &str,
        call_node: &CallNode<'_>,
        parse_result: &ParseResult<'_>,
        _comments: &[super::RbsComment],
    ) -> bool {
        if !cx.dsl_enabled(DslLibrary::Draper) {
            return false;
        }
        match method_name {
            "delegate_all" => {
                cx.collect_draper_delegate_all(class_name);
                true
            }
            "decorates" => {
                cx.collect_draper_decorates(class_name, call_node, parse_result);
                true
            }
            "decorates_finders" => {
                cx.collect_draper_decorates_finders(class_name);
                true
            }
            _ => false,
        }
    }

    fn class_body_method_names(&self) -> &'static [&'static str] {
        &["delegate_all", "decorates", "decorates_finders"]
    }

    fn register_on_class(&self, cx: &mut PluginCx<'_, '_>, class_name: &str, loc: SourceLocation) {
        cx.register_draper_class_methods(class_name, loc);
    }

    fn register_on_mixin(
        &self,
        cx: &mut PluginCx<'_, '_>,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        cx.register_draper_mixin_methods(class_name, module_name, loc);
    }
}

use super::super::*;

impl<'a> InferenceEngine<'a> {
    pub(in crate::inference) fn collect_draper_decorates(
        &mut self,
        class_name: &str,
        call_node: &ruby_prism::CallNode<'_>,
        parse_result: &ParseResult<'_>,
    ) {
        if !self.draper_dsl_enabled() || !self.is_draper_decorator_class(class_name) {
            return;
        }
        let Some(name) = self.first_symbol_or_string_arg(call_node) else {
            return;
        };
        let loc = offset_to_location(parse_result.source(), call_node.location().start_offset());
        let default_accessor =
            Self::underscore_name(&self.infer_draper_decorated_class_name(class_name));
        let explicit_accessor = Self::underscore_name(&self.camelize(&name));
        if default_accessor != explicit_accessor {
            self.registry
                .remove_method_variant(class_name, &default_accessor, false);
        }
        self.register_draper_decorator_methods(class_name, &self.camelize(&name), loc);
    }

    pub(in crate::inference) fn collect_draper_decorates_finders(&mut self, class_name: &str) {
        if !self.draper_dsl_enabled() || !self.is_draper_decorator_class(class_name) {
            return;
        }
        let loc = SourceLocation { line: 1, column: 0 };
        self.registry
            .add_mixin(class_name, "Draper::Finders", MixinKind::Extend);
        self.add_simple_method_if_missing(
            class_name,
            "decorate",
            Type::Class(Sym::new(class_name)),
            true,
            loc,
        );
    }

    pub(in crate::inference) fn collect_draper_delegate_all(&mut self, class_name: &str) {
        if !self.draper_dsl_enabled() || !self.is_draper_decorator_class(class_name) {
            return;
        }
        // `delegate_all` forwards undefined methods to the decorated model via `method_missing`.
        // If the decorated model is in another file and not yet loaded at collection time, the eager copy below is a no-op, so record the delegation itself as a synthetic `method_missing`.
        self.register_draper_delegate_all_marker(class_name);

        let Some(decorated_class) = self.infer_registered_draper_object_class(class_name) else {
            return;
        };
        let Some(methods) = self
            .registry
            .class_data_for(&decorated_class)
            .map(|data| data.methods.clone())
        else {
            return;
        };
        let loc = SourceLocation { line: 1, column: 0 };
        for method in methods {
            if method.is_singleton || method.name == "initialize" {
                continue;
            }
            if self
                .registry
                .has_method_variant(class_name, &method.name, false)
            {
                continue;
            }
            let mut delegated = (*method).clone();
            delegated.loc = Some(loc);
            delegated.attr_ivar = None;
            delegated.rbs_annotated = true;
            delegated.rbs_inline_annotated = false;
            delegated.sig_annotated = false;
            delegated.rbs_file_source = false;
            self.registry.add_method_def(class_name, delegated);
        }
    }

    fn register_draper_delegate_all_marker(&mut self, class_name: &str) {
        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("method_missing"),
                param_infos: Vec::new(),
                raw_return_type: Type::Untyped,
                sorbet_modifier_comments: Vec::new(),
                rbs_annotated: true,
                rbs_inline_annotated: false,
                sig_annotated: false,
                attr_ivar: None,
                is_singleton: false,
                rbs_file_source: true,
                synthetic_dsl_source: true,
                rbs_method_types: Default::default(),
                extra_overloads: Vec::new(),
                loc: None,
            },
        );
    }

    pub(in crate::inference) fn register_draper_class_methods(
        &mut self,
        class_name: &str,
        loc: SourceLocation,
    ) {
        if self.draper_dsl_enabled() && self.is_draper_decorator_class(class_name) {
            let decorated_class = self.infer_draper_decorated_class_name(class_name);
            self.register_draper_decorator_methods(class_name, &decorated_class, loc);
        }
    }

    pub(in crate::inference) fn register_draper_mixin_methods(
        &mut self,
        class_name: &str,
        module_name: &str,
        loc: SourceLocation,
    ) {
        if self.draper_dsl_enabled() && module_name == "Draper::Decoratable" {
            let decorator_class = format!("{class_name}Decorator");
            self.add_simple_method_if_missing(
                class_name,
                "decorate",
                Type::Class(Sym::new(decorator_class)),
                false,
                loc,
            );
        }
    }

    fn is_draper_decorator_class(&self, class_name: &str) -> bool {
        let mut current = Some(class_name.to_string());
        let mut seen: HashSet<String> = HashSet::new();
        while let Some(name) = current {
            if !seen.insert(name.clone()) {
                break;
            }
            let Some(data) = self.registry.class_data_for(&name) else {
                break;
            };
            if data.superclass.as_deref() == Some("Draper::Decorator") {
                return true;
            }
            current = data.superclass.as_ref().map(ToString::to_string);
        }
        false
    }

    fn infer_draper_decorated_class_name(&self, class_name: &str) -> String {
        if let Some(registered) = self.infer_registered_draper_object_class(class_name) {
            return registered;
        }
        let mut parts: Vec<&str> = class_name.split("::").collect();
        let last = parts.pop().unwrap_or(class_name);
        let base = last.strip_suffix("Decorator").unwrap_or(last);
        if parts.is_empty() {
            base.to_string()
        } else {
            format!("{}::{base}", parts.join("::"))
        }
    }

    fn infer_registered_draper_object_class(&self, class_name: &str) -> Option<String> {
        match self
            .registry
            .lookup_method_return_type(class_name, "object")
        {
            Some(Type::Class(name)) => Some(name.to_string()),
            _ => None,
        }
    }

    fn register_draper_decorator_methods(
        &mut self,
        class_name: &str,
        decorated_class: &str,
        loc: SourceLocation,
    ) {
        let decorated_type = Type::Class(Sym::new(decorated_class));
        self.record_reference(decorated_class);

        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("decorate"),
                param_infos: vec![
                    ParamInfo {
                        name: "object".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(decorated_type.clone()),
                    },
                    ParamInfo {
                        name: "options".to_string(),
                        kind: ParamKind::DoubleRest,
                        default_type: Some(Type::Untyped),
                    },
                ],
                raw_return_type: Type::Class(Sym::new(class_name)),
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
        self.registry.update_method_return_type_variant(
            class_name,
            "decorate",
            true,
            Type::Class(Sym::new(class_name)),
        );
        self.registry.update_method_param_default_type(
            class_name,
            "decorate",
            0,
            decorated_type.clone(),
        );

        self.registry.add_method_def_if_missing(
            class_name,
            MethodDef {
                name: Sym::new("initialize"),
                param_infos: vec![
                    ParamInfo {
                        name: "object".to_string(),
                        kind: ParamKind::Required,
                        default_type: Some(decorated_type.clone()),
                    },
                    ParamInfo {
                        name: "options".to_string(),
                        kind: ParamKind::DoubleRest,
                        default_type: Some(Type::Untyped),
                    },
                ],
                raw_return_type: Type::Void,
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
        self.registry.update_method_param_default_type(
            class_name,
            "initialize",
            0,
            decorated_type.clone(),
        );

        self.add_simple_method_if_missing(class_name, "object", decorated_type.clone(), false, loc);
        self.registry.update_instance_method_return_type(
            class_name,
            "object",
            decorated_type.clone(),
        );

        if !decorated_class.contains("::") {
            let accessor_name = Self::underscore_name(decorated_class);
            self.add_simple_method_if_missing(
                class_name,
                &accessor_name,
                decorated_type.clone(),
                false,
                loc,
            );
            self.registry.update_instance_method_return_type(
                class_name,
                &accessor_name,
                decorated_type,
            );
        }
    }

    pub(in crate::inference) fn underscore_name(name: &str) -> String {
        let mut out = String::with_capacity(name.len());
        for (idx, ch) in name.chars().enumerate() {
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
}
