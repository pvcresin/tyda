use crate::rbs::ir as rbs_ir;
use crate::types::Sym;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::rbs::convert::{convert_rbs_builtin_alias, convert_rbs_type};
use crate::registry::{MethodDef, MixinKind as RegistryMixinKind, ParamInfo, TypeRegistry};
use crate::types::{ParamKind, RecordField, RecordKey, Type};

#[derive(Debug, Clone)]
struct RawTypeAlias {
    type_params: Vec<String>,
    type_param_bounds: Vec<(String, rbs_ir::RbsType)>,
    type_param_defaults: Vec<(String, rbs_ir::RbsType)>,
    type_: rbs_ir::RbsType,
}

fn raw_alias_type_param_default_or_bound<'a>(
    raw_alias: &'a RawTypeAlias,
    param: &str,
) -> Option<&'a rbs_ir::RbsType> {
    raw_alias
        .type_param_defaults
        .iter()
        .find(|(name, _)| name == param)
        .map(|(_, type_)| type_)
        .or_else(|| {
            raw_alias
                .type_param_bounds
                .iter()
                .find(|(name, _)| name == param)
                .map(|(_, type_)| type_)
        })
}

pub fn load_rbs_definitions(paths: &[PathBuf]) -> TypeRegistry {
    load_rbs_definitions_with_classes(paths).0
}

pub fn load_rbs_definitions_with_classes(
    paths: &[PathBuf],
) -> (TypeRegistry, HashMap<String, Vec<String>>) {
    let rbs_files = collect_rbs_files(paths);

    let parsed: Vec<_> = rbs_files
        .par_iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(path).ok()?;
            let sig = rbs_sys::parse_signature(&content).ok()?;
            let aliases = extract_type_aliases(&sig);
            let declared_classes = collect_declared_classes_from_signature(&sig);
            Some((
                path.to_string_lossy().to_string(),
                sig,
                aliases,
                declared_classes,
            ))
        })
        .collect();

    let mut registry = TypeRegistry::new();
    let raw_aliases: HashMap<String, RawTypeAlias> = parsed
        .iter()
        .flat_map(|(_, _, aliases, _)| {
            aliases
                .iter()
                .map(|(name, alias)| (name.clone(), alias.clone()))
        })
        .collect();
    let resolved_aliases = resolve_rbs_type_aliases(&raw_aliases, registry.type_aliases());
    for (alias_name, ty) in resolved_aliases {
        registry.set_type_alias(&alias_name, ty);
    }
    let alias_snapshot = registry.type_aliases().clone();
    let mut type_file_classes = HashMap::new();
    for (path, sig, _, declared_classes) in &parsed {
        merge_signature_into_registry(sig, &alias_snapshot, &raw_aliases, &mut registry);
        if !declared_classes.is_empty() {
            type_file_classes.insert(path.clone(), declared_classes.clone());
        }
    }
    (registry, type_file_classes)
}

pub fn load_rbs_string(content: &str, registry: &mut TypeRegistry) {
    load_rbs_string_with_dependency_aliases(content, &[], registry);
}

pub fn rbs_parses(content: &str) -> bool {
    rbs_sys::parse_signature(content).is_ok()
}

pub(crate) fn load_rbs_string_with_dependency_aliases(
    content: &str,
    dependency_contents: &[String],
    registry: &mut TypeRegistry,
) {
    let sig = match rbs_sys::parse_signature(content) {
        Ok(sig) => sig,
        Err(_) => return,
    };

    let mut aliases = HashMap::new();
    for dependency_content in dependency_contents {
        if let Ok(dependency_sig) = rbs_sys::parse_signature(dependency_content) {
            aliases.extend(extract_type_aliases(&dependency_sig));
        }
    }
    aliases.extend(extract_type_aliases(&sig));
    let resolved_aliases = resolve_rbs_type_aliases(&aliases, registry.type_aliases());
    for (alias_name, ty) in resolved_aliases {
        registry.set_type_alias(&alias_name, ty);
    }
    let alias_snapshot = registry.type_aliases().clone();
    merge_signature_into_registry(&sig, &alias_snapshot, &aliases, registry);
}

fn merge_signature_into_registry(
    sig: &rbs_sys::Signature,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    registry: &mut TypeRegistry,
) {
    for decl in &sig.declarations {
        match decl {
            rbs_sys::Declaration::Class {
                name,
                methods,
                aliases,
                superclass,
                superclass_args,
                type_params,
                type_param_bounds,
                type_param_defaults,
                mixins,
                variables,
            } => {
                if let Some(sc) = superclass {
                    let superclass_name = resolve_classish_name_alias(sc, type_aliases);
                    registry.set_superclass(name, &superclass_name);
                    set_superclass_type_args(
                        registry,
                        type_aliases,
                        alias_templates,
                        name,
                        superclass_args,
                    );
                }
                if !type_params.is_empty() {
                    registry.set_class_type_params(name, type_params.clone());
                }
                set_class_type_param_bounds(
                    registry,
                    type_aliases,
                    alias_templates,
                    name,
                    type_param_bounds,
                );
                set_class_type_param_defaults(
                    registry,
                    type_aliases,
                    alias_templates,
                    name,
                    type_param_defaults,
                );
                add_mixins_to_registry(registry, type_aliases, alias_templates, name, mixins);
                add_variables_to_registry(registry, type_aliases, alias_templates, name, variables);
                add_methods_to_registry(registry, type_aliases, alias_templates, name, methods);
                apply_method_aliases(
                    registry,
                    type_aliases,
                    alias_templates,
                    name,
                    methods,
                    aliases,
                );
            }
            rbs_sys::Declaration::Module {
                name,
                methods,
                aliases,
                type_params,
                type_param_bounds,
                type_param_defaults,
                self_types,
                mixins,
                variables,
            } => {
                registry.set_is_module(name, true);
                if !type_params.is_empty() {
                    registry.set_class_type_params(name, type_params.clone());
                }
                set_class_type_param_bounds(
                    registry,
                    type_aliases,
                    alias_templates,
                    name,
                    type_param_bounds,
                );
                set_class_type_param_defaults(
                    registry,
                    type_aliases,
                    alias_templates,
                    name,
                    type_param_defaults,
                );
                for self_type in self_types {
                    let self_type_name = resolve_classish_name_alias(&self_type.name, type_aliases);
                    let type_args = resolve_rbs_types_for_inheritance(
                        &self_type.args,
                        type_aliases,
                        alias_templates,
                        Some(name),
                    );
                    registry.add_required_ancestor_with_type_args(name, &self_type_name, type_args);
                }
                add_mixins_to_registry(registry, type_aliases, alias_templates, name, mixins);
                add_variables_to_registry(registry, type_aliases, alias_templates, name, variables);
                add_methods_to_registry(registry, type_aliases, alias_templates, name, methods);
                apply_method_aliases(
                    registry,
                    type_aliases,
                    alias_templates,
                    name,
                    methods,
                    aliases,
                );
            }
            rbs_sys::Declaration::Interface {
                name,
                methods,
                aliases,
                type_params,
                type_param_bounds,
                type_param_defaults,
                mixins,
            } => {
                registry.set_is_module(name, true);
                if !type_params.is_empty() {
                    registry.set_class_type_params(name, type_params.clone());
                }
                set_class_type_param_bounds(
                    registry,
                    type_aliases,
                    alias_templates,
                    name,
                    type_param_bounds,
                );
                set_class_type_param_defaults(
                    registry,
                    type_aliases,
                    alias_templates,
                    name,
                    type_param_defaults,
                );
                add_mixins_to_registry(registry, type_aliases, alias_templates, name, mixins);
                add_methods_to_registry(registry, type_aliases, alias_templates, name, methods);
                apply_method_aliases(
                    registry,
                    type_aliases,
                    alias_templates,
                    name,
                    methods,
                    aliases,
                );
            }
            rbs_sys::Declaration::ClassAlias { new_name, old_name }
            | rbs_sys::Declaration::ModuleAlias { new_name, old_name } => {
                registry.set_type_alias(new_name, Type::Class(Sym::new(old_name)));
                register_rbs_constant(registry, new_name, Type::Singleton(Sym::new(old_name)));
            }
            rbs_sys::Declaration::Constant { name, type_ } => {
                let converted = convert_imported_rbs_type_with_templates(
                    &rbs_ir::RbsType::from(type_),
                    type_aliases,
                    alias_templates,
                    None,
                );
                register_rbs_constant(registry, name, converted);
            }
            rbs_sys::Declaration::Global { name, type_ } => {
                let converted = convert_imported_rbs_type_with_templates(
                    &rbs_ir::RbsType::from(type_),
                    type_aliases,
                    alias_templates,
                    None,
                );
                registry.set_global_variable_type(name, converted);
            }
            rbs_sys::Declaration::TypeAlias { name, .. } => {
                if let Some(converted) = type_aliases.get(name).cloned() {
                    registry.set_type_alias(name, converted);
                }
            }
        }
    }
}

/*
 * Backward-compatible entry point for Sorbet/RBS-comment aliases that do not
 * carry RBS type-parameter metadata.
 */
pub(crate) fn resolve_type_aliases(
    raw_aliases: &HashMap<String, rbs_ir::RbsType>,
    existing_aliases: &HashMap<String, Type>,
) -> HashMap<String, Type> {
    let wrapped: HashMap<String, RawTypeAlias> = raw_aliases
        .iter()
        .map(|(name, type_)| {
            (
                name.clone(),
                RawTypeAlias {
                    type_params: Vec::new(),
                    type_param_bounds: Vec::new(),
                    type_param_defaults: Vec::new(),
                    type_: type_.clone(),
                },
            )
        })
        .collect();
    resolve_rbs_type_aliases(&wrapped, existing_aliases)
}

fn resolve_rbs_type_aliases(
    raw_aliases: &HashMap<String, RawTypeAlias>,
    existing_aliases: &HashMap<String, Type>,
) -> HashMap<String, Type> {
    let mut resolved = existing_aliases.clone();
    let mut visiting = HashSet::new();
    let alias_names: Vec<String> = raw_aliases.keys().cloned().collect();
    for alias_name in alias_names {
        let ty = resolve_pending_alias_type(
            &alias_name,
            raw_aliases,
            &resolved,
            &mut visiting,
            existing_aliases,
            &HashMap::new(),
        );
        resolved.insert(alias_name, ty);
    }
    resolved
}

fn register_rbs_constant(registry: &mut TypeRegistry, qualified_name: &str, ty: Type) {
    let (owner, const_name) = match qualified_name.rsplit_once("::") {
        Some((owner, tail)) => (owner.trim_start_matches("::"), tail),
        None => ("Object", qualified_name),
    };
    registry.set_constant(owner, const_name, ty, None, None);
}

fn collect_declared_classes_from_signature(sig: &rbs_sys::Signature) -> Vec<String> {
    sig.declarations
        .iter()
        .filter_map(|declaration| match declaration {
            rbs_sys::Declaration::Class { name, .. }
            | rbs_sys::Declaration::Module { name, .. }
            | rbs_sys::Declaration::Interface { name, .. } => Some(name.clone()),
            rbs_sys::Declaration::ClassAlias { new_name, .. }
            | rbs_sys::Declaration::ModuleAlias { new_name, .. } => Some(new_name.clone()),
            rbs_sys::Declaration::Constant { .. }
            | rbs_sys::Declaration::Global { .. }
            | rbs_sys::Declaration::TypeAlias { .. } => None,
        })
        .collect()
}

fn add_mixins_to_registry(
    registry: &mut TypeRegistry,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    class_name: &str,
    mixins: &[rbs_sys::MixinDecl],
) {
    for mixin in mixins {
        let kind = match mixin.kind {
            rbs_sys::MixinKind::Include => RegistryMixinKind::Include,
            rbs_sys::MixinKind::Extend => RegistryMixinKind::Extend,
            rbs_sys::MixinKind::Prepend => RegistryMixinKind::Prepend,
        };
        let mixin_name = resolve_classish_name_alias(&mixin.name, type_aliases);
        let type_args = resolve_rbs_types_for_inheritance(
            &mixin.args,
            type_aliases,
            alias_templates,
            Some(class_name),
        );
        registry.add_external_mixin_with_type_args(class_name, &mixin_name, kind, type_args);
    }
}

fn set_superclass_type_args(
    registry: &mut TypeRegistry,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    class_name: &str,
    args: &[rbs_sys::RbsType],
) {
    if args.is_empty() {
        return;
    }
    let type_args =
        resolve_rbs_types_for_inheritance(args, type_aliases, alias_templates, Some(class_name));
    registry.set_superclass_type_args(class_name, type_args);
}

fn resolve_rbs_types_for_inheritance(
    args: &[rbs_sys::RbsType],
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
) -> Vec<rbs_ir::RbsType> {
    let mut visiting = HashSet::new();
    args.iter()
        .map(|arg| {
            resolve_rbs_method_alias_type(
                &rbs_ir::RbsType::from(arg),
                type_aliases,
                alias_templates,
                current_scope,
                &HashMap::new(),
                &mut visiting,
            )
        })
        .collect()
}

fn set_class_type_param_bounds(
    registry: &mut TypeRegistry,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    class_name: &str,
    bounds: &[(String, rbs_sys::RbsType)],
) {
    if bounds.is_empty() {
        return;
    }
    let mut visiting = HashSet::new();
    let converted = bounds
        .iter()
        .map(|(name, bound)| {
            (
                name.clone(),
                resolve_rbs_method_alias_type(
                    &rbs_ir::RbsType::from(bound),
                    type_aliases,
                    alias_templates,
                    Some(class_name),
                    &HashMap::new(),
                    &mut visiting,
                ),
            )
        })
        .collect();
    registry.set_class_type_param_bounds(class_name, converted);
}

fn set_class_type_param_defaults(
    registry: &mut TypeRegistry,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    class_name: &str,
    defaults: &[(String, rbs_sys::RbsType)],
) {
    if defaults.is_empty() {
        return;
    }
    let converted = defaults
        .iter()
        .map(|(name, default_type)| {
            (
                name.clone(),
                convert_imported_rbs_type_with_templates(
                    &rbs_ir::RbsType::from(default_type),
                    type_aliases,
                    alias_templates,
                    Some(class_name),
                ),
            )
        })
        .collect();
    registry.set_class_type_param_defaults(class_name, converted);
}

fn resolve_classish_name_alias(name: &str, type_aliases: &HashMap<String, Type>) -> String {
    match type_aliases.get(name) {
        Some(Type::Class(target)) | Some(Type::Singleton(target)) => (target.clone()).to_string(),
        _ => name.to_string(),
    }
}

fn add_variables_to_registry(
    registry: &mut TypeRegistry,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    class_name: &str,
    variables: &[rbs_sys::VariableDecl],
) {
    for variable in variables {
        let ty = convert_imported_rbs_type_with_templates(
            &rbs_ir::RbsType::from(&variable.type_),
            type_aliases,
            alias_templates,
            Some(class_name),
        );
        match variable.kind {
            rbs_sys::VariableKind::Instance => {
                registry.replace_ivar_type(class_name, &variable.name, ty);
            }
            rbs_sys::VariableKind::ClassInstance => {
                registry.replace_singleton_ivar_type(class_name, &variable.name, ty);
            }
            rbs_sys::VariableKind::Class => {
                registry.replace_class_variable_type(class_name, &variable.name, ty);
            }
        }
    }
}

fn convert_function_params_with_templates(
    function_type: &rbs_ir::FunctionType,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
) -> Vec<(String, ParamKind, Type)> {
    let mut params = Vec::new();
    let mut positional_index = 0;
    for param in &function_type.required_positionals {
        let name = param
            .name
            .map(String::from)
            .unwrap_or_else(|| format!("arg{positional_index}"));
        let ty = convert_imported_rbs_type_with_templates(
            &param.type_,
            type_aliases,
            alias_templates,
            current_scope,
        );
        params.push((name, ParamKind::Required, ty));
        positional_index += 1;
    }
    for param in &function_type.optional_positionals {
        let name = param
            .name
            .map(String::from)
            .unwrap_or_else(|| format!("arg{positional_index}"));
        let ty = convert_imported_rbs_type_with_templates(
            &param.type_,
            type_aliases,
            alias_templates,
            current_scope,
        );
        params.push((name, ParamKind::Optional, ty));
        positional_index += 1;
    }
    if let Some(param) = &function_type.rest_positionals {
        let name = param
            .name
            .map(String::from)
            .unwrap_or_else(|| "args".to_string());
        let ty = convert_imported_rbs_type_with_templates(
            &param.type_,
            type_aliases,
            alias_templates,
            current_scope,
        );
        params.push((name, ParamKind::Rest, ty));
    }
    for param in &function_type.trailing_positionals {
        let name = param
            .name
            .map(String::from)
            .unwrap_or_else(|| format!("arg{positional_index}"));
        let ty = convert_imported_rbs_type_with_templates(
            &param.type_,
            type_aliases,
            alias_templates,
            current_scope,
        );
        params.push((name, ParamKind::Required, ty));
        positional_index += 1;
    }
    for (keyword, param) in &function_type.required_keywords {
        let ty = convert_imported_rbs_type_with_templates(
            &param.type_,
            type_aliases,
            alias_templates,
            current_scope,
        );
        params.push((String::from(*keyword), ParamKind::KeywordRequired, ty));
    }
    for (keyword, param) in &function_type.optional_keywords {
        let ty = convert_imported_rbs_type_with_templates(
            &param.type_,
            type_aliases,
            alias_templates,
            current_scope,
        );
        params.push((String::from(*keyword), ParamKind::KeywordOptional, ty));
    }
    if let Some(param) = &function_type.rest_keywords {
        let name = param
            .name
            .map(String::from)
            .unwrap_or_else(|| "kwargs".to_string());
        let ty = convert_imported_rbs_type_with_templates(
            &param.type_,
            type_aliases,
            alias_templates,
            current_scope,
        );
        params.push((name, ParamKind::DoubleRest, ty));
    }
    params
}

fn apply_method_aliases(
    registry: &mut TypeRegistry,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    class_name: &str,
    methods: &[rbs_sys::MethodDecl],
    aliases: &[rbs_sys::MethodAliasDecl],
) {
    for alias in aliases {
        if let Some(original) = methods.iter().find(|method| {
            method.name == alias.old_name && method_kind_has_side(&method.kind, &alias.kind)
        }) {
            let ir_method_types = rbs_ir::method_types_from_rbs(&original.method_types);
            let first_overload = match ir_method_types.first() {
                Some(method_type) => method_type,
                None => continue,
            };
            let callable_without_block = method_type_callable_without_block(&ir_method_types);

            let return_type = callable_without_block
                .map(|method_type| {
                    let mut return_type = convert_imported_rbs_type_with_templates(
                        &method_type.function_type.return_type,
                        type_aliases,
                        alias_templates,
                        Some(class_name),
                    );
                    if return_type != Type::Untyped
                        && method_type
                            .annotations
                            .iter()
                            .any(|a| a == "implicitly-returns-nil")
                    {
                        return_type = return_type.union_with(Type::Nil);
                    }
                    return_type
                })
                .unwrap_or(Type::Untyped);

            let params = convert_function_params_with_templates(
                &first_overload.function_type,
                type_aliases,
                alias_templates,
                Some(class_name),
            );
            let is_singleton = alias.kind == rbs_sys::MethodKind::Singleton;
            let mut param_infos = Vec::new();
            for (index, (name, kind, ty)) in params.into_iter().enumerate() {
                registry.set_annotated_param_type(
                    class_name,
                    &alias.new_name,
                    is_singleton,
                    index,
                    ty,
                );
                param_infos.push(ParamInfo {
                    name,
                    kind,
                    default_type: None,
                });
            }

            let rbs_method_types = resolve_method_types_aliases(
                &ir_method_types,
                type_aliases,
                alias_templates,
                Some(class_name),
            );

            registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(&alias.new_name),
                    param_infos,
                    raw_return_type: return_type,
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: original.attr_ivar.clone(),
                    is_singleton,
                    rbs_file_source: true,
                    synthetic_dsl_source: false,
                    rbs_method_types: std::sync::Arc::new(rbs_method_types),
                    extra_overloads: Vec::new(),
                    loc: None,
                },
            );
        } else {
            apply_registry_method_alias(registry, class_name, alias);
        }
    }
}

fn apply_registry_method_alias(
    registry: &mut TypeRegistry,
    class_name: &str,
    alias: &rbs_sys::MethodAliasDecl,
) {
    for &is_singleton in method_kind_sides(&alias.kind) {
        if !register_method_alias_side(
            registry,
            class_name,
            &alias.old_name,
            &alias.new_name,
            is_singleton,
        ) {
            // Merge order isn't guaranteed, so if the alias target isn't merged yet, defer until finalize.
            registry.push_pending_method_alias(crate::registry::PendingMethodAlias {
                class_name: class_name.to_string(),
                old_name: alias.old_name.clone(),
                new_name: alias.new_name.clone(),
                is_singleton,
            });
        }
    }
}

fn register_method_alias_side(
    registry: &mut TypeRegistry,
    class_name: &str,
    old_name: &str,
    new_name: &str,
    is_singleton: bool,
) -> bool {
    let Some((source_owner, source_is_singleton, source_method)) =
        registry_alias_source_method(registry, class_name, old_name, is_singleton)
    else {
        return false;
    };

    for index in 0..source_method.param_infos.len() {
        if let Some(ty) =
            registry.get_annotated_param_type(&source_owner, old_name, source_is_singleton, index)
        {
            registry.set_annotated_param_type(class_name, new_name, is_singleton, index, ty);
        }
    }

    let mut alias_method = source_method;
    alias_method.name = Sym::new(new_name);
    alias_method.is_singleton = is_singleton;
    alias_method.rbs_annotated = true;
    alias_method.rbs_inline_annotated = false;
    alias_method.rbs_file_source = true;
    alias_method.synthetic_dsl_source = false;
    alias_method.loc = None;
    registry.add_method_def(class_name, alias_method);
    true
}

// After merging, re-resolves cross-file aliases over the ancestor graph (chains reach a fixpoint).
pub(crate) fn finalize_pending_method_aliases(registry: &mut TypeRegistry) {
    let pending = registry.take_pending_method_aliases();
    if pending.is_empty() {
        return;
    }
    for p in pending {
        if !register_method_alias_side(
            registry,
            &p.class_name,
            &p.old_name,
            &p.new_name,
            p.is_singleton,
        ) {
            // The target owner still isn't merged. With lazy loading, a later merge_class_into
            // call will bring the ancestors in, so keep it pending and retry on the next finalize.
            registry.push_pending_method_alias(p);
        }
    }
}

fn registry_alias_source_method(
    registry: &TypeRegistry,
    class_name: &str,
    old_name: &str,
    is_singleton: bool,
) -> Option<(String, bool, MethodDef)> {
    // Module method aliases fall back to the Object ancestor chain (a module chain has no Object/BasicObject).
    let (owner, source_is_singleton) = registry
        .resolve_method_call_owners(class_name, old_name, is_singleton)
        .into_iter()
        .next()
        .or_else(|| {
            registry
                .resolve_method_call_owners("Object", old_name, is_singleton)
                .into_iter()
                .next()
        })?;
    let source_method = registry
        .lookup_method_def(&owner, old_name, source_is_singleton)?
        .clone();
    Some((owner, source_is_singleton, source_method))
}

fn method_kind_sides(kind: &rbs_sys::MethodKind) -> &'static [bool] {
    match kind {
        rbs_sys::MethodKind::Instance => &[false],
        rbs_sys::MethodKind::Singleton => &[true],
        rbs_sys::MethodKind::SingletonInstance => &[false, true],
    }
}

fn method_kind_has_side(kind: &rbs_sys::MethodKind, requested: &rbs_sys::MethodKind) -> bool {
    match requested {
        rbs_sys::MethodKind::Instance => matches!(
            kind,
            rbs_sys::MethodKind::Instance | rbs_sys::MethodKind::SingletonInstance
        ),
        rbs_sys::MethodKind::Singleton => matches!(
            kind,
            rbs_sys::MethodKind::Singleton | rbs_sys::MethodKind::SingletonInstance
        ),
        rbs_sys::MethodKind::SingletonInstance => {
            matches!(kind, rbs_sys::MethodKind::SingletonInstance)
        }
    }
}

fn resolve_method_types_aliases(
    method_types: &[rbs_ir::MethodType],
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
) -> Vec<rbs_ir::MethodType> {
    method_types
        .iter()
        .map(|method_type| {
            let mut visiting = HashSet::new();
            resolve_method_type_aliases(
                method_type,
                type_aliases,
                alias_templates,
                current_scope,
                &HashMap::new(),
                &mut visiting,
            )
        })
        .collect()
}

pub(crate) fn resolve_imported_method_types_aliases(
    method_types: &[rbs_ir::MethodType],
    type_aliases: &HashMap<String, Type>,
    current_scope: Option<&str>,
) -> Vec<rbs_ir::MethodType> {
    resolve_method_types_aliases(method_types, type_aliases, &HashMap::new(), current_scope)
}

fn resolve_method_type_aliases(
    method_type: &rbs_ir::MethodType,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
    type_bindings: &HashMap<String, rbs_ir::RbsType>,
    visiting: &mut HashSet<String>,
) -> rbs_ir::MethodType {
    rbs_ir::MethodType {
        function_type: resolve_function_type_aliases(
            &method_type.function_type,
            type_aliases,
            alias_templates,
            current_scope,
            type_bindings,
            visiting,
        ),
        block: method_type.block.as_deref().map(|block| {
            Box::new(resolve_block_type_aliases(
                block,
                type_aliases,
                alias_templates,
                current_scope,
                type_bindings,
                visiting,
            ))
        }),
        self_type: method_type.self_type.as_deref().map(|self_type| {
            Box::new(resolve_rbs_method_alias_type(
                self_type,
                type_aliases,
                alias_templates,
                current_scope,
                type_bindings,
                visiting,
            ))
        }),
        type_params: method_type.type_params.clone(),
        type_param_bounds: method_type
            .type_param_bounds
            .iter()
            .map(|(name, bound)| {
                (
                    *name,
                    resolve_rbs_method_alias_type(
                        bound,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    ),
                )
            })
            .collect(),
        type_param_lower_bounds: method_type
            .type_param_lower_bounds
            .iter()
            .map(|(name, lower_bound)| {
                (
                    *name,
                    resolve_rbs_method_alias_type(
                        lower_bound,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    ),
                )
            })
            .collect(),
        annotations: method_type.annotations.clone(),
    }
}

fn resolve_block_type_aliases(
    block: &rbs_ir::BlockType,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
    type_bindings: &HashMap<String, rbs_ir::RbsType>,
    visiting: &mut HashSet<String>,
) -> rbs_ir::BlockType {
    rbs_ir::BlockType {
        function_type: resolve_function_type_aliases(
            &block.function_type,
            type_aliases,
            alias_templates,
            current_scope,
            type_bindings,
            visiting,
        ),
        required: block.required,
        self_type: block.self_type.as_deref().map(|self_type| {
            Box::new(resolve_rbs_method_alias_type(
                self_type,
                type_aliases,
                alias_templates,
                current_scope,
                type_bindings,
                visiting,
            ))
        }),
    }
}

fn resolve_function_type_aliases(
    function_type: &rbs_ir::FunctionType,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
    type_bindings: &HashMap<String, rbs_ir::RbsType>,
    visiting: &mut HashSet<String>,
) -> rbs_ir::FunctionType {
    rbs_ir::FunctionType {
        required_positionals: resolve_function_param_aliases_vec(
            &function_type.required_positionals,
            type_aliases,
            alias_templates,
            current_scope,
            type_bindings,
            visiting,
        ),
        optional_positionals: resolve_function_param_aliases_vec(
            &function_type.optional_positionals,
            type_aliases,
            alias_templates,
            current_scope,
            type_bindings,
            visiting,
        ),
        rest_positionals: function_type.rest_positionals.as_deref().map(|param| {
            Box::new(resolve_function_param_aliases(
                param,
                type_aliases,
                alias_templates,
                current_scope,
                type_bindings,
                visiting,
            ))
        }),
        trailing_positionals: resolve_function_param_aliases_vec(
            &function_type.trailing_positionals,
            type_aliases,
            alias_templates,
            current_scope,
            type_bindings,
            visiting,
        ),
        required_keywords: function_type
            .required_keywords
            .iter()
            .map(|(name, param)| {
                (
                    *name,
                    resolve_function_param_aliases(
                        param,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    ),
                )
            })
            .collect(),
        optional_keywords: function_type
            .optional_keywords
            .iter()
            .map(|(name, param)| {
                (
                    *name,
                    resolve_function_param_aliases(
                        param,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    ),
                )
            })
            .collect(),
        rest_keywords: function_type.rest_keywords.as_deref().map(|param| {
            Box::new(resolve_function_param_aliases(
                param,
                type_aliases,
                alias_templates,
                current_scope,
                type_bindings,
                visiting,
            ))
        }),
        return_type: resolve_rbs_method_alias_type(
            &function_type.return_type,
            type_aliases,
            alias_templates,
            current_scope,
            type_bindings,
            visiting,
        ),
    }
}

fn resolve_function_param_aliases_vec(
    params: &[rbs_ir::FunctionParam],
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
    type_bindings: &HashMap<String, rbs_ir::RbsType>,
    visiting: &mut HashSet<String>,
) -> Box<[rbs_ir::FunctionParam]> {
    params
        .iter()
        .map(|param| {
            resolve_function_param_aliases(
                param,
                type_aliases,
                alias_templates,
                current_scope,
                type_bindings,
                visiting,
            )
        })
        .collect()
}

fn resolve_function_param_aliases(
    param: &rbs_ir::FunctionParam,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
    type_bindings: &HashMap<String, rbs_ir::RbsType>,
    visiting: &mut HashSet<String>,
) -> rbs_ir::FunctionParam {
    rbs_ir::FunctionParam {
        type_: resolve_rbs_method_alias_type(
            &param.type_,
            type_aliases,
            alias_templates,
            current_scope,
            type_bindings,
            visiting,
        ),
        name: param.name,
    }
}

fn resolve_rbs_method_alias_type(
    rbs_type: &rbs_ir::RbsType,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
    type_bindings: &HashMap<String, rbs_ir::RbsType>,
    visiting: &mut HashSet<String>,
) -> rbs_ir::RbsType {
    match rbs_type {
        rbs_ir::RbsType::Variable(name) => type_bindings
            .get(name.as_str())
            .cloned()
            .unwrap_or_else(|| rbs_type.clone()),
        rbs_ir::RbsType::Alias(name, args) => {
            if let Some(qualified) =
                resolve_alias_reference_name(name.as_str(), current_scope, |candidate| {
                    alias_templates.contains_key(candidate) || type_aliases.contains_key(candidate)
                })
            {
                if let Some(raw_alias) = alias_templates.get(&qualified) {
                    return instantiate_rbs_method_alias_type(
                        &qualified,
                        raw_alias,
                        args,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    );
                }
                if args.is_empty()
                    && let Some(alias_type) = type_aliases.get(&qualified)
                    && let Some(rbs_alias_type) = type_to_rbs_type(alias_type)
                {
                    return rbs_alias_type;
                }
            }
            rbs_ir::RbsType::Alias(
                *name,
                args.iter()
                    .map(|arg| {
                        resolve_rbs_method_alias_type(
                            arg,
                            type_aliases,
                            alias_templates,
                            current_scope,
                            type_bindings,
                            visiting,
                        )
                    })
                    .collect(),
            )
        }
        rbs_ir::RbsType::Class(name, args) => {
            if let Some(qualified) =
                resolve_alias_reference_name(name.as_str(), current_scope, |candidate| {
                    alias_templates.contains_key(candidate) || type_aliases.contains_key(candidate)
                })
            {
                if let Some(raw_alias) = alias_templates.get(&qualified) {
                    return instantiate_rbs_method_alias_type(
                        &qualified,
                        raw_alias,
                        args,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    );
                }
                if args.is_empty()
                    && let Some(alias_type) = type_aliases.get(&qualified)
                    && let Some(rbs_alias_type) = type_to_rbs_type(alias_type)
                {
                    return rbs_alias_type;
                }
            }
            rbs_ir::RbsType::Class(
                *name,
                args.iter()
                    .map(|arg| {
                        resolve_rbs_method_alias_type(
                            arg,
                            type_aliases,
                            alias_templates,
                            current_scope,
                            type_bindings,
                            visiting,
                        )
                    })
                    .collect(),
            )
        }
        rbs_ir::RbsType::Union(types) => rbs_ir::RbsType::Union(
            types
                .iter()
                .map(|ty| {
                    resolve_rbs_method_alias_type(
                        ty,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    )
                })
                .collect(),
        ),
        rbs_ir::RbsType::Intersection(types) => rbs_ir::RbsType::Intersection(
            types
                .iter()
                .map(|ty| {
                    resolve_rbs_method_alias_type(
                        ty,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    )
                })
                .collect(),
        ),
        rbs_ir::RbsType::Optional(inner) => {
            rbs_ir::RbsType::Optional(Box::new(resolve_rbs_method_alias_type(
                inner,
                type_aliases,
                alias_templates,
                current_scope,
                type_bindings,
                visiting,
            )))
        }
        rbs_ir::RbsType::Tuple(types) => rbs_ir::RbsType::Tuple(
            types
                .iter()
                .map(|ty| {
                    resolve_rbs_method_alias_type(
                        ty,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    )
                })
                .collect(),
        ),
        rbs_ir::RbsType::Record(fields) => rbs_ir::RbsType::Record(
            fields
                .iter()
                .map(|field| rbs_ir::RbsRecordField {
                    key: field.key.clone(),
                    type_: resolve_rbs_method_alias_type(
                        &field.type_,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    ),
                    required: field.required,
                })
                .collect(),
        ),
        rbs_ir::RbsType::Proc(method_type) => {
            rbs_ir::RbsType::Proc(Box::new(resolve_method_type_aliases(
                method_type,
                type_aliases,
                alias_templates,
                current_scope,
                type_bindings,
                visiting,
            )))
        }
        _ => rbs_type.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn instantiate_rbs_method_alias_type(
    alias_name: &str,
    raw_alias: &RawTypeAlias,
    args: &[rbs_ir::RbsType],
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
    outer_bindings: &HashMap<String, rbs_ir::RbsType>,
    visiting: &mut HashSet<String>,
) -> rbs_ir::RbsType {
    if !visiting.insert(alias_name.to_string()) {
        return rbs_ir::RbsType::Untyped;
    }
    let mut local_bindings = outer_bindings.clone();
    for (index, param) in raw_alias.type_params.iter().enumerate() {
        let arg_type = if let Some(arg) = args.get(index) {
            resolve_rbs_method_alias_type(
                arg,
                type_aliases,
                alias_templates,
                current_scope,
                outer_bindings,
                visiting,
            )
        } else if let Some(default_or_bound) =
            raw_alias_type_param_default_or_bound(raw_alias, param)
        {
            resolve_rbs_method_alias_type(
                default_or_bound,
                type_aliases,
                alias_templates,
                current_scope,
                &local_bindings,
                visiting,
            )
        } else {
            rbs_ir::RbsType::Untyped
        };
        local_bindings.insert(param.clone(), arg_type);
    }

    let alias_scope = alias_name.rsplit_once("::").map(|(scope, _)| scope);
    let resolved = resolve_rbs_method_alias_type(
        &raw_alias.type_,
        type_aliases,
        alias_templates,
        alias_scope.or(current_scope),
        &local_bindings,
        visiting,
    );
    visiting.remove(alias_name);
    resolved
}

pub(crate) fn type_to_rbs_type(ty: &Type) -> Option<rbs_ir::RbsType> {
    match ty {
        Type::Integer => Some(rbs_ir::RbsType::Integer),
        Type::Float => Some(rbs_ir::RbsType::Float),
        Type::String => Some(rbs_ir::RbsType::String),
        Type::Symbol => Some(rbs_ir::RbsType::Symbol),
        Type::Bool | Type::True | Type::False => Some(rbs_ir::RbsType::Bool),
        Type::Nil => Some(rbs_ir::RbsType::Nil),
        Type::Untyped => Some(rbs_ir::RbsType::Untyped),
        Type::Void => Some(rbs_ir::RbsType::Void),
        Type::Top => Some(rbs_ir::RbsType::Top),
        Type::Bot => Some(rbs_ir::RbsType::Bottom),
        Type::LiteralInteger(value) => Some(rbs_ir::RbsType::Literal(value.to_string().into())),
        Type::LiteralFloat(value) => Some(rbs_ir::RbsType::Literal(value.as_str().into())),
        Type::LiteralString(value) => Some(rbs_ir::RbsType::Literal(format!("{value:?}").into())),
        Type::LiteralSymbol(value) => Some(rbs_ir::RbsType::Literal(format!(":{value}").into())),
        Type::Array(Some(inner)) => Some(rbs_ir::RbsType::Class(
            Sym::new("Array"),
            Box::new([type_to_rbs_type(inner)?]),
        )),
        Type::Array(None) => Some(rbs_ir::RbsType::Class(Sym::new("Array"), Box::default())),
        Type::Hash(Some(key), Some(value)) => Some(rbs_ir::RbsType::Class(
            Sym::new("Hash"),
            Box::new([type_to_rbs_type(key)?, type_to_rbs_type(value)?]),
        )),
        Type::Hash(None, None) => Some(rbs_ir::RbsType::Class(Sym::new("Hash"), Box::default())),
        Type::Union(types) => Some(rbs_ir::RbsType::Union(
            types.iter().filter_map(type_to_rbs_type).collect(),
        )),
        Type::Intersection(types) => Some(rbs_ir::RbsType::Intersection(
            types.iter().filter_map(type_to_rbs_type).collect(),
        )),
        Type::Tuple(types) => Some(rbs_ir::RbsType::Tuple(
            types.iter().filter_map(type_to_rbs_type).collect(),
        )),
        Type::Record(fields) => {
            let rbs_fields = fields
                .iter()
                .map(|field| {
                    Some(rbs_ir::RbsRecordField {
                        key: match &field.key {
                            RecordKey::Symbol(name) => {
                                rbs_ir::RbsRecordKey::Symbol(name.as_str().into())
                            }
                            RecordKey::String(name) => {
                                rbs_ir::RbsRecordKey::String(name.as_str().into())
                            }
                        },
                        type_: type_to_rbs_type(&field.value)?,
                        required: !field.optional,
                    })
                })
                .collect::<Option<Box<[_]>>>()?;
            Some(rbs_ir::RbsType::Record(rbs_fields))
        }
        Type::Class(name) if !name.contains('[') => {
            Some(rbs_ir::RbsType::Class(*name, Box::default()))
        }
        Type::Class(name) => rbs_sys::parse_type(name)
            .ok()
            .map(|parsed| rbs_ir::RbsType::from(&parsed)),
        Type::Generic { base, args } => {
            let rbs_args = args
                .iter()
                .map(type_to_rbs_type)
                .collect::<Option<Box<[_]>>>()?;
            Some(rbs_ir::RbsType::Class(*base, rbs_args))
        }
        Type::Singleton(name) => Some(rbs_ir::RbsType::Singleton(*name)),
        _ => None,
    }
}

fn collect_rbs_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for path in paths {
        if path.is_file() && path.extension().is_some_and(|ext| ext == "rbs") {
            result.push(path.clone());
        } else if path.is_dir() {
            collect_rbs_files_parallel(path, &mut result);
        }
    }
    // Sorts the parallel walk's results to make ordering deterministic (avoids platform/thread-order dependence).
    result.sort();
    result
}

/// Parallel directory walk for `.rbs` files (on large Rails apps, a sequential walk of non-rbs subtrees dominates init time).
fn collect_rbs_files_parallel(dir: &Path, result: &mut Vec<PathBuf>) {
    fn walk(dir: &Path) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let (files, subdirs): (Vec<_>, Vec<_>) = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .partition(|path| path.is_file());
        let mut found: Vec<PathBuf> = files
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "rbs"))
            .collect();
        let nested: Vec<Vec<PathBuf>> = subdirs
            .par_iter()
            .filter(|path| !should_skip_dir(path))
            .map(|path| walk(path))
            .collect();
        for items in nested {
            found.extend(items);
        }
        found
    }
    result.extend(walk(dir));
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(should_skip_dir_name)
}

fn should_skip_dir_name(name: &str) -> bool {
    matches!(
        name,
        "vendor" | "target" | "node_modules" | ".git" | ".bundle" | "tmp" | "log"
    )
}

fn add_methods_to_registry(
    registry: &mut TypeRegistry,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    class_name: &str,
    methods: &[rbs_sys::MethodDecl],
) {
    for method in methods {
        // Converts the parse result to compact IR here, and works only with IR from now on
        // (the registry keeps only the IR too).
        let ir_method_types = rbs_ir::method_types_from_rbs(&method.method_types);
        let first_overload = match ir_method_types.first() {
            Some(method_type) => method_type,
            None => continue,
        };
        let callable_without_block = method_type_callable_without_block(&ir_method_types);

        let return_type = callable_without_block
            .map(|method_type| {
                let mut return_type = convert_imported_rbs_type_with_templates(
                    &method_type.function_type.return_type,
                    type_aliases,
                    alias_templates,
                    Some(class_name),
                );
                if return_type != Type::Untyped
                    && method_type
                        .annotations
                        .iter()
                        .any(|a| a == "implicitly-returns-nil")
                {
                    return_type = return_type.union_with(Type::Nil);
                }
                return_type
            })
            .unwrap_or(Type::Untyped);

        let params = convert_function_params_with_templates(
            &first_overload.function_type,
            type_aliases,
            alias_templates,
            Some(class_name),
        );
        let kinds: &[bool] = match method.kind {
            rbs_sys::MethodKind::Singleton => &[true],
            rbs_sys::MethodKind::Instance => &[false],
            rbs_sys::MethodKind::SingletonInstance => &[false, true],
        };

        let mut param_infos = Vec::new();
        for (index, (name, kind, ty)) in params.into_iter().enumerate() {
            // annotated_params is keyed by (name, is_singleton), so write the same param type
            // to every side that method.kind covers.
            for &is_singleton in kinds {
                registry.set_annotated_param_type(
                    class_name,
                    &method.name,
                    is_singleton,
                    index,
                    ty.clone(),
                );
            }
            param_infos.push(ParamInfo {
                name,
                kind,
                default_type: None,
            });
        }

        let rbs_method_types = std::sync::Arc::new(resolve_method_types_aliases(
            &ir_method_types,
            type_aliases,
            alias_templates,
            Some(class_name),
        ));

        for &is_singleton in kinds {
            registry.add_method_def(
                class_name,
                MethodDef {
                    name: Sym::new(&method.name),
                    param_infos: param_infos.clone(),
                    raw_return_type: return_type.clone(),
                    sorbet_modifier_comments: Vec::new(),
                    rbs_annotated: true,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    attr_ivar: method.attr_ivar.clone(),
                    is_singleton,
                    rbs_file_source: true,
                    synthetic_dsl_source: false,
                    rbs_method_types: std::sync::Arc::clone(&rbs_method_types),
                    extra_overloads: Vec::new(),
                    loc: None,
                },
            );
        }
    }
}

fn method_type_callable_without_block(
    method_types: &[rbs_ir::MethodType],
) -> Option<&rbs_ir::MethodType> {
    method_types.iter().find(|method_type| {
        method_type
            .block
            .as_ref()
            .is_none_or(|block| !block.required)
    })
}

fn extract_type_aliases(sig: &rbs_sys::Signature) -> HashMap<String, RawTypeAlias> {
    sig.declarations
        .iter()
        .filter_map(|decl| match decl {
            rbs_sys::Declaration::TypeAlias {
                name,
                type_params,
                type_param_bounds,
                type_param_defaults,
                type_,
            } => Some((
                name.clone(),
                RawTypeAlias {
                    type_params: type_params.clone(),
                    type_param_bounds: type_param_bounds
                        .iter()
                        .map(|(name, ty)| (name.clone(), rbs_ir::RbsType::from(ty)))
                        .collect(),
                    type_param_defaults: type_param_defaults
                        .iter()
                        .map(|(name, ty)| (name.clone(), rbs_ir::RbsType::from(ty)))
                        .collect(),
                    type_: rbs_ir::RbsType::from(type_),
                },
            )),
            rbs_sys::Declaration::ClassAlias { new_name, old_name }
            | rbs_sys::Declaration::ModuleAlias { new_name, old_name } => Some((
                new_name.clone(),
                RawTypeAlias {
                    type_params: Vec::new(),
                    type_param_bounds: Vec::new(),
                    type_param_defaults: Vec::new(),
                    type_: rbs_ir::RbsType::Class(Sym::new(old_name), Box::default()),
                },
            )),
            _ => None,
        })
        .collect()
}

fn resolve_pending_alias_type(
    alias_name: &str,
    raw_aliases: &HashMap<String, RawTypeAlias>,
    resolved_aliases: &HashMap<String, Type>,
    visiting: &mut HashSet<String>,
    existing_aliases: &HashMap<String, Type>,
    type_bindings: &HashMap<String, Type>,
) -> Type {
    if type_bindings.is_empty()
        && let Some(ty) = resolved_aliases.get(alias_name)
    {
        return ty.clone();
    }
    if !visiting.insert(alias_name.to_string()) {
        // Keep a finite symbolic edge for recursive aliases. Collapsing the edge
        // to `untyped` loses the alias's shape and makes every recursive member
        // less precise. `Type::Generic` is already a finite, hashable type node;
        // an empty argument list is rendered as the bare alias name.
        return Type::Generic {
            base: Sym::new(alias_name),
            args: Box::default(),
        };
    }
    let Some(raw_alias) = raw_aliases.get(alias_name) else {
        visiting.remove(alias_name);
        return existing_aliases
            .get(alias_name)
            .cloned()
            .unwrap_or(Type::Untyped);
    };
    let current_scope = alias_name.rsplit_once("::").map(|(scope, _)| scope);
    let mut local_bindings = type_bindings.clone();
    for param in &raw_alias.type_params {
        if local_bindings.contains_key(param) {
            continue;
        }
        let ty = raw_alias_type_param_default_or_bound(raw_alias, param)
            .map(|default_or_bound| {
                convert_pending_rbs_type(
                    default_or_bound,
                    raw_aliases,
                    resolved_aliases,
                    visiting,
                    existing_aliases,
                    current_scope,
                    &local_bindings,
                )
            })
            .unwrap_or(Type::Untyped);
        local_bindings.insert(param.clone(), ty);
    }
    let ty = convert_pending_rbs_type(
        &raw_alias.type_,
        raw_aliases,
        resolved_aliases,
        visiting,
        existing_aliases,
        current_scope,
        &local_bindings,
    );
    visiting.remove(alias_name);
    ty
}

fn convert_pending_rbs_type(
    rbs_ty: &rbs_ir::RbsType,
    raw_aliases: &HashMap<String, RawTypeAlias>,
    resolved_aliases: &HashMap<String, Type>,
    visiting: &mut HashSet<String>,
    existing_aliases: &HashMap<String, Type>,
    current_scope: Option<&str>,
    type_bindings: &HashMap<String, Type>,
) -> Type {
    match rbs_ty {
        rbs_ir::RbsType::Variable(name) => type_bindings
            .get(name.as_str())
            .cloned()
            .unwrap_or(Type::Untyped),
        rbs_ir::RbsType::Alias(name, args) => {
            if let Some(ty) = convert_rbs_builtin_alias(name.as_str(), args, |arg| {
                convert_pending_rbs_type(
                    arg,
                    raw_aliases,
                    resolved_aliases,
                    visiting,
                    existing_aliases,
                    current_scope,
                    type_bindings,
                )
            }) {
                return ty;
            }
            if let Some(qualified) =
                resolve_alias_reference_name(name.as_str(), current_scope, |candidate| {
                    raw_aliases.contains_key(candidate)
                        || resolved_aliases.contains_key(candidate)
                        || existing_aliases.contains_key(candidate)
                })
            {
                if args.is_empty()
                    && let Some(ty) = resolved_aliases.get(&qualified)
                {
                    return ty.clone();
                }
                if let Some(raw_alias) = raw_aliases.get(&qualified) {
                    return instantiate_alias_type(
                        &qualified,
                        raw_alias,
                        args,
                        raw_aliases,
                        resolved_aliases,
                        visiting,
                        existing_aliases,
                        current_scope,
                        type_bindings,
                    );
                }
            }
            Type::Class(Sym::new(name.trim_start_matches("::")))
        }
        rbs_ir::RbsType::Class(_, _)
        | rbs_ir::RbsType::Singleton(_)
        | rbs_ir::RbsType::Literal(_)
        | rbs_ir::RbsType::Integer
        | rbs_ir::RbsType::Float
        | rbs_ir::RbsType::String
        | rbs_ir::RbsType::Symbol
        | rbs_ir::RbsType::Bool
        | rbs_ir::RbsType::Nil
        | rbs_ir::RbsType::Void
        | rbs_ir::RbsType::Untyped
        | rbs_ir::RbsType::Top
        | rbs_ir::RbsType::Bottom
        | rbs_ir::RbsType::SelfType
        | rbs_ir::RbsType::ClassType
        | rbs_ir::RbsType::InstanceType => convert_imported_rbs_type_inner(
            rbs_ty,
            resolved_aliases,
            raw_aliases,
            current_scope,
            type_bindings,
            visiting,
        ),
        rbs_ir::RbsType::Union(types) => Type::from_type_vec_preserve_untyped(
            types
                .iter()
                .map(|ty| {
                    convert_pending_rbs_type(
                        ty,
                        raw_aliases,
                        resolved_aliases,
                        visiting,
                        existing_aliases,
                        current_scope,
                        type_bindings,
                    )
                })
                .collect(),
        ),
        rbs_ir::RbsType::Intersection(types) => Type::Intersection(
            types
                .iter()
                .map(|ty| {
                    convert_pending_rbs_type(
                        ty,
                        raw_aliases,
                        resolved_aliases,
                        visiting,
                        existing_aliases,
                        current_scope,
                        type_bindings,
                    )
                })
                .collect(),
        ),
        rbs_ir::RbsType::Optional(inner) => convert_pending_rbs_type(
            inner,
            raw_aliases,
            resolved_aliases,
            visiting,
            existing_aliases,
            current_scope,
            type_bindings,
        )
        .union_with(Type::Nil),
        rbs_ir::RbsType::Tuple(types) => Type::Tuple(
            types
                .iter()
                .map(|ty| {
                    convert_pending_rbs_type(
                        ty,
                        raw_aliases,
                        resolved_aliases,
                        visiting,
                        existing_aliases,
                        current_scope,
                        type_bindings,
                    )
                })
                .collect(),
        ),
        rbs_ir::RbsType::Record(fields) => Type::Record(
            fields
                .iter()
                .map(|field| RecordField {
                    key: match &field.key {
                        rbs_ir::RbsRecordKey::Symbol(name) => RecordKey::Symbol(name.to_string()),
                        rbs_ir::RbsRecordKey::String(name) => RecordKey::String(name.to_string()),
                    },
                    value: convert_pending_rbs_type(
                        &field.type_,
                        raw_aliases,
                        resolved_aliases,
                        visiting,
                        existing_aliases,
                        current_scope,
                        type_bindings,
                    ),
                    optional: !field.required,
                })
                .collect(),
        ),
        rbs_ir::RbsType::Proc(method_type) => Type::Proc {
            return_type: Box::new(proc_return_type_with_self(
                convert_pending_rbs_type(
                    &method_type.function_type.return_type,
                    raw_aliases,
                    resolved_aliases,
                    visiting,
                    existing_aliases,
                    current_scope,
                    type_bindings,
                ),
                method_type.self_type.as_ref().map(|self_type| {
                    convert_pending_rbs_type(
                        self_type,
                        raw_aliases,
                        resolved_aliases,
                        visiting,
                        existing_aliases,
                        current_scope,
                        type_bindings,
                    )
                }),
            )),
            param_count: rbs_function_type_param_count(&method_type.function_type),
        },
    }
}

fn proc_return_type_with_self(return_type: Type, self_type: Option<Type>) -> Type {
    match self_type {
        Some(self_type) if self_type != Type::Untyped => return_type.replace_self_type(&self_type),
        _ => return_type,
    }
}

#[allow(clippy::too_many_arguments)]
fn instantiate_alias_type(
    alias_name: &str,
    raw_alias: &RawTypeAlias,
    args: &[rbs_ir::RbsType],
    raw_aliases: &HashMap<String, RawTypeAlias>,
    resolved_aliases: &HashMap<String, Type>,
    visiting: &mut HashSet<String>,
    existing_aliases: &HashMap<String, Type>,
    current_scope: Option<&str>,
    outer_bindings: &HashMap<String, Type>,
) -> Type {
    if !visiting.insert(alias_name.to_string()) {
        let converted_args: Vec<Type> = args
            .iter()
            .map(|arg| {
                convert_pending_rbs_type(
                    arg,
                    raw_aliases,
                    resolved_aliases,
                    visiting,
                    existing_aliases,
                    current_scope,
                    outer_bindings,
                )
            })
            .collect();
        return Type::Generic {
            base: Sym::new(alias_name),
            args: converted_args.into_boxed_slice(),
        };
    }
    let mut local_bindings = outer_bindings.clone();
    for (index, param) in raw_alias.type_params.iter().enumerate() {
        let arg_ty = if let Some(arg) = args.get(index) {
            convert_pending_rbs_type(
                arg,
                raw_aliases,
                resolved_aliases,
                visiting,
                existing_aliases,
                current_scope,
                outer_bindings,
            )
        } else if let Some(default_or_bound) =
            raw_alias_type_param_default_or_bound(raw_alias, param)
        {
            convert_pending_rbs_type(
                default_or_bound,
                raw_aliases,
                resolved_aliases,
                visiting,
                existing_aliases,
                current_scope,
                &local_bindings,
            )
        } else {
            Type::Untyped
        };
        local_bindings.insert(param.clone(), arg_ty);
    }
    let alias_scope = alias_name.rsplit_once("::").map(|(scope, _)| scope);
    let ty = convert_pending_rbs_type(
        &raw_alias.type_,
        raw_aliases,
        resolved_aliases,
        visiting,
        existing_aliases,
        alias_scope.or(current_scope),
        &local_bindings,
    );
    visiting.remove(alias_name);
    ty
}

fn convert_imported_rbs_type_with_templates(
    rbs_ty: &rbs_ir::RbsType,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
) -> Type {
    let mut visiting = HashSet::new();
    convert_imported_rbs_type_inner(
        rbs_ty,
        type_aliases,
        alias_templates,
        current_scope,
        &HashMap::new(),
        &mut visiting,
    )
}

pub(crate) fn convert_imported_rbs_type(
    rbs_ty: &rbs_ir::RbsType,
    type_aliases: &HashMap<String, Type>,
    current_scope: Option<&str>,
) -> Type {
    convert_imported_rbs_type_with_templates(rbs_ty, type_aliases, &HashMap::new(), current_scope)
}

fn convert_imported_rbs_type_inner(
    rbs_ty: &rbs_ir::RbsType,
    type_aliases: &HashMap<String, Type>,
    alias_templates: &HashMap<String, RawTypeAlias>,
    current_scope: Option<&str>,
    type_bindings: &HashMap<String, Type>,
    visiting: &mut HashSet<String>,
) -> Type {
    match rbs_ty {
        rbs_ir::RbsType::InstanceType => {
            // `instance` is left as-is for lazy resolution -> becomes the receiver at the call site (an owner instance when rendered, byte-compatible).
            Type::InstanceType
        }
        rbs_ir::RbsType::ClassType => current_scope
            .map(|scope| Type::Singleton(Sym::new(scope)))
            .unwrap_or_else(|| Type::Class(Sym::new("Class"))),
        rbs_ir::RbsType::Variable(name) => type_bindings
            .get(name.as_str())
            .cloned()
            .unwrap_or(Type::Untyped),
        rbs_ir::RbsType::Alias(name, args) => {
            if let Some(ty) = convert_rbs_builtin_alias(name.as_str(), args, |arg| {
                convert_imported_rbs_type_inner(
                    arg,
                    type_aliases,
                    alias_templates,
                    current_scope,
                    type_bindings,
                    visiting,
                )
            }) {
                return ty;
            }
            if let Some(qualified) =
                resolve_alias_reference_name(name.as_str(), current_scope, |candidate| {
                    alias_templates.contains_key(candidate) || type_aliases.contains_key(candidate)
                })
            {
                if let Some(raw_alias) = alias_templates.get(&qualified) {
                    return instantiate_alias_type(
                        &qualified,
                        raw_alias,
                        args,
                        alias_templates,
                        type_aliases,
                        visiting,
                        type_aliases,
                        current_scope,
                        type_bindings,
                    );
                }
                if let Some(ty) = type_aliases.get(&qualified) {
                    return ty.clone();
                }
            }
            Type::Class(Sym::new(name.trim_start_matches("::")))
        }
        rbs_ir::RbsType::Class(name, args) => {
            let bare = name.strip_prefix("::").unwrap_or(name);
            if let Some(qualified) =
                resolve_alias_reference_name(name.as_str(), current_scope, |candidate| {
                    alias_templates.contains_key(candidate) || type_aliases.contains_key(candidate)
                })
            {
                if let Some(raw_alias) = alias_templates.get(&qualified) {
                    return instantiate_alias_type(
                        &qualified,
                        raw_alias,
                        args,
                        alias_templates,
                        type_aliases,
                        visiting,
                        type_aliases,
                        current_scope,
                        type_bindings,
                    );
                }
                if args.is_empty()
                    && let Some(alias_type) = type_aliases.get(&qualified)
                {
                    return alias_type.clone();
                }
            }
            if args.is_empty()
                && let Some(alias_type) = type_aliases.get(bare)
            {
                return alias_type.clone();
            }
            match bare {
                "Integer" => Type::Integer,
                "Float" => Type::Float,
                "String" => Type::String,
                "Symbol" => Type::Symbol,
                "NilClass" if args.is_empty() => Type::Nil,
                "TrueClass" if args.is_empty() => Type::True,
                "FalseClass" if args.is_empty() => Type::False,
                "Array" if args.is_empty() => Type::Array(None),
                "Array" if args.len() == 1 => {
                    Type::Array(Some(Box::new(convert_imported_rbs_type_inner(
                        &args[0],
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    ))))
                }
                "Hash" if args.is_empty() => Type::Hash(None, None),
                "Hash" if args.len() == 2 => Type::Hash(
                    Some(Box::new(convert_imported_rbs_type_inner(
                        &args[0],
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    ))),
                    Some(Box::new(convert_imported_rbs_type_inner(
                        &args[1],
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    ))),
                ),
                _ if args.is_empty() => Type::Class(Sym::new(bare)),
                _ => Type::Generic {
                    base: Sym::new(bare),
                    args: args
                        .iter()
                        .map(|arg| {
                            convert_imported_rbs_type_inner(
                                arg,
                                type_aliases,
                                alias_templates,
                                current_scope,
                                type_bindings,
                                visiting,
                            )
                        })
                        .collect(),
                },
            }
        }
        rbs_ir::RbsType::Union(types) => Type::from_type_vec_preserve_untyped(
            types
                .iter()
                .map(|ty| {
                    convert_imported_rbs_type_inner(
                        ty,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    )
                })
                .collect(),
        ),
        rbs_ir::RbsType::Optional(inner) => {
            let inner_ty = convert_imported_rbs_type_inner(
                inner,
                type_aliases,
                alias_templates,
                current_scope,
                type_bindings,
                visiting,
            );
            if inner_ty == Type::Untyped {
                Type::Untyped
            } else {
                inner_ty.union_with(Type::Nil)
            }
        }
        rbs_ir::RbsType::Tuple(types) => Type::Tuple(
            types
                .iter()
                .map(|ty| {
                    convert_imported_rbs_type_inner(
                        ty,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    )
                })
                .collect(),
        ),
        rbs_ir::RbsType::Record(fields) => Type::Record(
            fields
                .iter()
                .map(|field| RecordField {
                    key: match &field.key {
                        rbs_ir::RbsRecordKey::Symbol(name) => RecordKey::Symbol(name.to_string()),
                        rbs_ir::RbsRecordKey::String(name) => RecordKey::String(name.to_string()),
                    },
                    value: convert_imported_rbs_type_inner(
                        &field.type_,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    ),
                    optional: !field.required,
                })
                .collect(),
        ),
        rbs_ir::RbsType::Proc(method_type) => Type::Proc {
            return_type: Box::new(proc_return_type_with_self(
                convert_imported_rbs_type_inner(
                    &method_type.function_type.return_type,
                    type_aliases,
                    alias_templates,
                    current_scope,
                    type_bindings,
                    visiting,
                ),
                method_type.self_type.as_ref().map(|self_type| {
                    convert_imported_rbs_type_inner(
                        self_type,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    )
                }),
            )),
            param_count: rbs_function_type_param_count(&method_type.function_type),
        },
        rbs_ir::RbsType::Intersection(types) => Type::Intersection(
            types
                .iter()
                .map(|ty| {
                    convert_imported_rbs_type_inner(
                        ty,
                        type_aliases,
                        alias_templates,
                        current_scope,
                        type_bindings,
                        visiting,
                    )
                })
                .collect(),
        ),
        _ => convert_rbs_type(rbs_ty),
    }
}

fn rbs_function_type_param_count(ft: &rbs_ir::FunctionType) -> usize {
    ft.required_positionals.len()
        + ft.optional_positionals.len()
        + usize::from(ft.rest_positionals.is_some())
        + ft.trailing_positionals.len()
        + ft.required_keywords.len()
        + ft.optional_keywords.len()
        + usize::from(ft.rest_keywords.is_some())
}

fn resolve_alias_reference_name<F>(
    alias_name: &str,
    current_scope: Option<&str>,
    has_alias: F,
) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    let bare = alias_name.trim();
    if bare.starts_with("::") {
        let global = bare.trim_start_matches("::").to_string();
        return has_alias(&global).then_some(global);
    }
    if bare.contains("::") {
        let qualified = bare.to_string();
        return has_alias(&qualified).then_some(qualified);
    }

    let mut scopes = Vec::new();
    let mut current = current_scope;
    while let Some(scope) = current {
        scopes.push(format!("{scope}::{bare}"));
        current = scope.rsplit_once("::").map(|(parent, _)| parent);
    }
    scopes.push(bare.to_string());

    scopes.into_iter().find(|candidate| has_alias(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Conformance audit against an external RBS tree (`TYDA_RBS_SWEEP_ROOT`, opt-in via `--ignored`).
    #[test]
    #[ignore]
    fn audit_external_rbs_tree() {
        let root = std::env::var("TYDA_RBS_SWEEP_ROOT")
            .expect("set TYDA_RBS_SWEEP_ROOT to the RBS tree root");
        let mut files = Vec::new();
        collect_rbs_files_under(std::path::Path::new(&root), &mut files);
        files.sort();
        let mut parse_failed = Vec::new();
        let mut zero_decls = Vec::new();
        let mut import_panicked = Vec::new();
        let mut ok = 0usize;
        for path in &files {
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            let declares_something = content.lines().any(|line| {
                let t = line.trim_start();
                t.starts_with("class ")
                    || t.starts_with("module ")
                    || t.starts_with("interface ")
                    || t.starts_with("type ")
            });
            let rel = path.to_string_lossy().into_owned();
            match rbs_sys::parse_signature(&content) {
                Err(_) => parse_failed.push(rel),
                Ok(sig) => {
                    if declares_something && sig.declarations.is_empty() {
                        zero_decls.push(rel);
                    } else {
                        ok += 1;
                    }
                    let body = content.clone();
                    let caught = std::panic::catch_unwind(move || {
                        let mut registry = TypeRegistry::new();
                        load_rbs_string(&body, &mut registry);
                    });
                    if caught.is_err() {
                        import_panicked.push(path.to_string_lossy().into_owned());
                    }
                }
            }
        }
        eprintln!(
            "RBS audit: {} files, {} ok, {} parse_failed, {} zero_decls, {} import_panicked",
            files.len(),
            ok,
            parse_failed.len(),
            zero_decls.len(),
            import_panicked.len()
        );
        for f in parse_failed
            .iter()
            .chain(&zero_decls)
            .chain(&import_panicked)
        {
            eprintln!("  PROBLEM {f}");
        }
        assert!(
            parse_failed.is_empty() && zero_decls.is_empty() && import_panicked.is_empty(),
            "RBS audit found {} parse failures, {} zero-decl files, {} import panics",
            parse_failed.len(),
            zero_decls.len(),
            import_panicked.len()
        );
    }

    fn collect_rbs_files_under(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rbs_files_under(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rbs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn skip_dir_name_matches_common_large_dirs() {
        assert!(should_skip_dir_name("vendor"));
        assert!(should_skip_dir_name("node_modules"));
        assert!(!should_skip_dir_name("sig"));
    }

    #[test]
    fn load_rbs_string_reads_zip_method() {
        let mut registry = TypeRegistry::new();
        load_rbs_string(
            "class Array\n  def zip: (Array[untyped] other, *Array[untyped] others) -> Symbol\nend\n",
            &mut registry,
        );

        assert_eq!(
            registry.lookup_method_return_type("Array", "zip"),
            Some(Type::Symbol)
        );
    }

    #[test]
    fn load_rbs_string_records_class_type_param_defaults() {
        let mut registry = TypeRegistry::new();
        load_rbs_string(
            "class Box[T < String, U = String]\n  def value: -> U\nend\n",
            &mut registry,
        );

        assert_eq!(
            registry.get_class_type_param_bounds("Box"),
            &[(
                "T".to_string(),
                rbs_ir::RbsType::Class(Sym::new("String"), Box::default())
            )]
        );
        assert_eq!(
            registry.get_class_type_param_defaults("Box"),
            &[("U".to_string(), Type::String)]
        );
    }

    #[test]
    fn load_rbs_string_records_inheritance_type_args() {
        let mut registry = TypeRegistry::new();
        load_rbs_string(
            "module Readable[T]\nend\n\
             class Child[T] < Parent[T]\n  include Readable[String]\nend\n",
            &mut registry,
        );

        assert_eq!(
            registry.get_superclass_type_args("Child"),
            &[rbs_ir::RbsType::Variable(Sym::new("T"))]
        );
        let mixins = &registry.class_data_for("Child").expect("Child").mixins;
        assert_eq!(mixins.len(), 1);
        assert_eq!(
            mixins[0].type_args,
            vec![rbs_ir::RbsType::Class(Sym::new("String"), Box::default())]
        );
    }

    #[test]
    fn load_rbs_string_records_module_self_type_args() {
        let mut registry = TypeRegistry::new();
        load_rbs_string(
            "interface _Readable[T]\n  def read: -> T\nend\n\
             module Reader[Elem] : _Readable[Elem]\nend\n",
            &mut registry,
        );

        let data = registry.class_data_for("Reader").expect("Reader data");
        let cold = data.cold();
        assert_eq!(cold.required_ancestors.len(), 1);
        assert_eq!(cold.required_ancestors[0].as_ref(), "_Readable");
        assert_eq!(
            cold.required_ancestor_type_args,
            vec![(
                cold.required_ancestors[0].clone(),
                vec![rbs_ir::RbsType::Variable(Sym::new("Elem"))]
            )]
        );
    }
}
