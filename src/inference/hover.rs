use super::plugins::known_class_body_dsl_method;
use super::*;
use crate::rbs::display::{
    format_hover_callable_type, format_hover_inferred_method_sig, format_hover_method_sig,
};
use crate::rbs::ir as rbs_ir;
use crate::types::Sym;
use crate::types::{HoverBlockSig, HoverOverloadSig, Param, ParamKind};
use std::collections::{BTreeSet, HashSet};

#[derive(Clone)]
pub(crate) enum HoverTarget {
    Value(Type),
    MethodCall {
        receiver_type: Type,
        result_type: Type,
    },
    MethodDefinition {
        owner_type: Type,
        is_singleton: bool,
    },
}

#[derive(Clone)]
pub(crate) struct HoverSnapshot {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) name: String,
    pub(crate) target: HoverTarget,
    pub(crate) class_context: String,
    pub(crate) method_context: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ArgCheckArg {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) ty: Type,
    pub(crate) keyword: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ArgCheckSite {
    pub(crate) receiver_type: Type,
    pub(crate) method_name: String,
    pub(crate) is_singleton: bool,
    pub(crate) class_context: String,
    pub(crate) method_context: Option<String>,
    pub(crate) args: Vec<ArgCheckArg>,
    pub(crate) positional_count: usize,
    pub(crate) has_pos_splat: bool,
    pub(crate) keyword_names: Vec<String>,
    pub(crate) has_kwsplat: bool,
    pub(crate) call_start: usize,
    pub(crate) call_end: usize,
}

#[derive(Clone)]
pub(crate) struct UnresolvedConstantSite {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) name: String,
    pub(crate) class_context: String,
}

// Only reports `No`; `Unknown` stays silent to avoid false positives.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ArgCompat {
    Yes,
    No,
    Unknown,
}

impl<'a> InferenceEngine<'a> {
    pub(super) fn unresolved_method_calls_for_snapshots(&mut self) -> Vec<UnresolvedMethodCall> {
        let snapshots = std::mem::take(&mut self.var_snapshots);
        let mut seen = BTreeSet::new();
        let mut calls = Vec::new();
        // Ancestor walks are memoized locally to this pass (a persistent memo could go stale while the replay registry is lazy-merging).
        let mut unknown_surface_cache: std::collections::HashMap<String, bool> =
            std::collections::HashMap::new();
        let mut ancestor_complete_cache: std::collections::HashMap<String, bool> =
            std::collections::HashMap::new();
        for snap in snapshots {
            if snap.start >= snap.end {
                continue;
            }
            let HoverTarget::MethodCall {
                ref result_type, ..
            } = snap.target
            else {
                continue;
            };
            let Some(unresolved_method) = Self::describe_unresolved_ref(result_type) else {
                continue;
            };
            if !Self::unresolved_method_matches_snapshot_name(&unresolved_method, &snap.name) {
                continue;
            }
            if Self::is_known_class_body_dsl_unresolved_call(&snap, &unresolved_method) {
                continue;
            }
            if let Some((owner, _)) = unresolved_method.rsplit_once('#')
                && !self.diagnostic_owner_is_known(owner)
            {
                continue;
            }
            let resolved_result = self.resolve_type_for_hover(
                result_type,
                &snap.class_context,
                snap.method_context.as_deref(),
            );
            if !matches!(resolved_result, Type::Untyped | Type::Todo)
                || self.method_call_is_discoverable(result_type)
            {
                continue;
            }
            if let HoverTarget::MethodCall {
                ref receiver_type, ..
            } = snap.target
            {
                let resolved_receiver = self.resolve_type_for_hover(
                    receiver_type,
                    &snap.class_context,
                    snap.method_context.as_deref(),
                );
                if self
                    .dsl_plugin_method_return(&resolved_receiver, &snap.name)
                    .is_some()
                {
                    continue;
                }
            }
            if snap.method_context.is_none()
                && self.dsl_plugin_consumes_class_body_call(&snap.class_context, &snap.name)
            {
                continue;
            }
            if self.module_call_resolvable_via_includers(&unresolved_method, &snap.name) {
                continue;
            }
            // Don't declare a method missing if its surface is unknowable (framework namespace / method_missing / unmodeled external base).
            if let Some((owner, _)) = unresolved_method.rsplit_once('#') {
                let owner = owner.trim_scope_prefix().to_string();
                // A bare `Object`/`Class`/`Module` means self is unknown = Unknown (e.g. receiver degraded by a block DSL).
                if owner == "Object" || owner == "Class" || owner == "Module" {
                    crate::diagnostics::record_gating_suppression(
                        crate::diagnostics::GatingReason::ObjectReceiver,
                    );
                    continue;
                }
                if !self.registry.is_user_defined_class(&owner)
                    && self
                        .external_rbs
                        .is_some_and(|registry| registry.is_known_constant_namespace(&owner))
                {
                    continue;
                }
                let unknown_surface = if let Some(cached) = unknown_surface_cache.get(&owner) {
                    *cached
                } else {
                    let result = self.receiver_defines_method_missing(&owner);
                    unknown_surface_cache.insert(owner.clone(), result);
                    result
                };
                if unknown_surface {
                    continue;
                }
                // Only report missing once the ancestor chain resolves to a real definition (framework bases only have a complete surface after the tapioca DSL RBI merge).
                let complete = if let Some(cached) = ancestor_complete_cache.get(&owner) {
                    *cached
                } else {
                    self.ensure_class_available_with_ancestors(&owner, 0);
                    let result = self
                        .registry
                        .method_surface_knowledge_complete(&owner, self.lazy_rbi_loader.is_some());
                    ancestor_complete_cache.insert(owner.clone(), result);
                    result
                };
                if !complete {
                    crate::diagnostics::record_gating_suppression(
                        crate::diagnostics::GatingReason::IncompleteAncestors,
                    );
                    continue;
                }
            }
            let key = (
                snap.start,
                snap.end,
                snap.name.clone(),
                unresolved_method.clone(),
            );
            if !seen.insert(key) {
                continue;
            }
            calls.push(UnresolvedMethodCall {
                start: snap.start,
                end: snap.end,
                method_name: snap.name,
                unresolved_method,
            });
        }
        calls
    }

    pub(super) fn unresolved_constant_refs_for_sites(&mut self) -> Vec<UnresolvedConstant> {
        let sites = std::mem::take(&mut self.unresolved_constant_sites);
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for site in sites {
            if site.start >= site.end {
                continue;
            }
            if Self::is_builtin_special_constant(&site.name) {
                continue;
            }
            if self.diagnostic_owner_is_known(&site.name) {
                continue;
            }
            // Framework / declared-gem namespaces are defined at runtime, so don't report them as undefined.
            if self
                .external_rbs
                .is_some_and(|registry| registry.is_known_constant_namespace(&site.name))
            {
                continue;
            }
            // Only phantom singletons / `untyped` are truly undefined; constants with a concrete value are excluded.
            if self.constant_resolves_to_concrete(&site.name, &site.class_context) {
                continue;
            }
            // Declared with value `untyped` is Unknown. Excludes `Const::Path.method`-shaped misreads via the scope walk.
            if self.constant_is_declared(&site.name, &site.class_context) {
                continue;
            }
            // If the outer scope's ancestor chain is unresolved, a bare constant is Unknown (it might be an inherited constant).
            if !site.name.starts_with("::")
                && !site.name.contains("::")
                && !site.class_context.is_empty()
                && self.lexical_scope_ancestors_incomplete(&site.class_context)
            {
                continue;
            }
            if !seen.insert((site.start, site.end)) {
                continue;
            }
            out.push(UnresolvedConstant {
                start: site.start,
                end: site.end,
                name: site.name,
            });
        }
        out
    }

    fn lexical_scope_ancestors_incomplete(&mut self, class_context: &str) -> bool {
        let mut scope = class_context.trim_scope_prefix().to_string();
        loop {
            if !scope.is_empty() && self.registry.class_data_for(&scope).is_some() {
                self.ensure_class_available_with_ancestors(&scope, 0);
                if !self.registry.ancestor_knowledge_complete(&scope) {
                    return true;
                }
            }
            match scope.rfind_scope_sep() {
                Some(idx) => scope.truncate(idx),
                None => return false,
            }
        }
    }

    fn is_builtin_special_constant(name: &str) -> bool {
        matches!(
            name.trim_scope_prefix(),
            "ARGV"
                | "ARGF"
                | "DATA"
                | "GC"
                | "ENV"
                | "STDIN"
                | "STDOUT"
                | "STDERR"
                | "__ENCODING__"
                | "__FILE__"
                | "__LINE__"
                | "RUBY_VERSION"
                | "RUBY_PLATFORM"
                | "RUBY_ENGINE"
                | "RUBY_RELEASE_DATE"
                | "RUBY_DESCRIPTION"
                | "RUBY_COPYRIGHT"
                | "RUBY_REVISION"
                | "RUBY_ENGINE_VERSION"
                | "RUBY_PATCHLEVEL"
        )
    }

    fn constant_resolves_to_concrete(&mut self, name: &str, class_context: &str) -> bool {
        let bare = name.trim_scope_prefix();
        match self.resolve_constant_type_in_scope(bare, class_context) {
            Type::Untyped => false,
            Type::Singleton(n) | Type::Class(n) => n.as_ref() != bare,
            _ => true,
        }
    }

    fn constant_is_declared(&mut self, name: &str, class_context: &str) -> bool {
        let bare = name.trim_scope_prefix();
        if bare.is_empty() {
            return false;
        }
        if let Some((prefix, last)) = bare.rsplit_once("::") {
            for owner in Self::scoped_constant_candidates(prefix, class_context) {
                self.ensure_class_available(&owner);
                if self
                    .registry
                    .lookup_constant_through_ancestors(&owner, last)
                    .is_some()
                {
                    return true;
                }
            }
            return false;
        }
        for candidate in Self::scoped_constant_candidates(bare, class_context) {
            if let Some((owner, const_name)) = candidate.rsplit_once("::") {
                self.ensure_class_available(owner);
                if self
                    .registry
                    .lookup_constant_through_ancestors(owner, const_name)
                    .is_some()
                {
                    return true;
                }
            }
        }
        if !class_context.is_empty() {
            self.ensure_class_available(class_context);
            if self
                .registry
                .lookup_constant_through_ancestors(class_context, bare)
                .is_some()
            {
                return true;
            }
        }
        self.ensure_class_available("Object");
        self.registry
            .lookup_constant_through_ancestors("Object", bare)
            .is_some()
    }

    pub(super) fn argument_type_mismatches_for_sites(&mut self) -> Vec<ArgumentTypeMismatch> {
        let sites = std::mem::take(&mut self.arg_check_sites);
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for site in sites {
            let receiver = self.resolve_type_for_hover(
                &site.receiver_type,
                &site.class_context,
                site.method_context.as_deref(),
            );
            let lookup_receiver = if Self::contains_nil(&receiver) {
                Self::remove_nil(&receiver)
            } else {
                receiver
            };
            let Some(class_name) = self.type_to_class_name(&lookup_receiver) else {
                continue;
            };
            self.ensure_external_class(&class_name);
            let prefer_singleton =
                site.is_singleton || matches!(lookup_receiver, Type::Singleton(_));
            let Some(sig) = self.registry.lookup_method_sig_for_receiver_with_hint(
                &class_name,
                &site.method_name,
                prefer_singleton,
            ) else {
                continue;
            };
            // Only annotated declarations qualify; `rbs_file_source` alone (call-site inference from an empty RBI def) is not treated as authoritative.
            if !(sig.rbs_annotated || sig.rbs_inline_annotated || sig.sig_annotated) {
                continue;
            }
            // For overloads, report only when every candidate is incompatible; stay silent if even one could match.
            if !sig.overloads.is_empty() {
                self.overloaded_arg_mismatches_for_site(
                    &site,
                    &class_name,
                    prefer_singleton,
                    &sig,
                    &mut seen,
                    &mut out,
                );
                continue;
            }

            let positional: Vec<&Param> = sig
                .params
                .iter()
                .filter(|p| {
                    matches!(
                        p.kind,
                        ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                    )
                })
                .collect();
            let rest_pos = positional.iter().position(|p| p.kind == ParamKind::Rest);
            // Skip when a required param follows `*rest`, since positional mapping is then ambiguous.
            let positional_ambiguous = matches!(rest_pos, Some(ri) if ri != positional.len() - 1);
            let fixed_count = rest_pos.unwrap_or(positional.len());
            let rest_param = rest_pos.map(|ri| positional[ri]);
            let kwrest = sig.params.iter().find(|p| p.kind == ParamKind::DoubleRest);

            // Judge duck alias / interface params structurally against raw RBS, since collapsing them to nominal would cause false positives.
            let rbs_function_type: Option<rbs_ir::FunctionType> = {
                let types = self.registry.lookup_rbs_method_types_with_hint(
                    &class_name,
                    &site.method_name,
                    prefer_singleton,
                );
                (types.len() == 1).then(|| types[0].function_type.clone())
            };

            let mut pos_i = 0usize;
            for arg in &site.args {
                // Structural slots are evaluated against raw RBS via `rbs_param_compat` (an already-collapsed nominal type would miss them).
                let mut structural_rbs: Option<&rbs_ir::RbsType> = None;
                let param = match &arg.keyword {
                    None => {
                        let idx = pos_i;
                        pos_i += 1;
                        if positional_ambiguous {
                            continue;
                        }
                        let rbs_ty = rbs_function_type
                            .as_ref()
                            .and_then(|ft| Self::rbs_positional_param_type(ft, idx));
                        if rbs_ty.is_some_and(Self::rbs_type_is_structural_liberal) {
                            structural_rbs = rbs_ty;
                        }
                        if idx < fixed_count {
                            positional[idx]
                        } else if let Some(rest) = rest_param {
                            rest
                        } else {
                            // Arity overflow — not a type error.
                            continue;
                        }
                    }
                    Some(name) => {
                        let rbs_ty = rbs_function_type
                            .as_ref()
                            .and_then(|ft| Self::rbs_keyword_param_type(ft, name));
                        if rbs_ty.is_some_and(Self::rbs_type_is_structural_liberal) {
                            structural_rbs = rbs_ty;
                        }
                        if let Some(p) = sig.params.iter().find(|p| {
                            matches!(
                                p.kind,
                                ParamKind::KeywordRequired | ParamKind::KeywordOptional
                            ) && p.name == *name
                        }) {
                            p
                        } else if let Some(kr) = kwrest {
                            kr
                        } else {
                            continue;
                        }
                    }
                };

                // A param whose type is only a literal/`nil` derived from its default value is not treated as a constraint (avoids false positives).
                if structural_rbs.is_none()
                    && matches!(
                        param.param_type,
                        Type::Nil
                            | Type::LiteralSymbol(_)
                            | Type::LiteralString(_)
                            | Type::LiteralInteger(_)
                            | Type::LiteralFloat(_)
                    )
                {
                    continue;
                }
                let actual = self.resolve_type_for_hover(
                    &arg.ty,
                    &site.class_context,
                    site.method_context.as_deref(),
                );
                let compat = match structural_rbs {
                    Some(rbs_ty) => self.rbs_param_compat(&actual, rbs_ty),
                    None => self.arg_compat(&actual, &param.param_type),
                };
                if compat != ArgCompat::No {
                    continue;
                }
                if arg.start >= arg.end || !seen.insert((arg.start, arg.end)) {
                    continue;
                }
                let expected = match structural_rbs {
                    Some(rbs_ty) => Self::rbs_param_type_display(rbs_ty),
                    None => param.param_type.to_string(),
                };
                out.push(ArgumentTypeMismatch {
                    start: arg.start,
                    end: arg.end,
                    method_name: site.method_name.clone(),
                    param_name: param.name.clone(),
                    expected,
                    actual: actual.to_string(),
                });
            }
        }
        out
    }

    fn overloaded_arg_mismatches_for_site(
        &mut self,
        site: &ArgCheckSite,
        class_name: &str,
        prefer_singleton: bool,
        sig: &MethodSig,
        seen: &mut BTreeSet<(usize, usize)>,
        out: &mut Vec<ArgumentTypeMismatch>,
    ) {
        // Resolve actual argument types just once, since they're reused across all overloads.
        let actuals: Vec<Type> = site
            .args
            .iter()
            .map(|arg| {
                self.resolve_type_for_hover(
                    &arg.ty,
                    &site.class_context,
                    site.method_context.as_deref(),
                )
            })
            .collect();
        // Raw RBS only aligns by index when its element count matches `[base]+overloads` (Sorbet sig overloads aren't aligned).
        let rbs_types = self
            .registry
            .lookup_rbs_method_types_with_hint(class_name, &site.method_name, prefer_singleton)
            .to_vec();
        let candidate_count = 1 + sig.overloads.len();
        let aligned = rbs_types.len() == candidate_count;

        let mut any_arity_ok = false;
        let mut any_could_match = false;
        let mut best: Option<Vec<ArgumentTypeMismatch>> = None;
        let candidates = std::iter::once(sig.params.as_slice())
            .chain(sig.overloads.iter().map(|o| o.params.as_slice()));
        for (ci, params) in candidates.enumerate() {
            let rbs_ft = if aligned {
                Some(&rbs_types[ci].function_type)
            } else {
                None
            };
            let Some(findings) =
                self.overload_candidate_arg_findings(site, &actuals, params, rbs_ft)
            else {
                continue;
            };
            any_arity_ok = true;
            if findings.is_empty() {
                any_could_match = true;
                break;
            }
            match &best {
                Some(b) if b.len() <= findings.len() => {}
                _ => best = Some(findings),
            }
        }

        // Leave arity mismatches to experimental; stay silent if even one candidate could match.
        if !any_arity_ok || any_could_match {
            return;
        }
        if let Some(findings) = best {
            for f in findings {
                if f.start >= f.end || !seen.insert((f.start, f.end)) {
                    continue;
                }
                out.push(f);
            }
        }
    }

    fn overload_candidate_arg_findings(
        &mut self,
        site: &ArgCheckSite,
        actuals: &[Type],
        params: &[Param],
        rbs_ft: Option<&rbs_ir::FunctionType>,
    ) -> Option<Vec<ArgumentTypeMismatch>> {
        let positional: Vec<&Param> = params
            .iter()
            .filter(|p| {
                matches!(
                    p.kind,
                    ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                )
            })
            .collect();
        let rest_pos = positional.iter().position(|p| p.kind == ParamKind::Rest);
        let positional_ambiguous = matches!(rest_pos, Some(ri) if ri != positional.len() - 1);
        let fixed_count = rest_pos.unwrap_or(positional.len());
        let rest_param = rest_pos.map(|ri| positional[ri]);
        let kwrest = params.iter().find(|p| p.kind == ParamKind::DoubleRest);
        let has_rest = rest_pos.is_some();
        let required = positional
            .iter()
            .filter(|p| p.kind == ParamKind::Required)
            .count();
        let max_positional = positional
            .iter()
            .filter(|p| p.kind != ParamKind::Rest)
            .count();
        let method_has_keywords = params.iter().any(|p| {
            matches!(
                p.kind,
                ParamKind::KeywordRequired | ParamKind::KeywordOptional | ParamKind::DoubleRest
            )
        });

        // Withhold the arity judgment when keyword-vs-trailing-Hash is ambiguous (same criterion as experimental).
        let positional_unambiguous = site.keyword_names.is_empty() || method_has_keywords;
        if positional_unambiguous && !site.has_pos_splat {
            let n = site.positional_count;
            if n < required || (!has_rest && n > max_positional) {
                return None;
            }
        }
        if positional_unambiguous && !site.has_kwsplat {
            let missing_required_kw = params.iter().any(|p| {
                p.kind == ParamKind::KeywordRequired
                    && !site.keyword_names.iter().any(|k| k == &p.name)
            });
            if missing_required_kw {
                return None;
            }
            if kwrest.is_none() {
                let has_unaccepted_kw = site.keyword_names.iter().any(|k| {
                    !params.iter().any(|p| {
                        matches!(
                            p.kind,
                            ParamKind::KeywordRequired | ParamKind::KeywordOptional
                        ) && &p.name == k
                    })
                });
                if has_unaccepted_kw {
                    return None;
                }
            }
        }

        let mut findings = Vec::new();
        let mut pos_i = 0usize;
        for (ai, arg) in site.args.iter().enumerate() {
            let mut structural_rbs: Option<&rbs_ir::RbsType> = None;
            let param = match &arg.keyword {
                None => {
                    let idx = pos_i;
                    pos_i += 1;
                    if positional_ambiguous {
                        continue;
                    }
                    let rbs_ty = rbs_ft.and_then(|ft| Self::rbs_positional_param_type(ft, idx));
                    if rbs_ty.is_some_and(Self::rbs_type_is_structural_liberal) {
                        structural_rbs = rbs_ty;
                    }
                    if idx < fixed_count {
                        positional[idx]
                    } else if let Some(rest) = rest_param {
                        rest
                    } else {
                        // Arity overflow — not a type error.
                        continue;
                    }
                }
                Some(name) => {
                    let rbs_ty = rbs_ft.and_then(|ft| Self::rbs_keyword_param_type(ft, name));
                    if rbs_ty.is_some_and(Self::rbs_type_is_structural_liberal) {
                        structural_rbs = rbs_ty;
                    }
                    if let Some(p) = params.iter().find(|p| {
                        matches!(
                            p.kind,
                            ParamKind::KeywordRequired | ParamKind::KeywordOptional
                        ) && p.name == *name
                    }) {
                        p
                    } else if let Some(kr) = kwrest {
                        kr
                    } else {
                        // Unknown keyword — not a type error here.
                        continue;
                    }
                }
            };

            // The overload path only handles scalar nominal params — containers / duck aliases clash with approximate inference and cause false positives.
            if structural_rbs.is_some() || !Self::is_scalar_nominal_constraint(&param.param_type) {
                continue;
            }
            let actual = &actuals[ai];
            if self.arg_compat(actual, &param.param_type) != ArgCompat::No {
                continue;
            }
            if arg.start >= arg.end {
                continue;
            }
            findings.push(ArgumentTypeMismatch {
                start: arg.start,
                end: arg.end,
                method_name: site.method_name.clone(),
                param_name: param.name.clone(),
                expected: param.param_type.to_string(),
                actual: actual.to_string(),
            });
        }
        Some(findings)
    }

    fn is_scalar_nominal_constraint(ty: &Type) -> bool {
        match ty {
            Type::Integer
            | Type::Float
            | Type::String
            | Type::Symbol
            | Type::Bool
            | Type::True
            | Type::False
            | Type::Class(_) => true,
            Type::Union(members) => members
                .iter()
                .all(|m| matches!(m, Type::Nil) || Self::is_scalar_nominal_constraint(m)),
            _ => false,
        }
    }

    pub(super) fn experimental_checks_for_sites(&mut self) -> Vec<ExperimentalDiagnostic> {
        let mut out = Vec::new();
        self.arity_mismatches_for_sites(&mut out);
        self.union_member_missing_methods_for_sites(&mut out);
        out
    }

    fn union_member_missing_methods_for_sites(&mut self, out: &mut Vec<ExperimentalDiagnostic>) {
        let snapshots = self.var_snapshots.clone();
        let mut seen: BTreeSet<(usize, usize, String, String)> = BTreeSet::new();
        for snap in snapshots {
            if snap.start >= snap.end {
                continue;
            }
            let HoverTarget::MethodCall {
                ref receiver_type, ..
            } = snap.target
            else {
                continue;
            };
            let method_name = snap.name.clone();
            if Self::is_universal_object_method_for_diagnostics(&method_name) {
                continue;
            }
            let receiver = self.resolve_type_for_hover(
                receiver_type,
                &snap.class_context,
                snap.method_context.as_deref(),
            );
            let Type::Union(parts) = &receiver else {
                continue;
            };
            // If any union member is untyped/a ref, its surface is unknown, so silence the whole union.
            let mut member_classes: Vec<(Type, String)> = Vec::with_capacity(parts.len());
            let mut all_judgeable = true;
            for member in parts {
                match self.type_to_class_name(member) {
                    Some(class) => member_classes.push((member.clone(), class)),
                    None => {
                        all_judgeable = false;
                        break;
                    }
                }
            }
            if !all_judgeable {
                continue;
            }
            let mut resolved_count = 0usize;
            let mut lacking: Vec<(Type, String)> = Vec::new();
            for (member, class) in &member_classes {
                let r = self.resolve_method_on_type(member, &method_name);
                let member_lacks_method = matches!(
                    &r,
                    Type::ReceiverMethodRef(recv, m)
                        if recv.as_ref() == member && m.as_str() == method_name
                );
                if member_lacks_method {
                    lacking.push((member.clone(), class.clone()));
                } else {
                    resolved_count += 1;
                }
            }
            if resolved_count == 0 || lacking.is_empty() {
                continue;
            }
            let mut display_parts: Vec<String> = parts
                .iter()
                .filter(|part| !matches!(part, Type::Nil))
                .map(|part| part.to_string())
                .collect();
            if parts.iter().any(|part| matches!(part, Type::Nil)) {
                display_parts.push("nil".to_string());
            }
            let receiver_display = display_parts.join(" | ");
            for (member, class) in lacking {
                // Report only when the member's surface is provably complete (same conservative gate as missing).
                if !self.member_surface_provably_complete(&class) {
                    continue;
                }
                let key = (snap.start, snap.end, class.clone(), method_name.clone());
                if !seen.insert(key) {
                    continue;
                }
                let member_display = member.to_string();
                out.push(ExperimentalDiagnostic {
                    start: snap.start,
                    end: snap.end,
                    code: "union_member_missing_method",
                    severity: "information",
                    message: format!(
                        "Method `{method_name}` not found for union member `{member_display}` of receiver `{receiver_display}`"
                    ),
                    method_name: method_name.clone(),
                });
            }
        }
    }

    fn member_surface_provably_complete(&mut self, class: &str) -> bool {
        let owner = class.trim_scope_prefix();
        // If the receiver has degraded to a bare `Object`, self is unknown = Unknown.
        if owner == "Object" {
            return false;
        }
        // There's no way to know the surface of something from an undefined class (e.g. a phantom singleton).
        if !self.diagnostic_owner_is_known(owner) {
            return false;
        }
        // Framework / gem namespaces (which Tyda has no type info for) are unknowable.
        if !self.registry.is_user_defined_class(owner)
            && self
                .external_rbs
                .is_some_and(|registry| registry.is_known_constant_namespace(owner))
        {
            return false;
        }
        // A receiver with method_missing responds to any message.
        if self.receiver_defines_method_missing(owner) {
            return false;
        }
        // The surface is only considered complete once the ancestor chain resolves to a real definition after lazy loading.
        self.ensure_class_available_with_ancestors(owner, 0);
        self.registry
            .method_surface_knowledge_complete(owner, self.lazy_rbi_loader.is_some())
    }

    fn arity_mismatches_for_sites(&mut self, out: &mut Vec<ExperimentalDiagnostic>) {
        let sites = self.arg_check_sites.clone();
        let mut seen: BTreeSet<(usize, usize, String)> = BTreeSet::new();
        for site in sites {
            if site.has_pos_splat {
                continue;
            }
            let receiver = self.resolve_type_for_hover(
                &site.receiver_type,
                &site.class_context,
                site.method_context.as_deref(),
            );
            let lookup_receiver = if Self::contains_nil(&receiver) {
                Self::remove_nil(&receiver)
            } else {
                receiver
            };
            let Some(class_name) = self.type_to_class_name(&lookup_receiver) else {
                continue;
            };
            self.ensure_external_class(&class_name);
            let prefer_singleton =
                site.is_singleton || matches!(lookup_receiver, Type::Singleton(_));
            let Some(sig) = self.registry.lookup_method_sig_for_receiver_with_hint(
                &class_name,
                &site.method_name,
                prefer_singleton,
            ) else {
                continue;
            };
            // Only judge against authoritative declarations (RBS/sig); against an inferred
            // signature, gaps on Tyda's side could cause false positives.
            if !(sig.rbs_annotated
                || sig.rbs_inline_annotated
                || sig.sig_annotated
                || sig.rbs_file_source)
            {
                continue;
            }
            // Skip overloads since the resolution target isn't unique (same policy as type checking).
            if !sig.overloads.is_empty() {
                continue;
            }

            let positional: Vec<&Param> = sig
                .params
                .iter()
                .filter(|p| {
                    matches!(
                        p.kind,
                        ParamKind::Required | ParamKind::Optional | ParamKind::Rest
                    )
                })
                .collect();
            let has_rest = positional.iter().any(|p| p.kind == ParamKind::Rest);
            let required = positional
                .iter()
                .filter(|p| p.kind == ParamKind::Required)
                .count();
            let max_positional = positional
                .iter()
                .filter(|p| p.kind != ParamKind::Rest)
                .count();
            let method_has_keywords = sig.params.iter().any(|p| {
                matches!(
                    p.kind,
                    ParamKind::KeywordRequired | ParamKind::KeywordOptional | ParamKind::DoubleRest
                )
            });

            let mut messages: Vec<String> = Vec::new();

            // Positional argument arity. If keywords are given but the callee has no
            // keyword params, it's ambiguous whether "keywords = a positional Hash", so skip (avoids false positives).
            let positional_unambiguous = site.keyword_names.is_empty() || method_has_keywords;
            if positional_unambiguous {
                let n = site.positional_count;
                if n < required || (!has_rest && n > max_positional) {
                    let expected = if has_rest {
                        format!("{required}+")
                    } else if required == max_positional {
                        required.to_string()
                    } else {
                        format!("{required}..{max_positional}")
                    };
                    messages.push(format!(
                        "wrong number of arguments (given {n}, expected {expected})"
                    ));
                }
            }

            // Missing required keyword. Skip if `**kwargs` forwarding is present, since the set can't be determined then.
            if !site.has_kwsplat {
                let missing: Vec<String> = sig
                    .params
                    .iter()
                    .filter(|p| p.kind == ParamKind::KeywordRequired)
                    .filter(|p| !site.keyword_names.iter().any(|k| k == &p.name))
                    .map(|p| p.name.clone())
                    .collect();
                if !missing.is_empty() {
                    let plural = if missing.len() > 1 { "s" } else { "" };
                    let names = missing
                        .iter()
                        .map(|m| format!(":{m}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    messages.push(format!("missing keyword{plural}: {names}"));
                }
            }

            for message in messages {
                if site.call_start >= site.call_end
                    || !seen.insert((site.call_start, site.call_end, message.clone()))
                {
                    continue;
                }
                out.push(ExperimentalDiagnostic {
                    start: site.call_start,
                    end: site.call_end,
                    code: "arity_mismatch",
                    severity: "warning",
                    message,
                    method_name: site.method_name.clone(),
                });
            }
        }
    }

    fn rbs_positional_param_type(
        ft: &rbs_ir::FunctionType,
        idx: usize,
    ) -> Option<&rbs_ir::RbsType> {
        let req = ft.required_positionals.len();
        let opt = ft.optional_positionals.len();
        if idx < req {
            Some(&ft.required_positionals[idx].type_)
        } else if idx < req + opt {
            Some(&ft.optional_positionals[idx - req].type_)
        } else {
            ft.rest_positionals.as_ref().map(|p| &p.type_)
        }
    }

    fn rbs_keyword_param_type<'r>(
        ft: &'r rbs_ir::FunctionType,
        name: &str,
    ) -> Option<&'r rbs_ir::RbsType> {
        ft.required_keywords
            .iter()
            .chain(ft.optional_keywords.iter())
            .find(|(k, _)| k.as_str() == name)
            .map(|(_, p)| &p.type_)
            .or_else(|| ft.rest_keywords.as_ref().map(|p| &p.type_))
    }

    fn rbs_type_is_structural_liberal(ty: &rbs_ir::RbsType) -> bool {
        match ty {
            rbs_ir::RbsType::Class(name, args) => {
                name.as_str().trim_scope_prefix().starts_with('_')
                    || args.iter().any(Self::rbs_type_is_structural_liberal)
            }
            rbs_ir::RbsType::Alias(name, args) => {
                matches!(
                    name.as_str().trim_scope_prefix(),
                    "string" | "path" | "int" | "io" | "interned" | "encoding"
                ) || args.iter().any(Self::rbs_type_is_structural_liberal)
            }
            rbs_ir::RbsType::Union(types)
            | rbs_ir::RbsType::Intersection(types)
            | rbs_ir::RbsType::Tuple(types) => {
                types.iter().any(Self::rbs_type_is_structural_liberal)
            }
            rbs_ir::RbsType::Optional(inner) => Self::rbs_type_is_structural_liberal(inner),
            _ => false,
        }
    }

    fn rbs_name_is_interface(name: &str) -> bool {
        name.trim_scope_prefix()
            .rsplit("::")
            .next()
            .is_some_and(|short| short.starts_with('_'))
    }

    fn rbs_param_type_display(rbs_type: &rbs_ir::RbsType) -> String {
        match rbs_type {
            rbs_ir::RbsType::Class(name, _) | rbs_ir::RbsType::Alias(name, _) => {
                name.as_str().trim_scope_prefix().to_string()
            }
            rbs_ir::RbsType::Union(members) => members
                .iter()
                .map(Self::rbs_param_type_display)
                .collect::<Vec<_>>()
                .join(" | "),
            rbs_ir::RbsType::Optional(inner) => {
                format!("{}?", Self::rbs_param_type_display(inner))
            }
            rbs_ir::RbsType::String => "String".to_string(),
            rbs_ir::RbsType::Integer => "Integer".to_string(),
            rbs_ir::RbsType::Symbol => "Symbol".to_string(),
            _ => crate::rbs::convert::convert_rbs_type(rbs_type).to_string(),
        }
    }

    fn builtin_duck_alias_members(alias: &str) -> Option<Vec<rbs_ir::RbsType>> {
        use rbs_ir::RbsType;
        let named = |name: &str| RbsType::Class(Sym::new(name), Box::new([]));
        let members = match alias.trim_scope_prefix() {
            "int" => vec![RbsType::Integer, named("_ToInt")],
            "string" => vec![RbsType::String, named("_ToStr")],
            "path" => vec![RbsType::String, named("_ToStr"), named("_ToPath")],
            "io" => vec![named("IO"), named("_ToIO")],
            "interned" => vec![RbsType::Symbol, RbsType::String, named("_ToStr")],
            "encoding" => vec![named("Encoding"), RbsType::String, named("_ToStr")],
            _ => return None,
        };
        Some(members)
    }

    fn builtin_interface_required_methods(interface: &str) -> Option<&'static [&'static str]> {
        let short = interface
            .trim_scope_prefix()
            .rsplit("::")
            .next()
            .unwrap_or(interface);
        Some(match short {
            "_ToC" => &["to_c"],
            "_ToR" => &["to_r"],
            "_ToF" => &["to_f"],
            "_ToI" => &["to_i"],
            "_ToInt" => &["to_int"],
            "_ToS" => &["to_s"],
            "_ToStr" => &["to_str"],
            "_ToSym" => &["to_sym"],
            "_ToH" => &["to_h"],
            "_ToHash" => &["to_hash"],
            "_ToA" => &["to_a"],
            "_ToAry" => &["to_ary"],
            "_ToProc" => &["to_proc"],
            "_ToPath" => &["to_path"],
            "_ToIO" => &["to_io"],
            "_Inspect" => &["inspect"],
            "_Each" => &["each"],
            "_EachEntry" => &["each_entry"],
            "_Reader" => &["read"],
            "_ReaderPartial" => &["readpartial"],
            "_Writer" => &["write"],
            "_Rewindable" => &["rewind"],
            "_Range" => &["begin", "end", "exclude_end?"],
            "_Exception" => &["exception"],
            _ => return None,
        })
    }

    fn interface_required_method_names(&self, interface: &str) -> Vec<String> {
        let bare = interface.trim_scope_prefix();
        let declared =
            self.declared_interface_required_methods(bare, &std::collections::HashMap::new());
        if !declared.is_empty() {
            return declared
                .into_iter()
                .map(|method| method.name.to_string())
                .collect();
        }
        Self::builtin_interface_required_methods(bare)
            .map(|methods| methods.iter().map(|name| name.to_string()).collect())
            .unwrap_or_default()
    }

    fn class_responds_to_method(&mut self, class_name: &str, method_name: &str) -> bool {
        Self::is_universal_object_method_for_diagnostics(method_name)
            || self
                .resolve_hover_method_return_opt(class_name, method_name, false)
                .is_some()
    }

    fn rbs_interface_compat(&mut self, interface: &str, actual: &Type) -> ArgCompat {
        let Some(actual_class) = self.type_to_class_name(actual) else {
            return ArgCompat::Unknown;
        };
        let required = self.interface_required_method_names(interface);
        if required.is_empty() {
            // An unknown interface whose required methods can't be looked up is unjudgeable.
            return ArgCompat::Unknown;
        }
        if !self.member_surface_provably_complete(&actual_class) {
            return ArgCompat::Unknown;
        }
        if required
            .iter()
            .all(|method| self.class_responds_to_method(&actual_class, method))
        {
            ArgCompat::Yes
        } else {
            ArgCompat::No
        }
    }

    fn rbs_param_compat_union(&mut self, actual: &Type, members: &[rbs_ir::RbsType]) -> ArgCompat {
        let mut any_unknown = false;
        for member in members {
            match self.rbs_param_compat(actual, member) {
                ArgCompat::Yes => return ArgCompat::Yes,
                ArgCompat::Unknown => any_unknown = true,
                ArgCompat::No => {}
            }
        }
        if any_unknown {
            ArgCompat::Unknown
        } else {
            ArgCompat::No
        }
    }

    fn rbs_param_compat(&mut self, actual: &Type, rbs_type: &rbs_ir::RbsType) -> ArgCompat {
        // interface: judged structurally by which method names it responds to.
        if let rbs_ir::RbsType::Class(name, _) = rbs_type
            && Self::rbs_name_is_interface(name)
        {
            return self.rbs_interface_compat(name, actual);
        }
        if let rbs_ir::RbsType::Union(members) = rbs_type {
            return self.rbs_param_compat_union(actual, members);
        }
        if let rbs_ir::RbsType::Alias(name, args) = rbs_type
            && args.is_empty()
            && let Some(members) = Self::builtin_duck_alias_members(name)
        {
            return self.rbs_param_compat_union(actual, &members);
        }
        // Otherwise, collapse to a concrete type and delegate to the existing arg_compat. If the
        // collapsed result is unresolved (unknown nominal / type variable, etc.), stay silent as unjudgeable.
        let converted = crate::rbs::convert::convert_rbs_type(rbs_type);
        if Self::type_is_unconstrained_for_check(&converted) {
            ArgCompat::Unknown
        } else {
            self.arg_compat(actual, &converted)
        }
    }

    fn actual_is_unresolved_type_variable(&self, actual: &Type) -> bool {
        let Type::Class(name) = actual else {
            return false;
        };
        let name = name.as_str();
        if name.contains("::") || name.contains('[') {
            return false;
        }
        if self.registry.is_user_defined_class(name) || Self::is_core_builtin_class(name) {
            return false;
        }
        match self.registry.class_data_for(name) {
            Some(data) => !data.has_type_substance() && !data.is_module,
            // A name found in no registry is also treated as an unresolvable type variable
            // (the nominal path is already Unknown, but this makes it explicit to guarantee silence).
            None => true,
        }
    }

    pub(super) fn arg_compat(&mut self, actual: &Type, declared: &Type) -> ArgCompat {
        if Self::type_is_unconstrained_for_check(declared) {
            return ArgCompat::Unknown;
        }
        // Unknown / unresolved actual type → we can't judge.
        match actual {
            Type::Bot => return ArgCompat::Yes,
            _ if Self::type_is_unconstrained_for_check(actual) => return ArgCompat::Unknown,
            _ if self.actual_is_unresolved_type_variable(actual) => return ArgCompat::Unknown,
            _ => {}
        }

        if let Type::Union(parts) = declared {
            let mut any_unknown = false;
            for p in parts {
                match self.arg_compat(actual, p) {
                    ArgCompat::Yes => return ArgCompat::Yes,
                    ArgCompat::Unknown => any_unknown = true,
                    ArgCompat::No => {}
                }
            }
            return if any_unknown {
                ArgCompat::Unknown
            } else {
                ArgCompat::No
            };
        }
        if let Type::Intersection(parts) = declared {
            let mut any_unknown = false;
            for p in parts {
                match self.arg_compat(actual, p) {
                    ArgCompat::No => return ArgCompat::No,
                    ArgCompat::Unknown => any_unknown = true,
                    ArgCompat::Yes => {}
                }
            }
            return if any_unknown {
                ArgCompat::Unknown
            } else {
                ArgCompat::Yes
            };
        }
        if let Type::Union(parts) = actual {
            let mut all_no = true;
            let mut all_yes = true;
            for p in parts {
                match self.arg_compat(p, declared) {
                    ArgCompat::No => all_yes = false,
                    ArgCompat::Yes => all_no = false,
                    ArgCompat::Unknown => {
                        all_no = false;
                        all_yes = false;
                    }
                }
            }
            return if all_no {
                ArgCompat::No
            } else if all_yes {
                ArgCompat::Yes
            } else {
                ArgCompat::Unknown
            };
        }

        self.arg_compat_scalar(actual, declared)
    }

    fn arg_compat_scalar(&mut self, actual: &Type, declared: &Type) -> ArgCompat {
        // `bool` is not a real class; match the boolean family explicitly.
        if matches!(declared, Type::Bool) {
            return if matches!(actual, Type::Bool | Type::True | Type::False) {
                ArgCompat::Yes
            } else if Self::actual_is_concrete_non_bool(actual) {
                ArgCompat::No
            } else {
                ArgCompat::Unknown
            };
        }

        // Literal declared types require an exact literal (or stay Unknown when
        // the actual is the broadened class).
        match declared {
            Type::LiteralInteger(n) => {
                return match actual {
                    Type::LiteralInteger(m) if m == n => ArgCompat::Yes,
                    Type::LiteralInteger(_) => ArgCompat::No,
                    Type::Integer => ArgCompat::Unknown,
                    _ => self.arg_compat_nominal(actual, "Integer"),
                };
            }
            Type::LiteralString(s) => {
                return match actual {
                    Type::LiteralString(o) if o == s => ArgCompat::Yes,
                    Type::LiteralString(_) => ArgCompat::No,
                    Type::String => ArgCompat::Unknown,
                    _ => self.arg_compat_nominal(actual, "String"),
                };
            }
            Type::LiteralSymbol(s) => {
                return match actual {
                    Type::LiteralSymbol(o) if o == s => ArgCompat::Yes,
                    Type::LiteralSymbol(_) => ArgCompat::No,
                    Type::Symbol => ArgCompat::Unknown,
                    _ => self.arg_compat_nominal(actual, "Symbol"),
                };
            }
            Type::LiteralFloat(s) => {
                return match actual {
                    Type::LiteralFloat(o) if o == s => ArgCompat::Yes,
                    Type::LiteralFloat(_) => ArgCompat::No,
                    Type::Float => ArgCompat::Unknown,
                    _ => self.arg_compat_nominal(actual, "Float"),
                };
            }
            // A literal `true` / `false` declared type is almost always a `= true` / `= false` default (or a `T::Boolean` param whose default narrowed it), not a real "must be exactly true" contract.
            // Accept the whole boolean family so passing the other boolean — `with_transaction: false` against a `= true` default — isn't flagged;
            Type::True | Type::False => {
                return match actual {
                    Type::True | Type::False | Type::Bool => ArgCompat::Unknown,
                    _ => self.arg_compat_nominal(
                        actual,
                        if matches!(declared, Type::True) {
                            "TrueClass"
                        } else {
                            "FalseClass"
                        },
                    ),
                };
            }
            _ => {}
        }

        match declared {
            Type::Array(inner) => return self.arg_compat_array(actual, inner.as_deref()),
            Type::Hash(key, value) => {
                return self.arg_compat_hash(actual, key.as_deref(), value.as_deref());
            }
            Type::Tuple(_) => {
                return match actual {
                    Type::Tuple(_) | Type::Array(_) => ArgCompat::Unknown,
                    _ if self.actual_is_array_like(actual) => ArgCompat::Unknown,
                    _ if Self::actual_is_concrete_non_collection(actual) => ArgCompat::No,
                    _ => ArgCompat::Unknown,
                };
            }
            Type::Record(_) => {
                return match actual {
                    Type::Record(_) | Type::Hash(_, _) => ArgCompat::Unknown,
                    _ if Self::actual_is_concrete_non_collection(actual) => ArgCompat::No,
                    _ => ArgCompat::Unknown,
                };
            }
            Type::Proc { .. } => {
                return match actual {
                    Type::Proc { .. } => ArgCompat::Yes,
                    _ => ArgCompat::Unknown,
                };
            }
            _ => {}
        }

        // Nominal (class / instance) comparison.
        let Some(declared_class) = self.type_to_class_name(declared) else {
            return ArgCompat::Unknown;
        };
        self.arg_compat_nominal(actual, &declared_class)
    }

    fn arg_compat_array(&mut self, actual: &Type, declared_elem: Option<&Type>) -> ArgCompat {
        match actual {
            Type::Array(None) => ArgCompat::Unknown,
            Type::Array(Some(elem)) => match declared_elem {
                None => ArgCompat::Yes,
                Some(decl) => self.arg_compat(elem, decl),
            },
            Type::Tuple(elems) => match declared_elem {
                None => ArgCompat::Yes,
                Some(decl) => {
                    let mut any_unknown = false;
                    for e in elems {
                        match self.arg_compat(e, decl) {
                            ArgCompat::No => return ArgCompat::No,
                            ArgCompat::Unknown => any_unknown = true,
                            ArgCompat::Yes => {}
                        }
                    }
                    if any_unknown {
                        ArgCompat::Unknown
                    } else {
                        ArgCompat::Yes
                    }
                }
            },
            _ if Self::actual_is_concrete_non_collection(actual) => ArgCompat::No,
            _ => ArgCompat::Unknown,
        }
    }

    fn arg_compat_hash(
        &mut self,
        actual: &Type,
        declared_key: Option<&Type>,
        declared_value: Option<&Type>,
    ) -> ArgCompat {
        match actual {
            Type::Hash(_, _) | Type::Record(_) => {
                let _ = (declared_key, declared_value);
                ArgCompat::Unknown
            }
            _ if Self::actual_is_concrete_non_collection(actual) => ArgCompat::No,
            _ => ArgCompat::Unknown,
        }
    }

    fn arg_compat_nominal(&mut self, actual: &Type, declared_class: &str) -> ArgCompat {
        let Some(actual_class) = self.type_to_class_name(actual) else {
            return ArgCompat::Unknown;
        };
        // Normalize absolute references (leading `::`) before comparing nominal names. Registry class names are registered without `::`, so if only one side is absolute (`::Billing::Invoice`) the same class would otherwise fail to match.
        let actual_class = actual_class.trim_scope_prefix().to_string();
        let declared_class = declared_class.trim_scope_prefix();
        if actual_class == declared_class {
            return ArgCompat::Yes;
        }
        // Core numeric tower: the stdlib RBS ancestry (`Integer < Numeric`, `Float < Numeric`, all `Comparable`) is often not merged in the lazy arg-check context, so recognize it explicitly to avoid flagging e.g.
        // `timeout(5)` against a `Numeric` parameter.
        if matches!(declared_class, "Numeric" | "Comparable")
            && matches!(
                actual_class.as_str(),
                "Integer" | "Float" | "Rational" | "Complex"
            )
        {
            return ArgCompat::Yes;
        }
        // TimeWithZone -> Time is Rails duck compatible (they're siblings in ancestry); the reverse direction is Unknown via the method_missing gate.
        if self.rails_feature_enabled()
            && actual_class == "ActiveSupport::TimeWithZone"
            && declared_class == "Time"
        {
            return ArgCompat::Yes;
        }
        // tapioca-generated relation aliases make the nominal name path-dependent in the ancestry-merge context, so treat them as duck compatible (Yes).
        if self.rails_feature_enabled()
            && let (Some(actual_model), Some(declared_model)) = (
                Self::relation_like_model(actual, &actual_class),
                Self::relation_like_model_from_name(declared_class),
            )
        {
            let models_compatible = match (actual_model.as_deref(), declared_model.as_deref()) {
                (Some(a), Some(d)) => a == d,
                _ => true,
            };
            if models_compatible {
                return ArgCompat::Yes;
            }
        }
        self.ensure_external_class(&actual_class);
        self.ensure_external_class(declared_class);
        if self.class_matches_kind_of_target(&actual_class, declared_class) {
            return ArgCompat::Yes;
        }
        if self.class_matches_kind_of_target(declared_class, &actual_class) {
            return ArgCompat::Unknown;
        }
        if self.class_is_module(declared_class) || self.class_is_module(&actual_class) {
            return ArgCompat::Unknown;
        }
        if !self.class_is_known(&actual_class) || !self.class_is_known(declared_class) {
            return ArgCompat::Unknown;
        }
        // A method_missing on the declared side is Unknown, symmetric with the receiver gate (its surface can't be judged statically).
        if self.receiver_defines_method_missing(declared_class) {
            return ArgCompat::Unknown;
        }
        ArgCompat::No
    }

    fn is_active_record_relation_base(name: &str) -> bool {
        matches!(
            name,
            "ActiveRecord::Relation"
                | "ActiveRecord::AssociationRelation"
                | "ActiveRecord::Associations::CollectionProxy"
        )
    }

    fn tapioca_relation_owner(name: &str) -> Option<&str> {
        let (owner, last) = name.rsplit_once("::")?;
        let is_alias = matches!(
            last,
            "PrivateRelation"
                | "PrivateRelationWhereChain"
                | "PrivateAssociationRelation"
                | "PrivateAssociationRelationWhereChain"
                | "PrivateCollectionProxy"
        ) || last.starts_with("ActiveRecord_");
        (is_alias && !owner.is_empty()).then_some(owner)
    }

    fn relation_like_model_from_name(name: &str) -> Option<Option<String>> {
        let name = name.trim_scope_prefix();
        if Self::is_active_record_relation_base(name) {
            return Some(None);
        }
        Self::tapioca_relation_owner(name).map(|owner| Some(owner.trim_scope_prefix().to_string()))
    }

    fn relation_like_model(actual: &Type, actual_class: &str) -> Option<Option<String>> {
        if let Type::Generic { base, args } = actual
            && Self::is_active_record_relation_base(base.as_str())
        {
            let model = match args.first() {
                Some(Type::Class(m)) => Some(m.trim_scope_prefix().to_string()),
                _ => None,
            };
            return Some(model);
        }
        Self::relation_like_model_from_name(actual_class)
    }

    fn class_is_module(&self, class_name: &str) -> bool {
        self.registry
            .class_data_for(class_name)
            .is_some_and(|data| data.is_module)
    }

    fn class_is_known(&self, class_name: &str) -> bool {
        Self::is_core_builtin_class(class_name)
            || self.registry.class_data_for(class_name).is_some()
    }

    fn is_core_builtin_class(class_name: &str) -> bool {
        matches!(
            class_name,
            "Integer"
                | "Float"
                | "String"
                | "Symbol"
                | "NilClass"
                | "TrueClass"
                | "FalseClass"
                | "Array"
                | "Hash"
                | "Range"
                | "Regexp"
                | "Proc"
        )
    }

    fn type_is_unconstrained_for_check(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Untyped
                | Type::Todo
                | Type::Top
                | Type::Void
                | Type::SelfType
                | Type::InstanceType
                | Type::ParamRef(_)
                | Type::KeywordParamRef(_)
                | Type::IvarRef(_)
                | Type::MethodReturnRef(_, _)
                | Type::ReceiverMethodRef(_, _)
                | Type::BlockReturnRef
                | Type::PatternIndexRef(_, _)
                | Type::PatternRestRef(_)
                | Type::PatternKeyRef(_, _)
                | Type::PatternKeyRestRef(_, _)
        )
    }

    fn actual_is_concrete_non_bool(actual: &Type) -> bool {
        matches!(
            actual,
            Type::Integer
                | Type::Float
                | Type::String
                | Type::Symbol
                | Type::Nil
                | Type::LiteralInteger(_)
                | Type::LiteralFloat(_)
                | Type::LiteralString(_)
                | Type::LiteralSymbol(_)
                | Type::Array(_)
                | Type::Tuple(_)
                | Type::Hash(_, _)
                | Type::Record(_)
                | Type::Class(_)
                | Type::Generic { .. }
                | Type::Singleton(_)
        )
    }

    fn actual_is_concrete_non_collection(actual: &Type) -> bool {
        matches!(
            actual,
            Type::Integer
                | Type::Float
                | Type::String
                | Type::Symbol
                | Type::Bool
                | Type::True
                | Type::False
                | Type::Nil
                | Type::LiteralInteger(_)
                | Type::LiteralFloat(_)
                | Type::LiteralString(_)
                | Type::LiteralSymbol(_)
        )
    }

    fn actual_is_array_like(&self, actual: &Type) -> bool {
        matches!(actual, Type::Array(_) | Type::Tuple(_))
    }

    pub fn method_definition_sig_at(
        &mut self,
        byte_offset: usize,
    ) -> Option<crate::types::MethodSig> {
        let snap = self.find_best_snapshot(byte_offset)?.clone();
        let HoverTarget::MethodDefinition {
            ref owner_type,
            is_singleton,
        } = snap.target
        else {
            return None;
        };
        let resolved_owner = self.resolve_type_for_hover(
            owner_type,
            &snap.class_context,
            snap.method_context.as_deref(),
        );
        let current =
            self.resolve_hover_definition_method_sig(&snap.name, &resolved_owner, is_singleton);
        self.maybe_improve_method_definition_sig_from_external(
            &snap,
            &snap.name,
            is_singleton,
            current,
        )
    }

    pub(crate) fn definition_lookup_target_at(
        &mut self,
        byte_offset: usize,
    ) -> Option<DefinitionLookupTarget> {
        if let Some(snap) = self.find_best_definition_snapshot_for_lookup(byte_offset) {
            return Some(snap.target.clone());
        }

        let snap = self
            .find_best_snapshot_for_definition_lookup(byte_offset)?
            .clone();
        match snap.target {
            HoverTarget::Value(ty) => {
                let resolved = self.resolve_type_for_hover(
                    &ty,
                    &snap.class_context,
                    snap.method_context.as_deref(),
                );
                if matches!(resolved, Type::Singleton(_))
                    && Self::definition_looks_like_constant_reference(&snap.name)
                {
                    Some(DefinitionLookupTarget::TypeDefinition(resolved))
                } else {
                    None
                }
            }
            HoverTarget::MethodCall { receiver_type, .. } => {
                let resolved_receiver = self.resolve_type_for_hover(
                    &receiver_type,
                    &snap.class_context,
                    snap.method_context.as_deref(),
                );
                Some(DefinitionLookupTarget::MethodCall {
                    receiver_type: resolved_receiver,
                    method_name: snap.name,
                })
            }
            HoverTarget::MethodDefinition {
                owner_type,
                is_singleton,
            } => {
                let resolved_owner = self.resolve_type_for_hover(
                    &owner_type,
                    &snap.class_context,
                    snap.method_context.as_deref(),
                );
                Some(DefinitionLookupTarget::MethodDefinition {
                    owner_type: resolved_owner,
                    method_name: snap.name,
                    is_singleton,
                })
            }
        }
    }

    fn definition_looks_like_constant_reference(name: &str) -> bool {
        name.rsplit("::")
            .next()
            .and_then(|segment| segment.chars().next())
            .is_some_and(|ch| ch == '_' || ch.is_ascii_uppercase())
    }

    pub fn find_hover_at(
        &mut self,
        source: &str,
        byte_offset: usize,
    ) -> Option<crate::parser::HoverResult> {
        let snap = match self.find_best_snapshot(byte_offset) {
            Some(snap) => snap.clone(),
            None => {
                return self
                    .resolve_block_param_hover_from_source(source, byte_offset)
                    .or_else(|| self.resolve_token_hover_from_source(source, byte_offset));
            }
        };
        let name = snap.name.clone();
        let target = snap.target.clone();
        let class_context = snap.class_context.clone();
        let method_context = snap.method_context.clone();
        let can_enrich_from_workspace = match &target {
            HoverTarget::Value(ty) => Self::hover_value_may_gain_from_workspace(ty),
            HoverTarget::MethodDefinition { .. } => true,
            _ => false,
        };
        let (resolved, display_rbs, type_params, unresolved_method) = match target {
            HoverTarget::Value(ref ty) => {
                let unresolved = Self::describe_unresolved_ref(ty);
                let resolved =
                    self.resolve_type_for_hover(ty, &class_context, method_context.as_deref());
                let type_params = self.resolve_hover_type_params(&resolved);
                let unresolved = if matches!(resolved, Type::Untyped | Type::Todo) {
                    if self.method_call_is_discoverable(ty) {
                        None
                    } else {
                        unresolved
                    }
                } else {
                    None
                };
                (resolved, None, type_params, unresolved)
            }
            HoverTarget::MethodCall {
                receiver_type,
                ref result_type,
            } => {
                let unresolved = Self::describe_unresolved_ref(result_type);
                let resolved_receiver = self.resolve_type_for_hover(
                    &receiver_type,
                    &class_context,
                    method_context.as_deref(),
                );
                let resolved_result = self.resolve_type_for_hover(
                    result_type,
                    &class_context,
                    method_context.as_deref(),
                );
                let display_rbs = self.resolve_hover_method_signature(&name, &resolved_receiver);
                let type_params = self.resolve_hover_type_params(&resolved_receiver);
                let unresolved = if matches!(resolved_result, Type::Untyped | Type::Todo) {
                    if self.method_call_is_discoverable(result_type) {
                        None
                    } else {
                        unresolved
                    }
                } else {
                    None
                };
                (resolved_result, display_rbs, type_params, unresolved)
            }
            HoverTarget::MethodDefinition {
                owner_type,
                is_singleton,
            } => {
                let resolved_owner = self.resolve_type_for_hover(
                    &owner_type,
                    &class_context,
                    method_context.as_deref(),
                );
                let method_sig =
                    self.resolve_hover_definition_method_sig(&name, &resolved_owner, is_singleton);
                let resolved_result = method_sig
                    .as_ref()
                    .map(|sig| sig.return_type.clone())
                    .unwrap_or(Type::Untyped);
                let display_rbs = method_sig.as_ref().map(format_hover_callable_type);
                let type_params = self.resolve_hover_type_params(&resolved_owner);
                (resolved_result, display_rbs, type_params, None)
            }
        };
        let display_rbs =
            self.maybe_improve_hover_signature_from_external(&snap, &name, display_rbs);
        Some(crate::parser::HoverResult {
            name,
            ty: resolved,
            display_rbs,
            type_params,
            can_enrich_from_workspace,
            unresolved_method,
        })
    }

    fn find_best_snapshot(&self, byte_offset: usize) -> Option<&HoverSnapshot> {
        let mut best: Option<&HoverSnapshot> = None;
        for snap in &self.var_snapshots {
            if byte_offset >= snap.start
                && byte_offset < snap.end
                && best
                    .as_ref()
                    .is_none_or(|b| (snap.end - snap.start) < (b.end - b.start))
            {
                best = Some(snap);
            }
        }
        best
    }

    fn find_best_snapshot_for_definition_lookup(
        &self,
        byte_offset: usize,
    ) -> Option<&HoverSnapshot> {
        self.find_best_snapshot(byte_offset).or_else(|| {
            byte_offset
                .checked_sub(1)
                .and_then(|offset| self.find_best_snapshot(offset))
        })
    }

    fn find_best_definition_snapshot(&self, byte_offset: usize) -> Option<&DefinitionSnapshot> {
        let mut best: Option<&DefinitionSnapshot> = None;
        for snap in &self.definition_snapshots {
            if byte_offset >= snap.start
                && byte_offset < snap.end
                && best
                    .as_ref()
                    .is_none_or(|b| (snap.end - snap.start) < (b.end - b.start))
            {
                best = Some(snap);
            }
        }
        best
    }

    fn find_best_definition_snapshot_for_lookup(
        &self,
        byte_offset: usize,
    ) -> Option<&DefinitionSnapshot> {
        self.find_best_definition_snapshot(byte_offset).or_else(|| {
            byte_offset
                .checked_sub(1)
                .and_then(|offset| self.find_best_definition_snapshot(offset))
        })
    }

    fn describe_unresolved_ref(ty: &Type) -> Option<String> {
        match ty {
            Type::MethodReturnRef(class, method) => Some(format!("{class}#{method}")),
            Type::ReceiverMethodRef(receiver, method) => {
                let inner = match receiver.as_ref() {
                    Type::Class(name) | Type::Singleton(name) => name.as_str(),
                    _ => return None,
                };
                Some(format!("{inner}#{method}"))
            }
            _ => None,
        }
    }

    fn diagnostic_owner_is_known(&mut self, owner: &str) -> bool {
        let owner = owner.trim_scope_prefix();
        if self.registry.is_user_defined_class(owner) {
            return true;
        }
        // Pull stdlib / lazy `.rbi` / external `.rbs` for the owner, then accept it only if it carries real declarations.
        // A speculative `Foo.new` for an undefined constant leaves an *empty* class stub with no source location, methods, superclass or mixins;
        self.ensure_external_class(owner);
        self.ensure_stdlib_class(owner);
        self.registry
            .class_data_for(owner)
            .is_some_and(crate::registry::ClassData::has_type_substance)
    }

    fn receiver_defines_method_missing(&mut self, class_name: &str) -> bool {
        self.ensure_class_available(class_name);
        let mut stack = vec![class_name.trim_scope_prefix().to_string()];
        let mut seen = BTreeSet::new();
        while let Some(current) = stack.pop() {
            if seen.len() > 64 || !seen.insert(current.clone()) {
                continue;
            }
            if matches!(current.as_str(), "Object" | "BasicObject" | "Kernel") {
                continue;
            }
            if self.registry.has_method_named(&current, "method_missing") {
                return true;
            }
            if let Some(data) = self.registry.class_data_for(&current) {
                if let Some(superclass) = data.superclass.as_ref() {
                    stack.push(superclass.trim_scope_prefix().to_string());
                }
                for mixin in &data.mixins {
                    stack.push(mixin.module_name.trim_scope_prefix().to_string());
                }
            }
        }
        false
    }

    fn unresolved_method_matches_snapshot_name(
        unresolved_method: &str,
        snapshot_name: &str,
    ) -> bool {
        unresolved_method
            .rsplit_once('#')
            .is_some_and(|(_, method)| method == snapshot_name)
    }

    fn module_call_resolvable_via_includers(
        &mut self,
        unresolved_method: &str,
        method_name: &str,
    ) -> bool {
        let Some((owner, _)) = unresolved_method.rsplit_once('#') else {
            return false;
        };
        let owner = owner.trim_scope_prefix();

        let owner_is_module = self
            .registry
            .class_data_for(owner)
            .is_some_and(|data| data.is_module)
            || self
                .external_rbs
                .is_some_and(|w| w.class_data_for(owner).is_some_and(|data| data.is_module));
        if !owner_is_module {
            return false;
        }

        let mut includers: Vec<(String, bool)> = Vec::new();
        self.collect_module_includers(owner, false, &mut includers);
        if let Some(enclosing) = owner.strip_suffix("::ClassMethods") {
            self.collect_module_includers(enclosing, true, &mut includers);
        }

        // A module with no static mixin edges has an unknown runtime host, so don't declare it missing.
        if includers.is_empty() {
            crate::diagnostics::record_gating_suppression(
                crate::diagnostics::GatingReason::RuntimeMixinHost,
            );
            return true;
        }

        for (includer, prefer_singleton) in includers.iter().take(32) {
            if includer.as_str() == owner {
                continue;
            }
            if let Some(workspace) = self.external_rbs
                && workspace
                    .lookup_method_return_type_with_hint(includer, method_name, *prefer_singleton)
                    .is_some()
            {
                return true;
            }
            // Pull in the includer's inheritance info (e.g. Grape API detection) via lazy merge
            // before querying the local registry / plugins.
            self.ensure_class_available(includer);
            if self
                .registry
                .lookup_method_return_type_with_hint(includer, method_name, *prefer_singleton)
                .is_some()
            {
                return true;
            }
            let receiver = if *prefer_singleton {
                Type::Singleton(Sym::new(includer.as_str()))
            } else {
                Type::Class(Sym::new(includer.as_str()))
            };
            if self
                .dsl_plugin_method_return(&receiver, method_name)
                .is_some()
            {
                return true;
            }
            // A bare call on a module is evaluated against the includer's surface; if that surface is unknowable, don't declare it missing.
            self.ensure_class_available_with_ancestors(includer, 0);
            if !self
                .registry
                .method_surface_knowledge_complete(includer, self.lazy_rbi_loader.is_some())
            {
                crate::diagnostics::record_gating_suppression(
                    crate::diagnostics::GatingReason::IncompleteAncestors,
                );
                return true;
            }
            if self.includer_has_opaque_mixin_hook(includer) {
                crate::diagnostics::record_gating_suppression(
                    crate::diagnostics::GatingReason::IncompleteAncestors,
                );
                return true;
            }
        }
        false
    }

    fn includer_has_opaque_mixin_hook(&mut self, includer: &str) -> bool {
        let Some(data) = self.registry.class_data_for(includer) else {
            return false;
        };
        let mixin_names: Vec<String> = data
            .mixins
            .iter()
            .map(|m| m.module_name.as_ref().to_string())
            .collect();
        for name in mixin_names {
            let fqn = self.registry.resolve_scoped_class_ref(includer, &name);
            self.ensure_class_available(&fqn);
            if self.registry.is_user_defined_class(&fqn) {
                continue;
            }
            if ["included", "extended", "prepended"]
                .iter()
                .any(|hook| self.registry.has_method_variant(&fqn, hook, true))
            {
                return true;
            }
        }
        false
    }

    fn collect_module_includers(
        &self,
        module_name: &str,
        prefer_singleton: bool,
        out: &mut Vec<(String, bool)>,
    ) {
        if self
            .registry
            .class_data_for(module_name)
            .is_some_and(|data| data.is_module)
        {
            out.extend(
                self.registry
                    .includers_of(module_name)
                    .iter()
                    .map(|name| (name.to_string(), prefer_singleton)),
            );
        }
        if let Some(workspace) = self.external_rbs
            && workspace
                .class_data_for(module_name)
                .is_some_and(|data| data.is_module)
        {
            out.extend(
                workspace
                    .includers_of(module_name)
                    .iter()
                    .map(|name| (name.to_string(), prefer_singleton)),
            );
        }
    }

    fn is_known_class_body_dsl_unresolved_call(
        snapshot: &HoverSnapshot,
        unresolved_method: &str,
    ) -> bool {
        known_class_body_dsl_method(&snapshot.name)
            && Self::is_class_body_self_call(snapshot, unresolved_method)
    }

    fn is_class_body_self_call(snapshot: &HoverSnapshot, unresolved_method: &str) -> bool {
        if snapshot.method_context.is_some() {
            return false;
        }
        unresolved_method
            .rsplit_once('#')
            .is_some_and(|(owner, _)| {
                owner.trim_scope_prefix() == snapshot.class_context.trim_scope_prefix()
            })
    }

    fn resolve_type_for_hover(
        &mut self,
        ty: &Type,
        class_context: &str,
        method_context: Option<&str>,
    ) -> Type {
        match ty {
            Type::ParamRef(idx) => {
                self.resolve_param_ref_for_hover(class_context, method_context, *idx)
            }
            Type::KeywordParamRef(name) => {
                self.resolve_keyword_param_ref_for_hover(class_context, method_context, name)
            }
            Type::IvarRef(ivar_name) => self
                .registry
                .lookup_ivar_type(class_context, ivar_name)
                .unwrap_or(Type::Todo),
            Type::MethodReturnRef(ref_class, method_name) => {
                self.resolve_hover_method_return(ref_class, method_name, false)
            }
            Type::ReceiverMethodRef(receiver_type, method_name) => {
                let resolved_receiver =
                    self.resolve_type_for_hover(receiver_type, class_context, method_context);
                if method_name == "each" {
                    return resolved_receiver;
                }
                let lookup_receiver = if Self::contains_nil(&resolved_receiver) {
                    Self::remove_nil(&resolved_receiver)
                } else {
                    resolved_receiver.clone()
                };
                let class_name = TypeRegistry::type_to_class_name_pub(&lookup_receiver);
                if let Some(cls) = class_name {
                    let prefer_singleton = matches!(lookup_receiver, Type::Singleton(_));
                    self.resolve_hover_method_return(&cls, method_name, prefer_singleton)
                } else {
                    Type::Untyped
                }
            }
            Type::Union(parts) => {
                let resolved: Vec<Type> = parts
                    .iter()
                    .map(|t| self.resolve_type_for_hover(t, class_context, method_context))
                    .collect();
                Type::from_type_vec_preserve_untyped(resolved)
            }
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(self.resolve_type_for_hover(
                inner,
                class_context,
                method_context,
            )))),
            _ => ty.clone(),
        }
    }

    fn resolve_param_ref_for_hover(
        &mut self,
        class_name: &str,
        method_name: Option<&str>,
        idx: usize,
    ) -> Type {
        self.ensure_class_available(class_name);
        let method_name = match method_name {
            Some(n) => n,
            None => return Type::Todo,
        };
        let current = self
            .registry
            .lookup_method_sig(class_name, method_name)
            .and_then(|sig| sig.params.get(idx).map(|param| param.param_type.widen()))
            .unwrap_or(Type::Todo);
        let Some(external) = self.external_rbs else {
            return current;
        };
        let candidate = external
            .lookup_method_sig(class_name, method_name)
            .and_then(|sig| sig.params.get(idx).map(|param| param.param_type.widen()))
            .unwrap_or(Type::Todo);
        Self::choose_richer_hover_type(current, candidate)
    }

    fn resolve_keyword_param_ref_for_hover(
        &mut self,
        class_name: &str,
        method_name: Option<&str>,
        kw_name: &str,
    ) -> Type {
        self.ensure_class_available(class_name);
        let method_name = match method_name {
            Some(n) => n,
            None => return Type::Todo,
        };
        let current = self
            .registry
            .lookup_method_sig(class_name, method_name)
            .and_then(|sig| {
                sig.params
                    .iter()
                    .find(|param| param.name == kw_name)
                    .map(|param| param.param_type.widen())
            })
            .unwrap_or(Type::Todo);
        let Some(external) = self.external_rbs else {
            return current;
        };
        let candidate = external
            .lookup_method_sig(class_name, method_name)
            .and_then(|sig| {
                sig.params
                    .iter()
                    .find(|param| param.name == kw_name)
                    .map(|param| param.param_type.widen())
            })
            .unwrap_or(Type::Todo);
        Self::choose_richer_hover_type(current, candidate)
    }

    fn resolve_hover_method_return(
        &mut self,
        class_name: &str,
        method_name: &str,
        prefer_singleton: bool,
    ) -> Type {
        self.resolve_hover_method_return_opt(class_name, method_name, prefer_singleton)
            .unwrap_or(Type::Untyped)
    }

    fn resolve_hover_method_return_opt(
        &mut self,
        class_name: &str,
        method_name: &str,
        prefer_singleton: bool,
    ) -> Option<Type> {
        let class_name = class_name.trim_scope_prefix();
        self.preload_hover_lookup_hierarchy(class_name);
        let lookup_receiver_type = if prefer_singleton {
            Type::Singleton(Sym::new(class_name))
        } else {
            Type::Class(Sym::new(class_name))
        };
        if let Some(ty) = self.synthetic_i18n_method_return(&lookup_receiver_type, method_name) {
            return Some(ty);
        }
        if let Some(ty) =
            self.synthetic_rails_singleton_method_return(&lookup_receiver_type, method_name)
        {
            return Some(ty);
        }
        if let Some(ty) =
            self.synthetic_rails_configuration_method_return(&lookup_receiver_type, method_name)
        {
            return Some(ty);
        }
        if let Some(ty) = self
            .synthetic_active_support_cache_store_method_return(&lookup_receiver_type, method_name)
        {
            return Some(ty);
        }
        if let Some(ty) = self
            .synthetic_action_controller_helper_method_return(&lookup_receiver_type, method_name)
        {
            return Some(ty);
        }
        if let Some(ty) =
            self.synthetic_action_dispatch_request_method_return(&lookup_receiver_type, method_name)
        {
            return Some(ty);
        }
        if !prefer_singleton {
            if let Some(ty) =
                self.synthetic_action_controller_method_return(&lookup_receiver_type, method_name)
            {
                return Some(ty);
            }
            if let Some(ty) =
                self.synthetic_active_support_hash_method_return(&lookup_receiver_type, method_name)
            {
                return Some(ty);
            }
            if let Some(ty) = self
                .synthetic_active_support_duration_method_return(&lookup_receiver_type, method_name)
            {
                return Some(ty);
            }
        }
        if let Some(ty) = self.registry.lookup_method_return_type_with_hint(
            class_name,
            method_name,
            prefer_singleton,
        ) {
            return Some(ty);
        }
        if let Some(external) = self.external_rbs
            && let Some(ty) =
                external.lookup_method_return_type_via_including_classes(class_name, method_name)
        {
            return Some(ty);
        }
        None
    }

    fn method_call_is_discoverable(&mut self, ty: &Type) -> bool {
        match ty {
            Type::MethodReturnRef(class, method) => {
                Self::is_universal_object_method_for_diagnostics(method)
                    || self
                        .resolve_hover_method_return_opt(class, method, false)
                        .is_some()
            }
            Type::ReceiverMethodRef(receiver, method) => {
                if Self::is_universal_object_method_for_diagnostics(method) {
                    return true;
                }
                let receiver_class = match receiver.as_ref() {
                    Type::Class(name) | Type::Singleton(name) => *name,
                    _ => return false,
                };
                let prefer_singleton = matches!(receiver.as_ref(), Type::Singleton(_));
                self.resolve_hover_method_return_opt(&receiver_class, method, prefer_singleton)
                    .is_some()
            }
            _ => false,
        }
    }

    fn is_universal_object_method_for_diagnostics(method_name: &str) -> bool {
        matches!(
            method_name,
            "__id__"
                | "class"
                | "equal?"
                | "eql?"
                | "frozen?"
                | "hash"
                | "inspect"
                | "instance_of?"
                | "is_a?"
                | "itself"
                | "kind_of?"
                | "method"
                | "methods"
                | "nil?"
                | "object_id"
                | "private_methods"
                | "protected_methods"
                | "public_method"
                | "public_methods"
                | "respond_to?"
                | "tap"
                | "then"
                | "to_s"
                | "yield_self"
        )
    }

    fn resolve_block_param_hover_from_source(
        &mut self,
        source: &str,
        byte_offset: usize,
    ) -> Option<crate::parser::HoverResult> {
        let bytes = source.as_bytes();
        let line_start = bytes[..byte_offset]
            .iter()
            .rposition(|b| *b == b'\n')
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let line_end = bytes[byte_offset..]
            .iter()
            .position(|b| *b == b'\n')
            .map(|idx| byte_offset + idx)
            .unwrap_or(bytes.len());
        let line = &source[line_start..line_end];

        let rel_offset = byte_offset.checked_sub(line_start)?;
        let open_rel = line[..rel_offset].rfind('|')?;
        let close_rel = line[open_rel + 1..].find('|')? + open_rel + 1;
        if !(open_rel < rel_offset && rel_offset < close_rel) {
            return None;
        }

        let inner = &line[open_rel + 1..close_rel];
        let mut search_from = 0usize;
        let mut selected: Option<(usize, String)> = None;
        for (index, raw_part) in inner.split(',').enumerate() {
            let trimmed = raw_part.trim();
            if trimmed.is_empty() {
                continue;
            }
            let rel = inner[search_from..].find(trimmed)?;
            let start = line_start + open_rel + 1 + search_from + rel;
            let end = start + trimmed.len();
            if byte_offset >= start && byte_offset < end {
                selected = Some((index, trimmed.to_string()));
                break;
            }
            search_from += rel + trimmed.len();
        }
        let (param_index, name) = selected?;

        let prefix = line[..open_rel].trim_end();
        let call_prefix = prefix.strip_suffix("do").unwrap_or(prefix).trim_end();
        let method_dot = call_prefix.rfind('.')?;
        let method_name = call_prefix[method_dot + 1..].trim();
        let receiver_prefix = &call_prefix[..method_dot];
        let receiver_end = receiver_prefix.len();
        let receiver_start = receiver_prefix[..receiver_end]
            .char_indices()
            .rev()
            .find(|(_, ch)| !(*ch == '_' || ch.is_ascii_alphanumeric()))
            .map(|(idx, ch)| idx + ch.len_utf8())
            .unwrap_or(0);
        let receiver_offset = line_start + receiver_start;

        let receiver_snap = self.find_best_snapshot(receiver_offset)?;
        let receiver_target = receiver_snap.target.clone();
        let receiver_class_context = receiver_snap.class_context.clone();
        let receiver_method_context = receiver_snap.method_context.clone();
        let receiver_ty = match receiver_target {
            HoverTarget::Value(ty) => self.resolve_type_for_hover(
                &ty,
                &receiver_class_context,
                receiver_method_context.as_deref(),
            ),
            HoverTarget::MethodCall {
                receiver_type,
                result_type,
            } => {
                let _ = receiver_type;
                self.resolve_type_for_hover(
                    &result_type,
                    &receiver_class_context,
                    receiver_method_context.as_deref(),
                )
            }
            HoverTarget::MethodDefinition { .. } => return None,
        };

        let ty = match (method_name, param_index) {
            ("each" | "map" | "select" | "filter" | "reject" | "filter_map", 0) => {
                Self::extract_element_type(&receiver_ty)
            }
            ("each_with_index", 0) => Self::extract_element_type(&receiver_ty),
            ("each_with_index", 1) => Type::Integer,
            ("tap" | "then" | "yield_self", 0) => receiver_ty,
            _ => return None,
        };

        Some(crate::parser::HoverResult {
            name,
            ty,
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        })
    }

    fn resolve_token_hover_from_source(
        &mut self,
        source: &str,
        byte_offset: usize,
    ) -> Option<crate::parser::HoverResult> {
        let bytes = source.as_bytes();
        if Self::is_double_colon_delimiter_at(bytes, byte_offset) {
            return None;
        }
        let (start, end) = Self::token_range_at(bytes, byte_offset)?;
        let raw = source.get(start..end)?;
        let name = raw.trim_end_matches(':').to_string();
        if name.is_empty() {
            return None;
        }

        if let Some(result) = self.resolve_named_snapshot_hover(&name, byte_offset) {
            return Some(result);
        }

        let ty = Self::infer_token_hover_type(&name);
        Some(crate::parser::HoverResult {
            name: name.clone(),
            ty,
            display_rbs: None,
            type_params: Vec::new(),
            can_enrich_from_workspace: false,
            unresolved_method: None,
        })
    }

    fn resolve_named_snapshot_hover(
        &mut self,
        name: &str,
        byte_offset: usize,
    ) -> Option<crate::parser::HoverResult> {
        let snap = self
            .var_snapshots
            .iter()
            .filter(|snap| snap.name == name && matches!(snap.target, HoverTarget::Value(_)))
            .min_by_key(|snap| {
                let distance = if byte_offset < snap.start {
                    snap.start - byte_offset
                } else if byte_offset >= snap.end {
                    byte_offset.saturating_sub(snap.end)
                } else {
                    0
                };
                (distance, byte_offset.abs_diff(snap.start))
            })?
            .clone();

        let raw_target = match snap.target {
            HoverTarget::Value(ty) => ty,
            _ => return None,
        };
        let can_enrich_from_workspace = Self::hover_value_may_gain_from_workspace(&raw_target);
        let resolved = self.resolve_type_for_hover(
            &raw_target,
            &snap.class_context,
            snap.method_context.as_deref(),
        );
        let type_params = self.resolve_hover_type_params(&resolved);
        let unresolved = if matches!(resolved, Type::Untyped | Type::Todo) {
            Self::describe_unresolved_ref(&raw_target)
        } else {
            None
        };
        Some(crate::parser::HoverResult {
            name: snap.name,
            ty: resolved,
            display_rbs: None,
            type_params,
            can_enrich_from_workspace,
            unresolved_method: unresolved,
        })
    }

    fn is_double_colon_delimiter_at(bytes: &[u8], byte_offset: usize) -> bool {
        bytes.get(byte_offset).is_some_and(|byte| {
            *byte == b':'
                && (bytes.get(byte_offset + 1) == Some(&b':')
                    || byte_offset
                        .checked_sub(1)
                        .is_some_and(|offset| bytes.get(offset) == Some(&b':')))
        })
    }

    fn token_range_at(bytes: &[u8], byte_offset: usize) -> Option<(usize, usize)> {
        if bytes.is_empty() {
            return None;
        }

        let mut idx = byte_offset.min(bytes.len().saturating_sub(1));
        if !Self::is_hover_token_byte(bytes[idx]) {
            if idx > 0 && Self::is_hover_token_byte(bytes[idx - 1]) {
                idx -= 1;
            } else {
                return None;
            }
        }

        let mut start = idx;
        while start > 0 && Self::is_hover_token_byte(bytes[start - 1]) {
            start -= 1;
        }

        let mut end = idx + 1;
        while end < bytes.len() && Self::is_hover_token_byte(bytes[end]) {
            end += 1;
        }

        Some((start, end))
    }

    fn is_hover_token_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'?' | b'!' | b'@' | b'$' | b':')
    }

    fn infer_token_hover_type(token: &str) -> Type {
        match token {
            "true" => Type::True,
            "false" => Type::False,
            "nil" => Type::Nil,
            "self" => Type::SelfType,
            _ if token.starts_with(':') && token.len() > 1 => {
                Type::LiteralSymbol(Sym::new(&token[1..]))
            }
            _ if token.chars().all(|ch| ch.is_ascii_digit()) => token
                .parse::<i64>()
                .map(Type::LiteralInteger)
                .unwrap_or(Type::Todo),
            _ => Type::Todo,
        }
    }

    fn resolve_hover_type_params(&mut self, ty: &Type) -> Vec<(String, Type)> {
        let Some(class_name) = Self::hover_type_param_class_name(ty) else {
            return Vec::new();
        };
        self.ensure_class_available(&class_name);
        let type_args = Self::extract_type_args(ty);
        self.class_type_param_pairs_from_args(&class_name, &type_args)
    }

    fn hover_type_param_class_name(ty: &Type) -> Option<String> {
        let class_name = TypeRegistry::type_to_class_name_pub(ty)?;
        Some(
            class_name
                .split_once('[')
                .map(|(bare, _)| bare)
                .unwrap_or(class_name.as_str())
                .to_string(),
        )
    }

    fn hover_value_may_gain_from_workspace(ty: &Type) -> bool {
        match ty {
            Type::ParamRef(_) | Type::KeywordParamRef(_) => true,
            Type::Array(Some(inner)) => Self::hover_value_may_gain_from_workspace(inner),
            Type::Union(parts) => parts.iter().any(Self::hover_value_may_gain_from_workspace),
            _ => false,
        }
    }

    fn resolve_hover_method_signature(
        &mut self,
        method_name: &str,
        receiver_type: &Type,
    ) -> Option<String> {
        let receiver_type = Self::normalize_receiver_type_for_signature(receiver_type);
        if method_name == "new"
            && let Some(method_sig) = self.resolve_hover_constructor_sig(&receiver_type)
        {
            return Some(format_hover_inferred_method_sig(method_name, &method_sig));
        }
        if let Some(method_sig) =
            self.synthetic_active_record_method_sig(&receiver_type, method_name)
        {
            return Some(format_hover_inferred_method_sig(method_name, &method_sig));
        }
        if let Some(overloads) = self.resolve_hover_method_overloads(method_name, &receiver_type) {
            return Some(format_hover_method_sig(method_name, &overloads));
        }

        let method_sig = self.resolve_hover_method_sig(method_name, &receiver_type)?;
        Some(format_hover_inferred_method_sig(method_name, &method_sig))
    }

    fn resolve_hover_constructor_sig(
        &mut self,
        receiver_type: &Type,
    ) -> Option<crate::types::MethodSig> {
        let receiver_class = TypeRegistry::type_to_class_name_pub(receiver_type)?;
        self.preload_hover_lookup_hierarchy(&receiver_class);
        let mut method_sig = self
            .registry
            .lookup_method_sig(&receiver_class, "initialize")?;
        method_sig.return_type = InferenceEngine::class_name_to_type(&receiver_class);
        Some(method_sig)
    }

    fn resolve_hover_method_sig(
        &mut self,
        method_name: &str,
        receiver_type: &Type,
    ) -> Option<crate::types::MethodSig> {
        if let Some(method_sig) =
            self.synthetic_active_record_method_sig(receiver_type, method_name)
        {
            return Some(method_sig);
        }
        let receiver_class = TypeRegistry::type_to_class_name_pub(receiver_type)?;
        self.preload_hover_lookup_hierarchy(&receiver_class);
        let prefer_singleton = matches!(receiver_type, Type::Singleton(_));
        self.registry
            .lookup_method_sig_with_hint(&receiver_class, method_name, prefer_singleton)
    }

    fn resolve_hover_definition_method_sig(
        &mut self,
        method_name: &str,
        owner_type: &Type,
        is_singleton: bool,
    ) -> Option<crate::types::MethodSig> {
        let owner_class = TypeRegistry::type_to_class_name_pub(owner_type)?;
        self.preload_hover_lookup_hierarchy(&owner_class);
        self.registry
            .lookup_method_sig_exact(&owner_class, method_name, is_singleton)
    }

    fn resolve_hover_method_overloads(
        &mut self,
        method_name: &str,
        receiver_type: &Type,
    ) -> Option<Vec<HoverOverloadSig>> {
        let receiver_class = TypeRegistry::type_to_class_name_pub(receiver_type)?;
        self.preload_hover_lookup_hierarchy(&receiver_class);
        let prefer_singleton = matches!(receiver_type, Type::Singleton(_));
        let mut overloads =
            self.collect_owned_rbs_overloads(&receiver_class, method_name, prefer_singleton, false);
        if overloads.is_empty() {
            overloads = self.collect_owned_rbs_overloads(
                &receiver_class,
                method_name,
                prefer_singleton,
                true,
            );
        }
        if overloads.is_empty() {
            return None;
        }

        let receiver_type_args = Self::extract_type_args(receiver_type);
        let mut base_type_vars =
            self.class_type_vars_from_args(&receiver_class, &receiver_type_args);
        Self::seed_enumerable_elem_type_var(
            &receiver_class,
            &receiver_type_args,
            &mut base_type_vars,
        );

        Some(
            overloads
                .iter()
                .map(|overload| {
                    let mut type_vars = self.class_type_vars_for_method_owner(
                        &receiver_class,
                        receiver_type,
                        &overload.owner_class,
                    );
                    for (name, ty) in &base_type_vars {
                        type_vars.entry(name.clone()).or_insert_with(|| ty.clone());
                    }
                    self.concretize_hover_overload(&overload.method_type, receiver_type, &type_vars)
                })
                .collect(),
        )
    }

    fn concretize_hover_overload(
        &self,
        overload: &rbs_ir::MethodType,
        receiver_type: &Type,
        type_vars: &HashMap<String, Type>,
    ) -> HoverOverloadSig {
        HoverOverloadSig {
            params: self.concretize_hover_function_params(
                &overload.function_type,
                receiver_type,
                type_vars,
            ),
            return_type: self.concretize_hover_type(
                &overload.function_type.return_type,
                receiver_type,
                type_vars,
            ),
            block: overload.block.as_ref().map(|block| HoverBlockSig {
                params: self.concretize_hover_function_params(
                    &block.function_type,
                    receiver_type,
                    type_vars,
                ),
                return_type: self.concretize_hover_type(
                    &block.function_type.return_type,
                    receiver_type,
                    type_vars,
                ),
                required: block.required,
            }),
        }
    }

    fn concretize_hover_function_params(
        &self,
        function_type: &rbs_ir::FunctionType,
        receiver_type: &Type,
        type_vars: &HashMap<String, Type>,
    ) -> Vec<Param> {
        let mut params = Vec::new();
        let mut positional_index = 0usize;

        for param in &function_type.required_positionals {
            params.push(Self::build_hover_param(
                param.name,
                positional_index,
                ParamKind::Required,
                &param.type_,
                receiver_type,
                type_vars,
            ));
            positional_index += 1;
        }
        for param in &function_type.optional_positionals {
            params.push(Self::build_hover_param(
                param.name,
                positional_index,
                ParamKind::Optional,
                &param.type_,
                receiver_type,
                type_vars,
            ));
            positional_index += 1;
        }
        if let Some(param) = &function_type.rest_positionals {
            params.push(Self::build_hover_param(
                param.name,
                positional_index,
                ParamKind::Rest,
                &param.type_,
                receiver_type,
                type_vars,
            ));
            positional_index += 1;
        }
        for param in &function_type.trailing_positionals {
            params.push(Self::build_hover_param(
                param.name,
                positional_index,
                ParamKind::Required,
                &param.type_,
                receiver_type,
                type_vars,
            ));
            positional_index += 1;
        }
        for (name, param) in &function_type.required_keywords {
            params.push(Param {
                name: name.to_string(),
                param_type: self.concretize_hover_type(&param.type_, receiver_type, type_vars),
                kind: ParamKind::KeywordRequired,
            });
        }
        for (name, param) in &function_type.optional_keywords {
            params.push(Param {
                name: name.to_string(),
                param_type: self.concretize_hover_type(&param.type_, receiver_type, type_vars),
                kind: ParamKind::KeywordOptional,
            });
        }
        if let Some(param) = &function_type.rest_keywords {
            params.push(Self::build_hover_param(
                param.name,
                positional_index,
                ParamKind::DoubleRest,
                &param.type_,
                receiver_type,
                type_vars,
            ));
        }

        params
    }

    fn build_hover_param(
        name: Option<Sym>,
        positional_index: usize,
        kind: ParamKind,
        rbs_type: &rbs_ir::RbsType,
        receiver_type: &Type,
        type_vars: &HashMap<String, Type>,
    ) -> Param {
        Param {
            name: name
                .map(String::from)
                .unwrap_or_else(|| format!("arg{positional_index}")),
            param_type: Self::concretize_hover_type_static(rbs_type, receiver_type, type_vars),
            kind,
        }
    }

    fn concretize_hover_type(
        &self,
        rbs_type: &rbs_ir::RbsType,
        receiver_type: &Type,
        type_vars: &HashMap<String, Type>,
    ) -> Type {
        Self::concretize_hover_type_static(rbs_type, receiver_type, type_vars)
    }

    fn concretize_hover_type_static(
        rbs_type: &rbs_ir::RbsType,
        receiver_type: &Type,
        type_vars: &HashMap<String, Type>,
    ) -> Type {
        if let rbs_ir::RbsType::Class(name, args) = rbs_type {
            let bare = name.strip_prefix("::").unwrap_or(name);
            return match bare {
                "Integer" if args.is_empty() => Type::Integer,
                "Float" if args.is_empty() => Type::Float,
                "String" if args.is_empty() => Type::String,
                "Symbol" if args.is_empty() => Type::Symbol,
                "Array" if args.is_empty() => Type::Array(None),
                "Array" if args.len() == 1 => Type::Array(Some(Box::new(
                    Self::concretize_hover_type_static(&args[0], receiver_type, type_vars),
                ))),
                "Hash" if args.is_empty() => Type::Hash(None, None),
                "Hash" if args.len() == 2 => Type::Hash(
                    Some(Box::new(Self::concretize_hover_type_static(
                        &args[0],
                        receiver_type,
                        type_vars,
                    ))),
                    Some(Box::new(Self::concretize_hover_type_static(
                        &args[1],
                        receiver_type,
                        type_vars,
                    ))),
                ),
                _ if !args.is_empty() => Type::Generic {
                    base: Sym::new(bare),
                    args: args
                        .iter()
                        .map(|arg| {
                            Self::concretize_hover_type_static(arg, receiver_type, type_vars)
                        })
                        .collect(),
                },
                _ => convert_rbs_type(rbs_type),
            };
        }
        let ty = Self::substitute_rbs_type_vars(rbs_type, type_vars);
        Self::resolve_self_type_static(&ty, receiver_type)
    }

    fn resolve_self_type_static(ty: &Type, receiver_type: &Type) -> Type {
        match ty {
            Type::SelfType => receiver_type.clone(),
            Type::InstanceType => Self::rbs_instance_type_for_receiver(receiver_type),
            Type::Union(parts) => {
                let resolved: Vec<Type> = parts
                    .iter()
                    .map(|part| Self::resolve_self_type_static(part, receiver_type))
                    .collect();
                Type::from_type_vec(resolved)
            }
            Type::Intersection(parts) => Type::Intersection(
                parts
                    .iter()
                    .map(|part| Self::resolve_self_type_static(part, receiver_type))
                    .collect(),
            ),
            Type::Array(Some(inner)) => Type::Array(Some(Box::new(
                Self::resolve_self_type_static(inner, receiver_type),
            ))),
            Type::Hash(Some(key), Some(value)) => Type::Hash(
                Some(Box::new(Self::resolve_self_type_static(key, receiver_type))),
                Some(Box::new(Self::resolve_self_type_static(
                    value,
                    receiver_type,
                ))),
            ),
            Type::Hash(Some(key), None) => Type::Hash(
                Some(Box::new(Self::resolve_self_type_static(key, receiver_type))),
                None,
            ),
            Type::Hash(None, Some(value)) => Type::Hash(
                None,
                Some(Box::new(Self::resolve_self_type_static(
                    value,
                    receiver_type,
                ))),
            ),
            Type::Tuple(parts) => Type::Tuple(
                parts
                    .iter()
                    .map(|part| Self::resolve_self_type_static(part, receiver_type))
                    .collect(),
            ),
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|field| RecordField {
                        key: field.key.clone(),
                        value: Self::resolve_self_type_static(&field.value, receiver_type),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    fn normalize_receiver_type_for_signature(receiver_type: &Type) -> Type {
        let normalized = Self::normalize_union_receiver(receiver_type);
        match normalized {
            Type::Tuple(parts) => Type::Array(Some(Box::new(Type::from_type_vec(
                parts.into_iter().map(|part| part.widen()).collect(),
            )))),
            Type::Record(fields) => Type::Hash(
                Some(Box::new(Type::Symbol)),
                Some(Box::new(Type::from_type_vec(
                    fields
                        .into_iter()
                        .map(|field| field.value.widen())
                        .collect(),
                ))),
            ),
            other => other.widen(),
        }
    }

    fn maybe_improve_hover_signature_from_external(
        &self,
        snap: &HoverSnapshot,
        name: &str,
        current: Option<String>,
    ) -> Option<String> {
        let Some(external) = self.external_rbs else {
            return current;
        };
        let candidate = match snap.target {
            HoverTarget::MethodCall {
                ref receiver_type, ..
            } => {
                let receiver_class = TypeRegistry::type_to_class_name_pub(receiver_type)
                    .unwrap_or_else(|| snap.class_context.clone());
                let prefer_singleton = matches!(receiver_type, Type::Singleton(_));
                let Some(sig) = external
                    .lookup_method_sig_for_receiver_with_hint(
                        &receiver_class,
                        name,
                        prefer_singleton,
                    )
                    .or_else(|| external.lookup_method_sig(&snap.class_context, name))
                else {
                    return current;
                };
                format_hover_inferred_method_sig(name, &sig)
            }
            HoverTarget::MethodDefinition { is_singleton, .. } => {
                let Some(sig) =
                    external.lookup_method_sig_exact(&snap.class_context, name, is_singleton)
                else {
                    return current;
                };
                format_hover_callable_type(&sig)
            }
            HoverTarget::Value(_) => return current,
        };
        match current {
            Some(current_display) => {
                if Self::hover_signature_untyped_slots(&candidate)
                    < Self::hover_signature_untyped_slots(&current_display)
                {
                    Some(candidate)
                } else {
                    Some(current_display)
                }
            }
            None => Some(candidate),
        }
    }

    fn maybe_improve_method_definition_sig_from_external(
        &self,
        snap: &HoverSnapshot,
        name: &str,
        is_singleton: bool,
        current: Option<crate::types::MethodSig>,
    ) -> Option<crate::types::MethodSig> {
        let Some(external) = self.external_rbs else {
            return current;
        };
        let candidate = external.lookup_method_sig_exact(&snap.class_context, name, is_singleton);
        match (current, candidate) {
            (Some(current_sig), Some(candidate_sig)) => {
                let candidate_display = format_hover_callable_type(&candidate_sig);
                let current_display = format_hover_callable_type(&current_sig);
                if Self::hover_signature_untyped_slots(&candidate_display)
                    < Self::hover_signature_untyped_slots(&current_display)
                {
                    Some(candidate_sig)
                } else {
                    Some(current_sig)
                }
            }
            (None, Some(candidate_sig)) => Some(candidate_sig),
            (Some(current_sig), None) => Some(current_sig),
            (None, None) => None,
        }
    }

    fn hover_signature_untyped_slots(display: &str) -> usize {
        display
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| {
                let mut count = 0;
                if let Some(arrow_idx) = line.rfind("->") {
                    let (params_part, return_part) = line.split_at(arrow_idx);
                    let return_part = return_part.trim_start_matches("->").trim();
                    if return_part.contains("untyped") {
                        count += 1;
                    }
                    if let (Some(open_idx), Some(close_idx)) =
                        (params_part.find('('), params_part.rfind(')'))
                    {
                        let params = &params_part[open_idx + 1..close_idx];
                        for segment in params.split(',') {
                            let segment = segment.trim();
                            if !segment.is_empty() && segment.contains("untyped") {
                                count += 1;
                            }
                        }
                    } else if params_part.contains("untyped") {
                        count += 1;
                    }
                } else if line.contains("untyped") {
                    count += 1;
                }
                count
            })
            .sum()
    }

    fn choose_richer_hover_type(current: Type, candidate: Type) -> Type {
        if current == candidate {
            return current;
        }
        match (&current, &candidate) {
            (Type::Untyped | Type::Todo, ty) if !matches!(ty, Type::Untyped | Type::Todo) => {
                return candidate;
            }
            (ty, Type::Untyped | Type::Todo) if !matches!(ty, Type::Untyped | Type::Todo) => {
                return current;
            }
            _ => {}
        }

        let current_parts = Self::hover_type_parts(&current);
        let candidate_parts = Self::hover_type_parts(&candidate);
        if candidate_parts != current_parts {
            if candidate_parts.is_superset(&current_parts)
                && candidate_parts.len() > current_parts.len()
            {
                return candidate;
            }
            if current_parts.is_superset(&candidate_parts)
                && current_parts.len() > candidate_parts.len()
            {
                return current;
            }
        }

        if Self::hover_literal_specificity(&candidate) > Self::hover_literal_specificity(&current) {
            candidate
        } else {
            current
        }
    }

    fn hover_type_parts(ty: &Type) -> BTreeSet<String> {
        let mut parts = BTreeSet::new();
        Self::collect_hover_type_parts(ty, &mut parts);
        parts
    }

    fn collect_hover_type_parts(ty: &Type, parts: &mut BTreeSet<String>) {
        match ty {
            Type::Union(inner) => {
                for part in inner {
                    Self::collect_hover_type_parts(part, parts);
                }
            }
            _ => {
                parts.insert(ty.widen().to_string());
            }
        }
    }

    fn hover_literal_specificity(ty: &Type) -> usize {
        match ty {
            Type::LiteralInteger(_)
            | Type::LiteralFloat(_)
            | Type::LiteralString(_)
            | Type::LiteralSymbol(_)
            | Type::True
            | Type::False => 1,
            Type::Union(parts) => parts.iter().map(Self::hover_literal_specificity).sum(),
            _ => 0,
        }
    }

    fn preload_hover_lookup_hierarchy(&mut self, class_name: &str) {
        let mut seen = HashSet::new();
        self.preload_hover_lookup_hierarchy_inner(class_name, &mut seen);
        self.preload_universal_tail();
    }

    pub(crate) fn preload_universal_tail(&mut self) {
        if self.universal_singleton_tail_preloaded {
            return;
        }
        self.universal_singleton_tail_preloaded = true;
        for universal in ["Object", "Kernel", "BasicObject", "Class", "Module"] {
            self.ensure_class_available(universal);
        }
    }

    fn preload_hover_lookup_hierarchy_inner(
        &mut self,
        class_name: &str,
        seen: &mut HashSet<String>,
    ) {
        if !seen.insert(class_name.to_string()) {
            return;
        }
        self.ensure_class_available(class_name);
        let Some(data) = self.registry.class_data_for(class_name).cloned() else {
            return;
        };
        if let Some(superclass) = data.superclass {
            let resolved = self.resolve_scoped_name_with_external(class_name, superclass.as_ref());
            self.preload_hover_lookup_hierarchy_inner(&resolved, seen);
        }
        for mixin in data.mixins {
            let resolved =
                self.resolve_scoped_name_with_external(class_name, mixin.module_name.as_ref());
            self.preload_hover_lookup_hierarchy_inner(&resolved, seen);
        }
    }

    fn resolve_scoped_name_with_external(&self, scope_class: &str, raw_name: &str) -> String {
        // Walk the scope chain looking for a scoped match in either registry before accepting a bare top-level hit.
        // Skip matches that equal `scope_class` itself (a class cannot be its own superclass / mixin).
        let mut scope: &str = scope_class;
        loop {
            if !scope.is_empty() {
                let candidate = crate::sym::join_scope(scope, raw_name);
                if candidate != scope_class {
                    if self.registry.has_class(&candidate) {
                        return candidate;
                    }
                    if let Some(external) = self.external_rbs
                        && external.has_class(&candidate)
                    {
                        return candidate;
                    }
                }
            }
            if scope.is_empty() {
                break;
            }
            match scope.rfind_scope_sep() {
                Some(idx) => scope = &scope[..idx],
                None => scope = "",
            }
        }
        if raw_name != scope_class {
            if self.registry.has_class(raw_name) {
                return raw_name.to_string();
            }
            if let Some(external) = self.external_rbs
                && external.has_class(raw_name)
            {
                return raw_name.to_string();
            }
        }
        raw_name.to_string()
    }
}

#[cfg(test)]
mod arg_compat_tests {
    use super::*;

    fn engine() -> InferenceEngine<'static> {
        InferenceEngine::new()
    }

    fn opt(t: Type) -> Type {
        Type::from_type_vec_preserve_untyped(vec![t, Type::Nil])
    }

    #[test]
    fn literal_integer_against_string_is_incompatible() {
        let mut e = engine();
        assert_eq!(
            e.arg_compat(&Type::LiteralInteger(1), &Type::String),
            ArgCompat::No
        );
    }

    #[test]
    fn literal_string_against_string_is_compatible() {
        let mut e = engine();
        assert_eq!(
            e.arg_compat(&Type::LiteralString("x".into()), &Type::String),
            ArgCompat::Yes
        );
    }

    #[test]
    fn optional_accepts_nil_and_inner_but_rejects_other() {
        let mut e = engine();
        assert_eq!(e.arg_compat(&Type::Nil, &opt(Type::String)), ArgCompat::Yes);
        assert_eq!(
            e.arg_compat(&Type::String, &opt(Type::String)),
            ArgCompat::Yes
        );
        assert_eq!(
            e.arg_compat(&Type::Integer, &opt(Type::String)),
            ArgCompat::No
        );
    }

    #[test]
    fn untyped_and_unresolved_stay_unknown() {
        let mut e = engine();
        assert_eq!(
            e.arg_compat(&Type::Untyped, &Type::String),
            ArgCompat::Unknown
        );
        assert_eq!(
            e.arg_compat(&Type::Integer, &Type::Untyped),
            ArgCompat::Unknown
        );
        assert_eq!(
            e.arg_compat(&Type::ParamRef(0), &Type::String),
            ArgCompat::Unknown
        );
    }

    #[test]
    fn actual_union_only_flagged_when_every_member_incompatible() {
        let mut e = engine();
        let mixed = Type::from_type_vec(vec![Type::Integer, Type::String]);
        assert_eq!(e.arg_compat(&mixed, &Type::String), ArgCompat::Unknown);
        let all_bad = Type::from_type_vec(vec![Type::Integer, Type::Float]);
        assert_eq!(e.arg_compat(&all_bad, &Type::String), ArgCompat::No);
        let all_good = Type::from_type_vec(vec![Type::String, Type::LiteralString("x".into())]);
        assert_eq!(e.arg_compat(&all_good, &Type::String), ArgCompat::Yes);
    }

    #[test]
    fn exact_literal_param_requires_exact_value() {
        let mut e = engine();
        assert_eq!(
            e.arg_compat(&Type::LiteralInteger(1), &Type::LiteralInteger(1)),
            ArgCompat::Yes
        );
        assert_eq!(
            e.arg_compat(&Type::LiteralInteger(2), &Type::LiteralInteger(1)),
            ArgCompat::No
        );
        // Broadened actual stays unknown (might be the exact value at runtime).
        assert_eq!(
            e.arg_compat(&Type::Integer, &Type::LiteralInteger(1)),
            ArgCompat::Unknown
        );
    }

    #[test]
    fn array_element_mismatch_is_incompatible() {
        let mut e = engine();
        let declared = Type::Array(Some(Box::new(Type::Integer)));
        let bad = Type::Tuple(vec![Type::LiteralString("x".into())]);
        assert_eq!(e.arg_compat(&bad, &declared), ArgCompat::No);
        let good = Type::Tuple(vec![Type::LiteralInteger(1), Type::LiteralInteger(2)]);
        assert_eq!(e.arg_compat(&good, &declared), ArgCompat::Yes);
        // Element-untyped array stays unknown.
        assert_eq!(
            e.arg_compat(&Type::Array(None), &declared),
            ArgCompat::Unknown
        );
    }

    #[test]
    fn literal_bool_param_does_not_reject_other_boolean() {
        let mut e = engine();
        // A `= true` default narrows the param type to literal `true`, but
        // passing `false` / `bool` must not be flagged (it's a boolean param).
        assert_ne!(e.arg_compat(&Type::False, &Type::True), ArgCompat::No);
        assert_ne!(e.arg_compat(&Type::Bool, &Type::True), ArgCompat::No);
        assert_ne!(e.arg_compat(&Type::True, &Type::False), ArgCompat::No);
        assert_eq!(
            e.arg_compat(&Type::LiteralString("x".into()), &Type::True),
            ArgCompat::No
        );
    }

    #[test]
    fn numeric_param_accepts_integer_and_float() {
        let mut e = engine();
        let numeric = Type::Class(crate::types::Sym::new("Numeric"));
        assert_eq!(e.arg_compat(&Type::Integer, &numeric), ArgCompat::Yes);
        assert_eq!(e.arg_compat(&Type::Float, &numeric), ArgCompat::Yes);
        assert_ne!(e.arg_compat(&Type::Integer, &opt(numeric)), ArgCompat::No);
    }

    #[test]
    fn absolute_path_class_name_matches_relative() {
        let mut e = engine();
        let absolute = Type::Class(crate::types::Sym::new("::Foo::Bar"));
        let relative = Type::Class(crate::types::Sym::new("Foo::Bar"));
        assert_eq!(e.arg_compat(&absolute, &relative), ArgCompat::Yes);
        assert_eq!(e.arg_compat(&relative, &absolute), ArgCompat::Yes);
    }

    fn method_def(name: &str) -> crate::registry::MethodDef {
        crate::registry::MethodDef {
            name: crate::types::Sym::new(name),
            param_infos: Vec::new(),
            raw_return_type: Type::Untyped,
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
        }
    }

    #[test]
    fn declared_class_with_method_missing_is_unknown() {
        // If the declared param type's class has method_missing, we can't statically judge
        // whether actual satisfies that surface, so the result is Unknown rather than No.
        let mut e = engine();
        e.registry
            .add_method_def("MashLike", method_def("method_missing"));
        e.registry.add_method_def("Visitor", method_def("name"));
        let actual = Type::Class(crate::types::Sym::new("Visitor"));
        let declared = Type::Class(crate::types::Sym::new("MashLike"));
        assert_eq!(e.arg_compat(&actual, &declared), ArgCompat::Unknown);
    }

    #[test]
    fn declared_class_without_method_missing_still_rejects() {
        // Contrast: two unrelated known classes without method_missing still yield No as before.
        let mut e = engine();
        e.registry.add_method_def("PlainClass", method_def("foo"));
        e.registry.add_method_def("Visitor", method_def("name"));
        let actual = Type::Class(crate::types::Sym::new("Visitor"));
        let declared = Type::Class(crate::types::Sym::new("PlainClass"));
        assert_eq!(e.arg_compat(&actual, &declared), ArgCompat::No);
    }

    #[test]
    fn timewithzone_actual_accepts_time_declared_under_rails() {
        let mut e = engine();
        e.set_rails_mode(true);
        let actual = Type::Class(crate::types::Sym::new("ActiveSupport::TimeWithZone"));
        let declared = Type::Class(crate::types::Sym::new("Time"));
        assert_eq!(e.arg_compat(&actual, &declared), ArgCompat::Yes);
        assert_ne!(e.arg_compat(&actual, &opt(declared)), ArgCompat::No);
    }

    #[test]
    fn timewithzone_actual_not_forced_yes_without_rails() {
        // Without Rails, this isn't a universal relationship, so don't short-circuit to Yes (Rails-specific duck compatibility is limited to rails_feature). TimeWithZone is then an unknown class, so it stays Unknown.
        let mut e = engine();
        let actual = Type::Class(crate::types::Sym::new("ActiveSupport::TimeWithZone"));
        let declared = Type::Class(crate::types::Sym::new("Time"));
        assert_eq!(e.arg_compat(&actual, &declared), ArgCompat::Unknown);
    }

    fn relation_of(model: &str) -> Type {
        Type::Generic {
            base: crate::types::Sym::new("ActiveRecord::Relation"),
            args: vec![Type::Class(crate::types::Sym::new(model))].into(),
        }
    }

    #[test]
    fn tapioca_private_relation_accepts_inferred_relation_under_rails() {
        let mut e = engine();
        e.set_rails_mode(true);
        // Put `Post::PrivateRelation` and `ActiveRecord::Relation` in the "known but with unmerged ancestry" state (i.e. the resolution contract of the interactive LSP display context). In this state, subtype judgment can't resolve in either direction; before the fix, the nominal check fell through to No and produced a diagnostic (in CLI batch mode, ancestry gets merged and it silently resolves to Unknown instead — this path difference was the symptom behind this roadmap item).
        e.registry
            .add_method_def("Post::PrivateRelation", method_def("where"));
        e.registry
            .add_method_def("ActiveRecord::Relation", method_def("where"));
        let actual = relation_of("Post");
        let declared = Type::Class(crate::types::Sym::new("Post::PrivateRelation"));
        assert_eq!(e.arg_compat(&actual, &declared), ArgCompat::Yes);
        let actual_rel = Type::Class(crate::types::Sym::new("Post::PrivateRelation"));
        let declared_rel = relation_of("Post");
        assert_eq!(e.arg_compat(&actual_rel, &declared_rel), ArgCompat::Yes);
        let declared_base = Type::Class(crate::types::Sym::new("ActiveRecord::Relation"));
        assert_eq!(e.arg_compat(&actual_rel, &declared_base), ArgCompat::Yes);
    }

    #[test]
    fn tapioca_private_relation_not_forced_yes_without_rails() {
        // This naming is Rails-specific, so don't short-circuit when rails is disabled.
        // If PrivateRelation is unknown, it stays Unknown (not No).
        let mut e = engine();
        let actual = relation_of("Post");
        let declared = Type::Class(crate::types::Sym::new("Post::PrivateRelation"));
        assert_ne!(e.arg_compat(&actual, &declared), ArgCompat::Yes);
    }

    #[test]
    fn tapioca_private_relation_different_model_is_not_forced_yes() {
        let mut e = engine();
        e.set_rails_mode(true);
        e.registry
            .add_method_def("Post::PrivateRelation", method_def("where"));
        let actual = relation_of("User");
        let declared = Type::Class(crate::types::Sym::new("Post::PrivateRelation"));
        assert_ne!(e.arg_compat(&actual, &declared), ArgCompat::Yes);
    }

    #[test]
    fn unresolved_type_variable_actual_is_unknown() {
        // Fix 2: an unresolved type variable (a phantom `Type::Class("U")` from an unbound `T.type_parameter(:U)` etc. turned nominal) has an unknown concrete value type, so against a concrete class param it's Unknown rather than No. By contrast, an unrelated actual class with real substance still yields No.
        let mut e = engine();
        e.registry.add_method_def("Target", method_def("call"));
        let type_var = Type::Class(Sym::new("U"));
        let target = Type::Class(Sym::new("Target"));
        assert_eq!(e.arg_compat(&type_var, &target), ArgCompat::Unknown);

        e.registry.add_method_def("RealClass", method_def("x"));
        let real = Type::Class(Sym::new("RealClass"));
        assert_eq!(e.arg_compat(&real, &target), ArgCompat::No);
    }

    #[test]
    fn rbs_structural_liberal_detects_interfaces_and_duck_aliases() {
        let iface = rbs_ir::RbsType::Class(Sym::new("_ToPath"), Box::new([]));
        assert!(InferenceEngine::rbs_type_is_structural_liberal(&iface));

        for alias in ["string", "path", "int", "io", "interned", "encoding"] {
            let ty = rbs_ir::RbsType::Alias(Sym::new(alias), Box::new([]));
            assert!(
                InferenceEngine::rbs_type_is_structural_liberal(&ty),
                "alias `{alias}` should be treated as structural-liberal"
            );
        }

        let union = rbs_ir::RbsType::Union(Box::new([
            rbs_ir::RbsType::Alias(Sym::new("path"), Box::new([])),
            rbs_ir::RbsType::Class(Sym::new("IO"), Box::new([])),
        ]));
        assert!(InferenceEngine::rbs_type_is_structural_liberal(&union));

        assert!(!InferenceEngine::rbs_type_is_structural_liberal(
            &rbs_ir::RbsType::String
        ));
        assert!(!InferenceEngine::rbs_type_is_structural_liberal(
            &rbs_ir::RbsType::Class(Sym::new("String"), Box::new([]))
        ));
        assert!(!InferenceEngine::rbs_type_is_structural_liberal(
            &rbs_ir::RbsType::Alias(Sym::new("real"), Box::new([]))
        ));
    }

    fn alias(name: &str) -> rbs_ir::RbsType {
        rbs_ir::RbsType::Alias(Sym::new(name), Box::new([]))
    }

    fn iface(name: &str) -> rbs_ir::RbsType {
        rbs_ir::RbsType::Class(Sym::new(name), Box::new([]))
    }

    #[test]
    fn rbs_param_compat_pathname_against_path_alias_is_yes() {
        let mut e = engine();
        e.registry.add_method_def("Pathname", method_def("to_path"));
        let actual = Type::Class(Sym::new("Pathname"));
        assert_eq!(e.rbs_param_compat(&actual, &alias("path")), ArgCompat::Yes);
    }

    #[test]
    fn rbs_param_compat_string_against_string_alias_is_yes() {
        // (2) Passing a String to a `string` declaration -> immediate Yes via the nominal member.
        let mut e = engine();
        assert_eq!(
            e.rbs_param_compat(&Type::String, &alias("string")),
            ArgCompat::Yes
        );
    }

    #[test]
    fn rbs_param_compat_surface_complete_class_without_to_path_is_no() {
        let mut e = engine();
        e.registry.add_method_def("Widget", method_def("render"));
        let actual = Type::Class(Sym::new("Widget"));
        assert_eq!(e.rbs_param_compat(&actual, &alias("path")), ArgCompat::No);
    }

    #[test]
    fn rbs_param_compat_method_missing_class_is_unknown() {
        // (4) An actual with method_missing has an incomplete surface, so it's Unknown (silent).
        let mut e = engine();
        e.registry
            .add_method_def("Dynamic", method_def("method_missing"));
        let actual = Type::Class(Sym::new("Dynamic"));
        assert_eq!(
            e.rbs_param_compat(&actual, &alias("path")),
            ArgCompat::Unknown
        );
    }

    #[test]
    fn rbs_param_compat_bare_interface_param_matches_alias_behavior() {
        // (5) A declaration with an interface written directly as the param (`def f: (_ToPath) -> void`)
        //     also gets the same structural judgment as a duck alias.
        let mut e = engine();
        e.registry.add_method_def("Pathish", method_def("to_path"));
        let has = Type::Class(Sym::new("Pathish"));
        assert_eq!(e.rbs_param_compat(&has, &iface("_ToPath")), ArgCompat::Yes);

        e.registry.add_method_def("Plain", method_def("render"));
        let lacks = Type::Class(Sym::new("Plain"));
        assert_eq!(e.rbs_param_compat(&lacks, &iface("_ToPath")), ArgCompat::No);
    }

    #[test]
    fn rbs_param_compat_untyped_actual_is_unknown() {
        // actual doesn't reduce to a concrete class (e.g. Untyped) -> unjudgeable, stays silent.
        let mut e = engine();
        assert_eq!(
            e.rbs_param_compat(&Type::Untyped, &alias("path")),
            ArgCompat::Unknown
        );
    }

    #[test]
    fn rbs_param_compat_symbol_against_interned_is_yes() {
        // `interned` = Symbol | String | _ToStr. Symbol is Yes via the nominal member.
        let mut e = engine();
        assert_eq!(
            e.rbs_param_compat(&Type::Symbol, &alias("interned")),
            ArgCompat::Yes
        );
    }
}
