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
) -> String {
    format!("Expected `{expected}` for parameter `{param_name}`, but got `{actual}`")
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
    let (unresolved, mismatches, unresolved_constants) =
        analysis.method_call_diagnostics(stdlib_loader, lazy_rbi_loader, workspace_registry);
    let mut diagnostics = method_call_diagnostics_from_sites_without_experimental(
        unresolved,
        mismatches,
        unresolved_constants,
        source,
        file_path,
    );
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
    let (unresolved, mismatches, unresolved_constants) =
        analysis.method_call_diagnostics_into(stdlib_loader, lazy_rbi_loader, workspace_registry);
    let mut diagnostics = method_call_diagnostics_from_sites_without_experimental(
        unresolved,
        mismatches,
        unresolved_constants,
        source,
        file_path,
    );
    if let Some(experimental) = experimental {
        diagnostics.extend(experimental_diagnostics(experimental, source, file_path));
    }
    diagnostics
}

fn method_call_diagnostics_from_sites_without_experimental(
    unresolved: Vec<crate::inference::UnresolvedMethodCall>,
    mismatches: Vec<crate::inference::ArgumentTypeMismatch>,
    unresolved_constants: Vec<crate::inference::UnresolvedConstant>,
    source: &str,
    file_path: &str,
) -> Vec<TypeDiagnostic> {
    let mut diagnostics: Vec<TypeDiagnostic> = unresolved
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
    diagnostics.extend(mismatches.into_iter().map(|mismatch| {
        let (line, column) = byte_offset_to_line_col(source, mismatch.start);
        let (end_line, end_column) = byte_offset_to_line_col(source, mismatch.end);
        let message = argument_type_mismatch_message(
            &mismatch.param_name,
            &mismatch.expected,
            &mismatch.actual,
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
        } => Type::Proc {
            return_type: Box::new(user_facing_type(return_type)),
            param_count: *param_count,
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
        | Type::ReceiverMethodRef(_, _) => true,
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

    fn union_member_diag_codes(source: &str) -> Vec<String> {
        use crate::analysis::{AnalysisOptions, analyze_cached_file_with_deps};
        let core_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
        let loader = LazyRbsLoader::new(core_dir);
        let (analysis, _) = analyze_cached_file_with_deps(
            source,
            None,
            Some(&loader),
            Some("union_member.rb"),
            AnalysisOptions::default(),
        );
        // Computes the diagnostic directly, bypassing the env gate (equivalent to the flag being ON).
        analysis
            .experimental_check_diagnostics(&loader, None, None)
            .into_iter()
            .filter(|d| d.code == "union_member_missing_method")
            .map(|d| d.message)
            .collect()
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
        let messages = union_member_diag_codes(source);
        assert_eq!(messages.len(), 1, "expected one diagnostic: {messages:?}");
        assert!(
            messages[0].contains("Method `name` not found for union member `nil`")
                && messages[0].contains("receiver `Corporation | nil`"),
            "message shape: {messages:?}"
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
            union_member_diag_codes(source).is_empty(),
            "no member lacks the method"
        );
    }

    #[test]
    fn union_member_missing_method_silent_when_all_members_lack_it() {
        // The case where no member has `bogus` is a total union miss, handled by the existing missing_method check.
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
        assert!(
            union_member_diag_codes(source).is_empty(),
            "union total miss is handled by missing_method, not this check"
        );
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
            union_member_diag_codes(source).is_empty(),
            "untyped member suppresses the union diagnostic"
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
