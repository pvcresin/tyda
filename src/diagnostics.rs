use std::collections::BTreeMap;

use serde::Serialize;

use crate::inference::FileAnalysisSnapshot;
use crate::rbs::render::render_rbs_for_file;
use crate::rbs::stdlib_loader::LazyRbsLoader;
use crate::registry::{ClassData, MethodDef, ParamInfo, TypeRegistry};
use crate::types::{MethodSig, ParamKind, Type};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeHoleKind {
    Parameter,
    Return,
    InstanceVariable,
}

impl TypeHoleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TypeHoleKind::Parameter => "parameter",
            TypeHoleKind::Return => "return",
            TypeHoleKind::InstanceVariable => "ivar",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeHoleReason {
    ExplicitUntyped,
    MissingCallSite,
    MissingMethod,
    MissingInstanceVariable,
    DeferredType,
    Unknown,
}

impl TypeHoleReason {
    pub fn as_str(self) -> &'static str {
        match self {
            TypeHoleReason::ExplicitUntyped => "explicit_untyped",
            TypeHoleReason::MissingCallSite => "call_site_missing",
            TypeHoleReason::MissingMethod => "method_unresolved",
            TypeHoleReason::MissingInstanceVariable => "ivar_unresolved",
            TypeHoleReason::DeferredType => "deferred_type",
            TypeHoleReason::Unknown => "unknown",
        }
    }
}

/// Reason a `missing_method` diagnostic was suppressed (used with `--debug` to measure distribution and detect over-silencing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatingReason {
    /// Receiver resolved to bare `Object` (self degraded inside a block DSL).
    ObjectReceiver,
    /// Receiver's superclass / mixin chain has an unresolvable or stub edge.
    IncompleteAncestors,
    /// Bare call inside a module with no static mixin edge: the runtime host is unknown (e.g. `obj.extend(M)`).
    RuntimeMixinHost,
}

impl GatingReason {
    pub fn as_str(self) -> &'static str {
        match self {
            GatingReason::ObjectReceiver => "object_receiver",
            GatingReason::IncompleteAncestors => "incomplete_ancestors",
            GatingReason::RuntimeMixinHost => "runtime_mixin_host",
        }
    }
}

/// Process-global counters for gating suppressions. CLI and LSP share the same
/// diagnostic core, so both feed these; `--debug` reads them after a run.
mod gating_stats {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static OBJECT_RECEIVER: AtomicUsize = AtomicUsize::new(0);
    static INCOMPLETE_ANCESTORS: AtomicUsize = AtomicUsize::new(0);
    static RUNTIME_MIXIN_HOST: AtomicUsize = AtomicUsize::new(0);

    pub fn record(reason: super::GatingReason) {
        match reason {
            super::GatingReason::ObjectReceiver => OBJECT_RECEIVER.fetch_add(1, Ordering::Relaxed),
            super::GatingReason::IncompleteAncestors => {
                INCOMPLETE_ANCESTORS.fetch_add(1, Ordering::Relaxed)
            }
            super::GatingReason::RuntimeMixinHost => {
                RUNTIME_MIXIN_HOST.fetch_add(1, Ordering::Relaxed)
            }
        };
    }

    pub fn snapshot() -> Vec<(&'static str, usize)> {
        vec![
            (
                super::GatingReason::ObjectReceiver.as_str(),
                OBJECT_RECEIVER.load(Ordering::Relaxed),
            ),
            (
                super::GatingReason::IncompleteAncestors.as_str(),
                INCOMPLETE_ANCESTORS.load(Ordering::Relaxed),
            ),
            (
                super::GatingReason::RuntimeMixinHost.as_str(),
                RUNTIME_MIXIN_HOST.load(Ordering::Relaxed),
            ),
        ]
    }
}

/// Record that a `missing_method` diagnostic was suppressed by gating, tagged by
/// reason. Called from the shared diagnostic core.
pub fn record_gating_suppression(reason: GatingReason) {
    gating_stats::record(reason);
}

/// Snapshot of per-reason gating suppression counts for `--debug` output.
pub fn gating_suppression_counts() -> Vec<(&'static str, usize)> {
    gating_stats::snapshot()
}

#[derive(Debug, Clone)]
pub struct TypeHole {
    pub file_path: Option<String>,
    pub class_name: String,
    pub member_name: String,
    pub slot_name: String,
    pub kind: TypeHoleKind,
    pub rendered_type: String,
    pub reason: TypeHoleReason,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct TypeHoleSummary {
    pub holes: Vec<TypeHole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeDiagnostic {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub byte_start: usize,
    pub byte_end: usize,
    pub severity: &'static str,
    pub code: &'static str,
    pub message: String,
    pub method_name: String,
    pub unresolved_method: String,
    /// Argument-type-mismatch detail; absent (and skipped in JSON) for other
    /// diagnostic kinds such as `missing_method`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param_name: Option<String>,
}

impl TypeHoleSummary {
    pub fn total_count(&self) -> usize {
        self.holes.len()
    }

    pub fn untyped_count(&self) -> usize {
        self.holes
            .iter()
            .filter(|hole| hole.rendered_type.contains("untyped"))
            .count()
    }

    pub fn counts_by_kind(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for hole in &self.holes {
            *counts.entry(hole.kind.as_str()).or_insert(0) += 1;
        }
        counts
    }

    pub fn counts_by_reason(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for hole in &self.holes {
            *counts.entry(hole.reason.as_str()).or_insert(0) += 1;
        }
        counts
    }
}

pub fn unresolved_method_diagnostics(
    analysis: &FileAnalysisSnapshot,
    source: &str,
    file_path: &str,
    stdlib_loader: &LazyRbsLoader,
    workspace_registry: Option<&TypeRegistry>,
) -> Vec<TypeDiagnostic> {
    analysis
        .unresolved_method_calls(stdlib_loader, workspace_registry)
        .into_iter()
        .map(|call| {
            let (line, column) = byte_offset_to_line_col(source, call.start);
            let (end_line, end_column) = byte_offset_to_line_col(source, call.end);
            TypeDiagnostic {
                path: file_path.to_string(),
                line,
                column,
                end_line,
                end_column,
                byte_start: call.start,
                byte_end: call.end,
                severity: "warning",
                code: "missing_method",
                message: missing_method_diagnostic_message(
                    &call.method_name,
                    &call.unresolved_method,
                ),
                method_name: call.method_name,
                unresolved_method: call.unresolved_method,
                expected_type: None,
                actual_type: None,
                param_name: None,
            }
        })
        .collect()
}

/// Report arguments whose inferred type is confidently incompatible with the
/// callee's declared parameter type. Parallels `unresolved_method_diagnostics`.
pub fn argument_type_diagnostics(
    analysis: &FileAnalysisSnapshot,
    source: &str,
    file_path: &str,
    stdlib_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&crate::sorbet::rbi::LazyRbiLoader>,
    workspace_registry: Option<&TypeRegistry>,
) -> Vec<TypeDiagnostic> {
    analysis
        .argument_type_mismatches(stdlib_loader, lazy_rbi_loader, workspace_registry)
        .into_iter()
        .map(|mismatch| {
            let (line, column) = byte_offset_to_line_col(source, mismatch.start);
            let (end_line, end_column) = byte_offset_to_line_col(source, mismatch.end);
            let message = argument_type_mismatch_message(
                &mismatch.param_name,
                &mismatch.expected,
                &mismatch.actual,
                mismatch.receiver_type.as_deref(),
            );
            TypeDiagnostic {
                path: file_path.to_string(),
                line,
                column,
                end_line,
                end_column,
                byte_start: mismatch.start,
                byte_end: mismatch.end,
                severity: "error",
                code: "argument_type_mismatch",
                message,
                method_name: mismatch.method_name,
                unresolved_method: String::new(),
                expected_type: Some(mismatch.expected),
                actual_type: Some(mismatch.actual),
                param_name: Some(mismatch.param_name),
            }
        })
        .collect()
}

pub(crate) fn argument_type_mismatch_message(
    param_name: &str,
    expected: &str,
    actual: &str,
    receiver_type: Option<&str>,
) -> String {
    let message = format!("Expected `{expected}` for parameter `{param_name}`, but got `{actual}`");
    receiver_type.map_or(message.clone(), |receiver| {
        format!("{message} for union member `{receiver}`")
    })
}

/// Unresolved+arg-mismatch diagnostics run in a single replay-engine build. Experimental checks are opt-in via `TYDA_EXPERIMENTAL_CHECKS`.
pub fn experimental_checks_enabled() -> bool {
    use std::sync::OnceLock;
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("TYDA_EXPERIMENTAL_CHECKS")
            .ok()
            .is_some_and(|v| !v.is_empty() && v != "0")
    })
}

pub fn method_call_diagnostics(
    analysis: &FileAnalysisSnapshot,
    source: &str,
    file_path: &str,
    stdlib_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&crate::sorbet::rbi::LazyRbiLoader>,
    workspace_registry: Option<&TypeRegistry>,
) -> Vec<TypeDiagnostic> {
    let sites =
        analysis.method_call_diagnostics(stdlib_loader, lazy_rbi_loader, workspace_registry);
    let mut diagnostics =
        method_call_diagnostics_from_sites_without_experimental(sites, source, file_path);
    if experimental_checks_enabled() {
        let experimental = analysis.experimental_check_diagnostics(
            stdlib_loader,
            lazy_rbi_loader,
            workspace_registry,
        );
        diagnostics.extend(experimental_diagnostics(experimental, source, file_path));
    }
    diagnostics
}

pub fn method_call_diagnostics_owned(
    analysis: FileAnalysisSnapshot,
    source: &str,
    file_path: &str,
    stdlib_loader: &LazyRbsLoader,
    lazy_rbi_loader: Option<&crate::sorbet::rbi::LazyRbiLoader>,
    workspace_registry: Option<&TypeRegistry>,
) -> Vec<TypeDiagnostic> {
    let experimental = experimental_checks_enabled().then(|| {
        analysis.experimental_check_diagnostics(stdlib_loader, lazy_rbi_loader, workspace_registry)
    });
    let sites =
        analysis.method_call_diagnostics_into(stdlib_loader, lazy_rbi_loader, workspace_registry);
    let mut diagnostics =
        method_call_diagnostics_from_sites_without_experimental(sites, source, file_path);
    if let Some(experimental) = experimental {
        diagnostics.extend(experimental_diagnostics(experimental, source, file_path));
    }
    diagnostics
}

fn method_call_diagnostics_from_sites_without_experimental(
    sites: crate::inference::MethodCallDiagnosticSites,
    source: &str,
    file_path: &str,
) -> Vec<TypeDiagnostic> {
    let crate::inference::MethodCallDiagnosticSites {
        unresolved_methods,
        argument_type_mismatches,
        unresolved_constants,
        static_sites,
        return_type_mismatches,
    } = sites;
    let mut diagnostics: Vec<TypeDiagnostic> = unresolved_methods
        .into_iter()
        .map(|call| {
            let (line, column) = byte_offset_to_line_col(source, call.start);
            let (end_line, end_column) = byte_offset_to_line_col(source, call.end);
            TypeDiagnostic {
                path: file_path.to_string(),
                line,
                column,
                end_line,
                end_column,
                byte_start: call.start,
                byte_end: call.end,
                severity: "warning",
                code: "missing_method",
                message: missing_method_diagnostic_message(
                    &call.method_name,
                    &call.unresolved_method,
                ),
                method_name: call.method_name,
                unresolved_method: call.unresolved_method,
                expected_type: None,
                actual_type: None,
                param_name: None,
            }
        })
        .collect();
    diagnostics.extend(argument_type_mismatches.into_iter().map(|mismatch| {
        let (line, column) = byte_offset_to_line_col(source, mismatch.start);
        let (end_line, end_column) = byte_offset_to_line_col(source, mismatch.end);
        let message = argument_type_mismatch_message(
            &mismatch.param_name,
            &mismatch.expected,
            &mismatch.actual,
            mismatch.receiver_type.as_deref(),
        );
        TypeDiagnostic {
            path: file_path.to_string(),
            line,
            column,
            end_line,
            end_column,
            byte_start: mismatch.start,
            byte_end: mismatch.end,
            severity: "error",
            code: "argument_type_mismatch",
            message,
            method_name: mismatch.method_name,
            unresolved_method: String::new(),
            expected_type: Some(mismatch.expected),
            actual_type: Some(mismatch.actual),
            param_name: Some(mismatch.param_name),
        }
    }));
    diagnostics.extend(return_type_mismatches.into_iter().map(|mismatch| {
        let (line, column) = byte_offset_to_line_col(source, mismatch.start);
        let (end_line, end_column) = byte_offset_to_line_col(source, mismatch.end);
        TypeDiagnostic {
            path: file_path.to_string(),
            line,
            column,
            end_line,
            end_column,
            byte_start: mismatch.start,
            byte_end: mismatch.end,
            severity: "error",
            code: "return_type_mismatch",
            message: format!(
                "Expected `{}` as the return type of `{}`, but got `{}`",
                mismatch.expected, mismatch.method_name, mismatch.actual
            ),
            method_name: mismatch.method_name,
            unresolved_method: String::new(),
            expected_type: Some(mismatch.expected),
            actual_type: Some(mismatch.actual),
            param_name: None,
        }
    }));
    diagnostics.extend(unresolved_constants.into_iter().map(|constant| {
        let (line, column) = byte_offset_to_line_col(source, constant.start);
        let (end_line, end_column) = byte_offset_to_line_col(source, constant.end);
        TypeDiagnostic {
            path: file_path.to_string(),
            line,
            column,
            end_line,
            end_column,
            byte_start: constant.start,
            byte_end: constant.end,
            severity: "information",
            code: "unresolved_constant",
            message: unresolved_constant_message(&constant.name),
            method_name: String::new(),
            unresolved_method: String::new(),
            expected_type: None,
            actual_type: None,
            param_name: None,
        }
    }));
    diagnostics.extend(static_sites.into_iter().map(|site| {
        let (line, column) = byte_offset_to_line_col(source, site.start);
        let (end_line, end_column) = byte_offset_to_line_col(source, site.end);
        TypeDiagnostic {
            path: file_path.to_string(),
            line,
            column,
            end_line,
            end_column,
            byte_start: site.start,
            byte_end: site.end,
            severity: site.severity,
            code: site.code,
            message: site.message,
            method_name: site.method_name,
            unresolved_method: String::new(),
            expected_type: None,
            actual_type: None,
            param_name: None,
        }
    }));
    diagnostics
}

fn experimental_diagnostics(
    experimental: Vec<crate::inference::ExperimentalDiagnostic>,
    source: &str,
    file_path: &str,
) -> Vec<TypeDiagnostic> {
    experimental
        .into_iter()
        .map(|d| {
            let (line, column) = byte_offset_to_line_col(source, d.start);
            let (end_line, end_column) = byte_offset_to_line_col(source, d.end);
            TypeDiagnostic {
                path: file_path.to_string(),
                line,
                column,
                end_line,
                end_column,
                byte_start: d.start,
                byte_end: d.end,
                severity: d.severity,
                code: d.code,
                message: format!("[experimental] {}", d.message),
                method_name: d.method_name,
                unresolved_method: String::new(),
                expected_type: None,
                actual_type: None,
                param_name: None,
            }
        })
        .collect()
}

pub(crate) fn unresolved_constant_message(name: &str) -> String {
    format!("Constant `{name}` is not defined")
}

fn missing_method_diagnostic_message(method_name: &str, unresolved_method: &str) -> String {
    if let Some((owner, method)) = unresolved_method.rsplit_once('#') {
        return format!("Method `{method}` not found for `{owner}`");
    }
    format!("Method `{method_name}` not found")
}

fn byte_offset_to_line_col(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut column = 0u32;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            return (line, column);
        }
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    (line, column)
}

pub fn summarize_type_holes(registry: &TypeRegistry) -> TypeHoleSummary {
    let mut holes = Vec::new();
    for class_name in registry.user_defined_class_names() {
        let Some(data) = registry.class_data_for(&class_name) else {
            continue;
        };
        collect_ivar_holes(&class_name, data, &mut holes);
        collect_method_holes(registry, &class_name, data, &mut holes);
    }
    holes.sort_by(|a, b| {
        a.file_path
            .cmp(&b.file_path)
            .then(a.class_name.cmp(&b.class_name))
            .then(a.member_name.cmp(&b.member_name))
            .then(a.slot_name.cmp(&b.slot_name))
    });
    TypeHoleSummary { holes }
}

pub fn explain_type_hole_reason(ty: &Type) -> Option<&'static str> {
    if !type_contains_hole(ty) {
        return None;
    }
    Some(reason_from_type(ty, false).as_str())
}

pub fn has_type_hole(ty: &Type) -> bool {
    type_contains_hole(ty)
}

pub fn has_todo_marker(ty: &Type) -> bool {
    match ty {
        Type::Todo => true,
        Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => {
            parts.iter().any(has_todo_marker)
        }
        Type::Array(Some(inner)) => has_todo_marker(inner),
        Type::Hash(Some(key), Some(value)) => has_todo_marker(key) || has_todo_marker(value),
        Type::Hash(Some(key), None) => has_todo_marker(key),
        Type::Hash(None, Some(value)) => has_todo_marker(value),
        Type::Record(fields) => fields.iter().any(|field| has_todo_marker(&field.value)),
        Type::Proc { return_type, .. } => has_todo_marker(return_type),
        _ => false,
    }
}

fn user_facing_type_string(ty: &Type) -> String {
    user_facing_type(ty).to_string()
}

fn user_facing_type(ty: &Type) -> Type {
    match ty {
        Type::Todo => Type::Untyped,
        Type::Union(parts) => {
            Type::from_type_vec_preserve_untyped(parts.iter().map(user_facing_type).collect())
        }
        Type::Intersection(parts) => {
            Type::Intersection(parts.iter().map(user_facing_type).collect())
        }
        Type::Array(Some(inner)) => Type::Array(Some(Box::new(user_facing_type(inner)))),
        Type::Hash(Some(key), Some(value)) => Type::Hash(
            Some(Box::new(user_facing_type(key))),
            Some(Box::new(user_facing_type(value))),
        ),
        Type::Hash(Some(key), None) => Type::Hash(Some(Box::new(user_facing_type(key))), None),
        Type::Hash(None, Some(value)) => Type::Hash(None, Some(Box::new(user_facing_type(value)))),
        Type::Tuple(parts) => Type::Tuple(parts.iter().map(user_facing_type).collect()),
        Type::Record(fields) => Type::Record(
            fields
                .iter()
                .map(|field| crate::types::RecordField {
                    key: field.key.clone(),
                    value: user_facing_type(&field.value),
                    optional: field.optional,
                })
                .collect(),
        ),
        Type::Proc {
            return_type,
            param_count,
            signature,
        } => Type::Proc {
            return_type: Box::new(user_facing_type(return_type)),
            param_count: *param_count,
            signature: signature.clone(),
        },
        _ => ty.clone(),
    }
}

fn collect_ivar_holes(class_name: &str, data: &ClassData, holes: &mut Vec<TypeHole>) {
    for (ivar_name, types) in &data.ivars {
        let ty = Type::from_type_vec_preserve_untyped(types.clone());
        if !type_contains_hole(&ty) {
            continue;
        }
        holes.push(TypeHole {
            file_path: data.file_path.as_deref().map(str::to_string),
            class_name: class_name.to_string(),
            member_name: ivar_name.to_string(),
            slot_name: ivar_name.to_string(),
            kind: TypeHoleKind::InstanceVariable,
            rendered_type: user_facing_type_string(&ty),
            reason: reason_from_type(&ty, false),
            line: data.loc.map(|loc| loc.line),
        });
    }
}

fn collect_method_holes(
    registry: &TypeRegistry,
    class_name: &str,
    data: &ClassData,
    holes: &mut Vec<TypeHole>,
) {
    for method in &data.methods {
        if method.rbs_file_source {
            continue;
        }
        let Some(sig) = registry.lookup_method_sig(class_name, &method.name) else {
            continue;
        };
        collect_param_holes(class_name, data, method, &sig, holes);
        collect_return_hole(class_name, data, method, &sig, holes);
    }
}

fn collect_param_holes(
    class_name: &str,
    data: &ClassData,
    method: &MethodDef,
    sig: &MethodSig,
    holes: &mut Vec<TypeHole>,
) {
    for (idx, param) in sig.params.iter().enumerate() {
        if !type_contains_hole(&param.param_type) {
            continue;
        }
        let info = method.param_infos.get(idx);
        holes.push(TypeHole {
            file_path: data.file_path.as_deref().map(str::to_string),
            class_name: class_name.to_string(),
            member_name: method.name.to_string(),
            slot_name: param.name.clone(),
            kind: TypeHoleKind::Parameter,
            rendered_type: user_facing_type_string(&param.param_type),
            reason: reason_for_param(method, info, &param.param_type),
            line: method.loc.map(|loc| loc.line),
        });
    }
}

fn collect_return_hole(
    class_name: &str,
    data: &ClassData,
    method: &MethodDef,
    sig: &MethodSig,
    holes: &mut Vec<TypeHole>,
) {
    if !type_contains_hole(&sig.return_type) {
        return;
    }
    holes.push(TypeHole {
        file_path: data.file_path.as_deref().map(str::to_string),
        class_name: class_name.to_string(),
        member_name: method.name.to_string(),
        slot_name: "return".to_string(),
        kind: TypeHoleKind::Return,
        rendered_type: user_facing_type_string(&sig.return_type),
        reason: reason_from_type(&method.raw_return_type, method.has_annotation()),
        line: method.loc.map(|loc| loc.line),
    });
}

fn reason_for_param(method: &MethodDef, info: Option<&ParamInfo>, ty: &Type) -> TypeHoleReason {
    if method.has_annotation() && matches!(ty, Type::Untyped) {
        return TypeHoleReason::ExplicitUntyped;
    }
    if let Some(info) = info {
        match info.kind {
            ParamKind::Required | ParamKind::Optional | ParamKind::Rest => {
                return TypeHoleReason::MissingCallSite;
            }
            ParamKind::KeywordRequired | ParamKind::KeywordOptional | ParamKind::DoubleRest => {
                return TypeHoleReason::MissingCallSite;
            }
            ParamKind::Block => {}
        }
    }
    reason_from_type(ty, method.has_annotation())
}

fn reason_from_type(ty: &Type, annotated: bool) -> TypeHoleReason {
    if annotated && matches!(ty, Type::Untyped) {
        return TypeHoleReason::ExplicitUntyped;
    }
    match ty {
        Type::Untyped | Type::Todo => TypeHoleReason::Unknown,
        Type::ParamRef(_) | Type::KeywordParamRef(_) => TypeHoleReason::MissingCallSite,
        Type::MethodReturnRef(_, _) | Type::ReceiverMethodRef(_, _) => {
            TypeHoleReason::MissingMethod
        }
        Type::IvarRef(_) => TypeHoleReason::MissingInstanceVariable,
        Type::Union(parts) | Type::Intersection(parts) => parts
            .iter()
            .find_map(|part| {
                let reason = reason_from_type(part, annotated);
                (reason != TypeHoleReason::Unknown).then_some(reason)
            })
            .unwrap_or(TypeHoleReason::DeferredType),
        Type::Array(Some(inner)) => reason_from_type(inner, annotated),
        Type::Hash(Some(key), Some(value)) => {
            let key_reason = reason_from_type(key, annotated);
            if key_reason != TypeHoleReason::Unknown {
                key_reason
            } else {
                reason_from_type(value, annotated)
            }
        }
        Type::Hash(Some(key), None) => reason_from_type(key, annotated),
        Type::Hash(None, Some(value)) => reason_from_type(value, annotated),
        Type::Tuple(parts) => parts
            .iter()
            .find_map(|part| {
                let reason = reason_from_type(part, annotated);
                (reason != TypeHoleReason::Unknown).then_some(reason)
            })
            .unwrap_or(TypeHoleReason::DeferredType),
        Type::Record(fields) => fields
            .iter()
            .find_map(|field| {
                let reason = reason_from_type(&field.value, annotated);
                (reason != TypeHoleReason::Unknown).then_some(reason)
            })
            .unwrap_or(TypeHoleReason::DeferredType),
        Type::Proc { return_type, .. } => reason_from_type(return_type, annotated),
        _ => TypeHoleReason::DeferredType,
    }
}

fn type_contains_hole(ty: &Type) -> bool {
    match ty {
        Type::Untyped | Type::Todo => true,
        Type::ParamRef(_)
        | Type::KeywordParamRef(_)
        | Type::IvarRef(_)
        | Type::MethodReturnRef(_, _)
        | Type::ReceiverMethodRef(_, _)
        | Type::DeferredSafeNavigation(_, _) => true,
        Type::Union(parts) | Type::Intersection(parts) | Type::Tuple(parts) => {
            parts.iter().any(type_contains_hole)
        }
        Type::Array(Some(inner)) => type_contains_hole(inner),
        Type::Hash(Some(key), Some(value)) => type_contains_hole(key) || type_contains_hole(value),
        Type::Hash(Some(key), None) => type_contains_hole(key),
        Type::Hash(None, Some(value)) => type_contains_hole(value),
        Type::Record(fields) => fields.iter().any(|field| type_contains_hole(&field.value)),
        Type::Proc { return_type, .. } => type_contains_hole(return_type),
        _ => false,
    }
}

pub fn build_scenario_seed(
    file_path: &str,
    source: &str,
    registry: &TypeRegistry,
    summary: &TypeHoleSummary,
) -> Option<String> {
    let hole = summary.holes.iter().find(|hole| {
        hole.file_path
            .as_deref()
            .is_some_and(|path| path == file_path)
    })?;
    let snippet = extract_reduced_snippet(source, hole.line.unwrap_or(1) as usize);
    let expected_rbs = render_rbs_for_file(registry, file_path);
    Some(format!(
        "# Auto Reduced Scenario Seed\n\n## {}:{} {}\n\n### update\n\n`{}`\n\n```ruby\n{}```\n\n### result\n\n```rbs\n{}```\n",
        hole.class_name,
        hole.line.unwrap_or(1),
        hole.reason.as_str(),
        file_path,
        snippet,
        expected_rbs
    ))
}

fn extract_reduced_snippet(source: &str, target_line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let target_idx = target_line
        .saturating_sub(1)
        .min(lines.len().saturating_sub(1));
    let start_idx = (0..=target_idx)
        .rev()
        .find(|idx| {
            let trimmed = lines[*idx].trim_start();
            trimmed.starts_with("class ")
                || trimmed.starts_with("module ")
                || trimmed.starts_with("def ")
        })
        .unwrap_or(target_idx.saturating_sub(3));
    let mut depth = 0isize;
    let mut end_idx = (target_idx + 3).min(lines.len().saturating_sub(1));
    for (idx, line) in lines.iter().enumerate().skip(start_idx) {
        let trimmed = line.trim();
        if starts_block(trimmed) {
            depth += 1;
        }
        if trimmed == "end" {
            depth -= 1;
            if depth <= 0 {
                end_idx = idx;
                break;
            }
        }
    }
    let mut snippet = lines[start_idx..=end_idx].join("\n");
    if !snippet.ends_with('\n') {
        snippet.push('\n');
    }
    snippet
}

fn starts_block(trimmed: &str) -> bool {
    trimmed.starts_with("class ")
        || trimmed.starts_with("module ")
        || trimmed.starts_with("def ")
        || trimmed.starts_with("if ")
        || trimmed.starts_with("unless ")
        || trimmed.starts_with("case ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("until ")
        || trimmed.starts_with("for ")
        || trimmed.ends_with(" do")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::AnalysisOptions;
    use crate::registry::{MethodDef, ParamInfo};
    use crate::types::{ParamKind, SourceLocation, Sym};

    #[test]
    fn summarize_reports_missing_callsites_and_returns() {
        let mut registry = TypeRegistry::new();
        registry.mark_user_defined("Probe");
        registry.set_file_path("Probe", "probe.rb");
        registry.set_class_location("Probe", SourceLocation { line: 1, column: 0 });
        registry.add_method_def(
            "Probe",
            MethodDef {
                name: Sym::new("missing"),
                param_infos: vec![ParamInfo {
                    name: "value".to_string(),
                    kind: ParamKind::Required,
                    default_type: None,
                }],
                raw_return_type: Type::ReceiverMethodRef(
                    Box::new(Type::ParamRef(0)),
                    crate::types::Sym::new("unknown"),
                ),
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
                loc: Some(SourceLocation { line: 2, column: 2 }),
            },
        );

        let summary = summarize_type_holes(&registry);
        assert_eq!(summary.total_count(), 2);
        assert_eq!(
            summary.counts_by_reason().get("call_site_missing").copied(),
            Some(1)
        );
        assert_eq!(
            summary.counts_by_reason().get("method_unresolved").copied(),
            Some(1)
        );
    }

    fn union_member_missing_methods(source: &str) -> Vec<String> {
        union_member_missing_methods_with_options(source, AnalysisOptions::default())
    }

    fn union_member_missing_methods_with_options(
        source: &str,
        options: AnalysisOptions,
    ) -> Vec<String> {
        use crate::analysis::analyze_cached_file_with_deps;
        let core_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let (analysis, _) = analyze_cached_file_with_deps(
            source,
            None,
            Some(&loader),
            Some("union_member.rb"),
            options,
        );
        analysis
            .method_call_diagnostics(&loader, None, None)
            .unresolved_methods
            .into_iter()
            .map(|call| call.unresolved_method)
            .collect()
    }

    #[test]
    fn active_support_try_is_available_on_any_object() {
        let source = concat!(
            "class MergeRequest\n",
            "  def resource_parent = Object.new\n",
            "end\n",
            "class WorkItem\n",
            "  def resource_parent = Object.new\n",
            "end\n",
            "\n",
            "#: ((MergeRequest | WorkItem)?) -> void\n",
            "def process(issuable)\n",
            "  issuable.try(:resource_parent)\n",
            "  issuable.try!(:resource_parent)\n",
            "end\n",
        );
        let options = AnalysisOptions {
            rails_mode: true,
            ..AnalysisOptions::default()
        };

        let methods = union_member_missing_methods_with_options(source, options);
        assert!(
            methods.is_empty(),
            "Rails Object#try is available: {methods:?}"
        );
    }

    #[test]
    fn rails_method_table_fallbacks_are_not_reported_missing() {
        let source = concat!(
            "class Probe\n",
            "  def duration = 2.days\n",
            "  def minutes = 5.minutes\n",
            "  def humanized = 'rails'.humanize\n",
            "  def html_safe = 'rails'.html_safe\n",
            "end\n",
        );
        let options = AnalysisOptions {
            rails_mode: true,
            ..AnalysisOptions::default()
        };

        let methods = union_member_missing_methods_with_options(source, options);
        assert!(
            methods.is_empty(),
            "known Rails methods are available: {methods:?}"
        );
    }

    #[test]
    fn optional_default_parameter_is_narrowed_after_early_return() {
        let source = concat!(
            "class AvatarUser\n",
            "  def avatar_url = 'avatar'\n",
            "  def blocked? = false\n",
            "  def confirmed? = true\n",
            "end\n",
            "\n",
            "module AvatarsHelper\n",
            "  def avatar_for(user = nil, size = nil, scale = 2, only_path: true, current_user: nil)\n",
            "    return 'default' unless user\n",
            "    return 'default' if blocked_or_unconfirmed?(user) && !can_admin?(current_user)\n",
            "    image_size = !size.nil? ? size * 2 : size\n",
            "    user.avatar_url(size: image_size, only_path: only_path) || 'default'\n",
            "  end\n",
            "\n",
            "  def blocked_or_unconfirmed?(user)\n",
            "    user.blocked? || !user.confirmed?\n",
            "  end\n",
            "\n",
            "  def can_admin?(user) = false\n",
            "end\n",
            "\n",
            "def avatar_for_known_user = AvatarsHelper.new.avatar_for(AvatarUser.new)\n",
            "def avatar_for_nil_user = AvatarsHelper.new.avatar_for(nil)\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "the early return leaves only non-nil receivers: {methods:?}"
        );
    }

    #[test]
    fn non_rails_try_does_not_hide_nil_receiver_diagnostic() {
        let source = concat!(
            "class Resource\n",
            "  def try = true\n",
            "end\n",
            "\n",
            "#: ((Resource | nil)) -> void\n",
            "def process(resource)\n",
            "  resource.try\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.contains(&"NilClass#try".to_string()),
            "non-Rails try must not be treated as nil-safe: {methods:?}"
        );
    }

    #[test]
    fn union_member_missing_method_reports_nil_member() {
        // With a `Corporation | nil` receiver, the nil member lacks `name` -> reported.
        let source = concat!(
            "class Corporation\n",
            "  #: () -> String\n",
            "  def name\n",
            "    \"x\"\n",
            "  end\n",
            "end\n",
            "\n",
            "#: (Corporation?) -> void\n",
            "def process(corp)\n",
            "  corp.name\n",
            "end\n",
        );
        let messages = union_member_missing_methods(source);
        assert_eq!(messages.len(), 1, "expected one diagnostic: {messages:?}");
        assert!(
            messages[0].contains("NilClass#name"),
            "missing nil member: {messages:?}"
        );
    }

    #[test]
    fn union_member_missing_method_silent_when_all_members_have_it() {
        // Both `Integer` and `Float` have `abs` -> no report.
        let source = concat!(
            "#: (Integer | Float) -> void\n",
            "def process(n)\n",
            "  n.abs\n",
            "end\n",
        );
        assert!(
            union_member_missing_methods(source).is_empty(),
            "no member lacks the method"
        );
    }

    #[test]
    fn standard_class_case_equality_guards_a_union_receiver() {
        let source = concat!(
            "class CaseTarget\n",
            "  def value = 'value'\n",
            "end\n",
            "class OtherTarget; end\n",
            "\n",
            "#: (CaseTarget | OtherTarget) -> void\n",
            "def process(value)\n",
            "  return nil unless CaseTarget === value\n",
            "  value.value\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "the class equality guard excludes OtherTarget: {methods:?}"
        );
    }

    #[test]
    fn standard_class_case_equality_closes_a_disjoint_receiver_branch() {
        let source = concat!(
            "class CaseTarget; end\n",
            "\n",
            "#: (String) -> void\n",
            "def process(value)\n",
            "  return nil unless CaseTarget === value\n",
            "  value.missing\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "the class equality guard excludes String: {methods:?}"
        );
    }

    #[test]
    fn custom_class_case_equality_does_not_close_a_receiver_branch() {
        let source = concat!(
            "class CaseTarget\n",
            "  def self.===(_value) = true\n",
            "end\n",
            "\n",
            "#: (String) -> void\n",
            "def process(value)\n",
            "  return nil unless CaseTarget === value\n",
            "  value.missing\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert_eq!(
            methods.len(),
            1,
            "a custom === can admit String values: {methods:?}"
        );
        assert!(
            methods[0].contains("String#missing"),
            "expected missing String method: {methods:?}"
        );
    }

    #[test]
    fn class_case_equality_narrows_parent_to_a_known_subclass() {
        let source = concat!(
            "class BaseTarget; end\n",
            "class CaseTarget < BaseTarget; end\n",
            "\n",
            "#: (BaseTarget) -> void\n",
            "def process(value)\n",
            "  return nil unless CaseTarget === value\n",
            "  value.missing\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert_eq!(
            methods.len(),
            1,
            "BaseTarget can contain a CaseTarget instance: {methods:?}"
        );
        assert!(
            methods[0].contains("CaseTarget#missing"),
            "expected the branch to narrow to CaseTarget: {methods:?}"
        );
    }

    #[test]
    fn standard_type_predicates_close_a_disjoint_receiver_branch() {
        let source = concat!(
            "class CaseTarget; end\n",
            "\n",
            "#: (String) -> void\n",
            "def check_is_a(value)\n",
            "  return nil unless value.is_a?(CaseTarget)\n",
            "  value.missing\n",
            "end\n",
            "\n",
            "#: (String) -> void\n",
            "def check_instance_of(value)\n",
            "  return nil unless value.instance_of?(CaseTarget)\n",
            "  value.missing\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "standard type predicates exclude String: {methods:?}"
        );
    }

    #[test]
    fn standard_type_predicates_narrow_parent_to_a_known_subclass() {
        let source = concat!(
            "class PredicateBase; end\n",
            "class PredicateChild < PredicateBase\n",
            "  def child_only; end\n",
            "end\n",
            "\n",
            "#: (String | PredicateBase) -> void\n",
            "def check(value)\n",
            "  return nil unless value.is_a?(PredicateChild)\n",
            "  value.child_only\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "is_a? should narrow the parent arm to PredicateChild: {methods:?}"
        );
    }

    #[test]
    fn standard_type_predicates_narrow_parent_instance_variables_to_a_known_subclass() {
        let source = concat!(
            "class IvarPredicateBase; end\n",
            "class IvarPredicateChild < IvarPredicateBase\n",
            "  def child_only; end\n",
            "end\n",
            "\n",
            "class IvarPredicateChecker\n",
            "  #: (IvarPredicateBase) -> void\n",
            "  def check(value)\n",
            "    @node = value\n",
            "    return nil unless @node.is_a?(IvarPredicateChild)\n",
            "    @node.child_only\n",
            "  end\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "is_a? should narrow the parent instance variable to IvarPredicateChild: {methods:?}"
        );
    }

    #[test]
    fn unknown_superclass_keeps_a_possible_is_a_branch_open() {
        let source = concat!(
            "class CaseTarget; end\n",
            "parent_class = CaseTarget\n",
            "UnknownChild = Class.new(parent_class) do\n",
            "end\n",
            "\n",
            "#: (UnknownChild) -> void\n",
            "def process(value)\n",
            "  return nil unless value.is_a?(CaseTarget)\n",
            "  value.missing\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods
                .iter()
                .any(|method| method.contains("UnknownChild#missing")),
            "an unknown superclass may include CaseTarget: {methods:?}"
        );
    }

    #[test]
    fn unknown_target_superclass_keeps_a_possible_instance_of_branch_open() {
        let source = concat!(
            "class BaseTarget; end\n",
            "parent_class = BaseTarget\n",
            "UnknownTarget = Class.new(parent_class) do\n",
            "end\n",
            "\n",
            "#: (BaseTarget) -> void\n",
            "def process(value)\n",
            "  return nil unless value.instance_of?(UnknownTarget)\n",
            "  value.missing\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods
                .iter()
                .any(|method| method.contains("UnknownTarget#missing")),
            "UnknownTarget may inherit BaseTarget and be the exact runtime class: {methods:?}"
        );
    }

    #[test]
    fn refinement_of_is_a_keeps_a_possible_branch_open() {
        let source = concat!(
            "class CaseTarget; end\n",
            "module AlwaysMatch\n",
            "  refine String do\n",
            "    def is_a?(_target) = true\n",
            "  end\n",
            "end\n",
            "using AlwaysMatch\n",
            "\n",
            "#: (String) -> void\n",
            "def process(value)\n",
            "  return nil unless value.is_a?(CaseTarget)\n",
            "  value.missing\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods
                .iter()
                .any(|method| method.contains("String#missing")),
            "an active refinement can make String#is_a? return true: {methods:?}"
        );
    }

    #[test]
    fn refinement_of_class_case_equality_keeps_a_possible_branch_open() {
        let source = concat!(
            "class CaseTarget; end\n",
            "module AlwaysMatch\n",
            "  refine Class do\n",
            "    def ===(_value) = true\n",
            "  end\n",
            "end\n",
            "using AlwaysMatch\n",
            "\n",
            "#: (String) -> void\n",
            "def process(value)\n",
            "  return nil unless CaseTarget === value\n",
            "  value.missing\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods
                .iter()
                .any(|method| method.contains("String#missing")),
            "an active refinement can make Class#=== return true: {methods:?}"
        );
    }

    #[test]
    fn hash_predicate_override_keeps_a_shape_mismatch_branch_open() {
        let source = concat!(
            "class Hash\n",
            "  #: (Symbol) -> bool\n",
            "  def key?(_key) = true\n",
            "end\n",
            "\n",
            "def check_hash_key\n",
            "  values = { present: \"value\" }\n",
            "  values[:missing].upcase if values.key?(:missing)\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods
                .iter()
                .any(|method| method.contains("NilClass#upcase")),
            "the source-defined predicate may admit an absent key: {methods:?}"
        );
    }

    #[test]
    fn instance_of_superclass_excludes_a_subclass_receiver_branch() {
        let source = concat!(
            "class BaseTarget; end\n",
            "class ChildTarget < BaseTarget; end\n",
            "\n",
            "#: (ChildTarget) -> void\n",
            "def process(value)\n",
            "  return nil unless value.instance_of?(BaseTarget)\n",
            "  value.missing\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "an instance of ChildTarget cannot be an exact BaseTarget: {methods:?}"
        );
    }

    #[test]
    fn source_defined_is_a_does_not_narrow_a_receiver() {
        let source = concat!(
            "class CaseTarget; end\n",
            "class String\n",
            "  def is_a?(_target) = true\n",
            "end\n",
            "\n",
            "#: (String) -> void\n",
            "def process(value)\n",
            "  return nil unless value.is_a?(CaseTarget)\n",
            "  value.missing\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert_eq!(
            methods.len(),
            1,
            "a source-defined is_a? can admit String values: {methods:?}"
        );
        assert!(
            methods[0].contains("String#missing"),
            "expected missing String method: {methods:?}"
        );
    }

    #[test]
    fn active_record_model_metadata_and_readonly_are_known_methods() {
        let source = concat!(
            "class ActiveRecord::Base; end\n",
            "class Model < ActiveRecord::Base; end\n",
            "\n",
            "def model_reflections = Model.reflections\n",
            "def model_base_class = Model.base_class\n",
            "def readonly_model = Model.new.readonly!\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "Active Record model methods should be available: {methods:?}"
        );
    }

    #[test]
    fn break_if_equality_guard_excludes_the_sentinel_after_break() {
        let source = concat!(
            "class DownloadRequest\n",
            "  def spec = :spec\n",
            "end\n",
            "\n",
            "#: (DownloadRequest | :stop) -> void\n",
            "def download(req)\n",
            "  while req\n",
            "    break if req == :stop\n",
            "    req.spec\n",
            "  end\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "the :stop request exits before the method call: {methods:?}"
        );
    }

    #[test]
    fn queue_symbol_sentinel_is_excluded_after_break() {
        let source = concat!(
            "class DownloadRequest\n",
            "  def spec = :spec\n",
            "  def installed? = true\n",
            "end\n",
            "\n",
            "#: (Array[DownloadRequest]) -> void\n",
            "def download(sorted_requests)\n",
            "  download_queue = Thread::Queue.new\n",
            "  sorted_requests.each do |req|\n",
            "    download_queue << req\n",
            "  end\n",
            "  download_queue << :stop\n",
            "  while req = download_queue.pop\n",
            "    break if req == :stop\n",
            "    req.spec\n",
            "    req.installed?\n",
            "  end\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "the queued :stop sentinel exits before request methods: {methods:?}"
        );
    }

    #[test]
    fn queue_sentinel_does_not_discard_unknown_elements() {
        let source = concat!(
            "class DownloadRequest\n",
            "  def spec = :spec\n",
            "  def installed? = true\n",
            "end\n",
            "\n",
            "#: (Array[untyped]) -> void\n",
            "def download(sorted_requests)\n",
            "  download_queue = Thread::Queue.new\n",
            "  sorted_requests.each do |req|\n",
            "    download_queue << req\n",
            "  end\n",
            "  download_queue << :stop\n",
            "  while req = download_queue.pop\n",
            "    break if req == :stop\n",
            "    req.spec\n",
            "    req.installed?\n",
            "  end\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "unknown queue elements must not collapse to the :stop symbol: {methods:?}"
        );
    }

    #[test]
    fn known_method_with_untyped_return_is_not_reported_missing() {
        let source = concat!(
            "class Node\n",
            "  #: () -> untyped\n",
            "  def lenv; end\n",
            "end\n",
            "\n",
            "class Owner\n",
            "  #: (Node node) -> void\n",
            "  def initialize(node)\n",
            "    @node = node\n",
            "  end\n",
            "\n",
            "  #: (Node node) -> untyped\n",
            "  def read\n",
            "    @node.lenv\n",
            "  end\n",
            "\n",
            "  #: (Node node) -> untyped\n",
            "  def missing\n",
            "    @node.unknown\n",
            "  end\n",
            "end\n",
            "\n",
            "class MutexOwner\n",
            "  #: (Thread::Mutex mutex) -> void\n",
            "  def initialize(mutex)\n",
            "    @mutex = mutex\n",
            "  end\n",
            "\n",
            "  def synchronize\n",
            "    @mutex.synchronize { :ok }\n",
            "  end\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert_eq!(methods, ["Node#unknown"]);
    }

    #[test]
    fn union_member_missing_method_reports_all_members_that_lack_it() {
        let source = concat!(
            "class Alpha\n",
            "end\n",
            "class Beta\n",
            "end\n",
            "\n",
            "#: (Alpha | Beta) -> void\n",
            "def process(x)\n",
            "  x.bogus\n",
            "end\n",
        );
        let methods = union_member_missing_methods(source);
        assert_eq!(
            methods.len(),
            2,
            "both members lack the method: {methods:?}"
        );
        assert!(methods[0].contains("Alpha#bogus"));
        assert!(methods[1].contains("Beta#bogus"));
    }

    #[test]
    fn union_member_missing_method_silent_with_untyped_member() {
        // If an untyped member is mixed in, the receiver surface is partly unknown -> stay silent.
        let source = concat!(
            "class Corporation\n",
            "  #: () -> String\n",
            "  def name\n",
            "    \"x\"\n",
            "  end\n",
            "end\n",
            "\n",
            "#: (Corporation | untyped) -> void\n",
            "def process(corp)\n",
            "  corp.bogus\n",
            "end\n",
        );
        assert!(
            union_member_missing_methods(source).is_empty(),
            "untyped member suppresses the union diagnostic"
        );
    }

    #[test]
    fn unknown_count_receiver_does_not_become_an_integer() {
        let source = concat!(
            "class UnknownBuilder\n",
            "  #: () -> untyped\n",
            "  def self.make; end\n",
            "end\n",
            "\n",
            "def build_query\n",
            "  UnknownBuilder.make.count.as(:total)\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "unknown count result should not create a missing method on Integer: {methods:?}"
        );
    }

    #[test]
    fn unless_known_falsy_return_makes_following_calls_unreachable() {
        let source = concat!(
            "def render_404 = nil\n",
            "\n",
            "def guarded_nil\n",
            "  value = nil\n",
            "  return render_404 unless value\n",
            "  value.filename\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "a known-falsy unless guard exits before the following call: {methods:?}"
        );
    }

    #[test]
    fn self_method_truthiness_guard_narrows_a_repeated_call() {
        let source = concat!(
            "class Project\n",
            "  #: () -> String\n",
            "  def name\n",
            "    \"project\"\n",
            "  end\n",
            "end\n",
            "\n",
            "class Finder\n",
            "  #: () -> Project?\n",
            "  def project\n",
            "    @project\n",
            "  end\n",
            "\n",
            "  def project_name\n",
            "    if project\n",
            "      project.name\n",
            "    else\n",
            "      \"none\"\n",
            "    end\n",
            "  end\n",
            "end\n",
        );

        let methods = union_member_missing_methods(source);
        assert!(
            methods.is_empty(),
            "the truthy guard applies to the repeated reader call: {methods:?}"
        );
    }

    #[test]
    fn scenario_seed_extracts_enclosing_block() {
        let source = concat!(
            "class Probe\n",
            "  def sample(value)\n",
            "    value.unknown_call\n",
            "  end\n",
            "end\n"
        );
        let snippet = extract_reduced_snippet(source, 3);
        assert!(snippet.contains("def sample(value)"));
        assert!(snippet.contains("value.unknown_call"));
    }
}
