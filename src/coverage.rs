use std::collections::BTreeMap;

use rustc_hash::FxHashMap;
use serde::Serialize;

use crate::inference::FileAnalysisSnapshot;
use crate::inference::hover::{HoverSnapshot, HoverTarget, UnresolvedConstantSite};
use crate::registry::TypeRegistry;
use crate::types::Type;

const COVERAGE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CoverageLevel {
    Unknown,
    Untyped,
    Typed,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CoverageSiteKind {
    Value,
    MethodCall,
    ConstantReference,
    MethodParameter,
    MethodReturn,
    InstanceVariable,
}

impl CoverageSiteKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::MethodCall => "method_call",
            Self::ConstantReference => "constant_reference",
            Self::MethodParameter => "method_parameter",
            Self::MethodReturn => "method_return",
            Self::InstanceVariable => "instance_variable",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CoverageCounts {
    pub total: usize,
    pub typed: usize,
    pub untyped: usize,
    pub unknown: usize,
}

impl CoverageCounts {
    fn add(&mut self, level: CoverageLevel) {
        self.total = self.total.saturating_add(1);
        match level {
            CoverageLevel::Unknown => self.unknown = self.unknown.saturating_add(1),
            CoverageLevel::Untyped => self.untyped = self.untyped.saturating_add(1),
            CoverageLevel::Typed => self.typed = self.typed.saturating_add(1),
        }
    }

    fn merge(&mut self, other: &Self) {
        self.total = self.total.saturating_add(other.total);
        self.typed = self.typed.saturating_add(other.typed);
        self.untyped = self.untyped.saturating_add(other.untyped);
        self.unknown = self.unknown.saturating_add(other.unknown);
    }

    fn percentage(value: usize, total: usize) -> f64 {
        if total == 0 {
            return 0.0;
        }
        (value as f64 * 100.0) / total as f64
    }

    pub fn type_coverage_percent(&self) -> f64 {
        Self::percentage(self.typed, self.total)
    }

    pub fn tracking_coverage_percent(&self) -> f64 {
        Self::percentage(self.typed.saturating_add(self.untyped), self.total)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CoverageFileCounts {
    pub discovered: usize,
    pub analyzed: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CoverageStats {
    #[serde(flatten)]
    pub counts: CoverageCounts,
    pub type_coverage_percent: f64,
    pub tracking_coverage_percent: f64,
}

impl CoverageStats {
    fn from_counts(counts: CoverageCounts) -> Self {
        Self {
            type_coverage_percent: counts.type_coverage_percent(),
            tracking_coverage_percent: counts.tracking_coverage_percent(),
            counts,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CoverageReport {
    pub schema_version: u32,
    pub files: CoverageFileCounts,
    pub declarations: CoverageStats,
    pub references: CoverageStats,
    pub by_kind: BTreeMap<String, CoverageStats>,
}

#[derive(Clone, Copy)]
struct CoverageCandidate {
    kind: CoverageSiteKind,
    level: CoverageLevel,
    explicit_unknown: bool,
}

/// Per-file coverage is reduced to counters before the file analysis snapshot
/// leaves the worker. The source-level span map is only needed while one file
/// is being analyzed to deduplicate overlapping inference observations.
#[derive(Clone, Default)]
pub(crate) struct CoverageFile {
    references: CoverageCounts,
    by_kind: BTreeMap<CoverageSiteKind, CoverageCounts>,
}

#[derive(Default)]
pub(crate) struct CoverageRecorder {
    candidates: FxHashMap<(usize, usize), CoverageCandidate>,
}

impl CoverageRecorder {
    pub(crate) fn record_hover_snapshot(&mut self, snapshot: HoverSnapshot) {
        let HoverSnapshot {
            start,
            end,
            target,
            class_context,
            method_context,
            ..
        } = snapshot;
        if start >= end {
            return;
        }

        let (kind, level) = match target {
            HoverTarget::Value(ty) => {
                let kind = if looks_like_constant_reference(&snapshot.name) {
                    CoverageSiteKind::ConstantReference
                } else {
                    CoverageSiteKind::Value
                };
                let level = if kind == CoverageSiteKind::ConstantReference
                    && matches!(ty, Type::Untyped | Type::Todo)
                {
                    CoverageLevel::Unknown
                } else {
                    type_level(&ty, &class_context, method_context.as_deref())
                };
                (kind, level)
            }
            HoverTarget::MethodCall {
                receiver_type,
                result_type,
            } => (
                CoverageSiteKind::MethodCall,
                method_call_level(
                    &receiver_type,
                    &result_type,
                    &class_context,
                    method_context.as_deref(),
                ),
            ),
            HoverTarget::MethodDefinition { .. } => return,
        };
        self.record_candidate((start, end), kind, level);
    }

    pub(crate) fn record_unresolved_constant_site(&mut self, site: UnresolvedConstantSite) {
        if site.start < site.end {
            self.record_candidate_with_marker(
                (site.start, site.end),
                CoverageSiteKind::ConstantReference,
                CoverageLevel::Unknown,
                true,
            );
        }
    }

    pub(crate) fn finish(self) -> CoverageFile {
        let mut file = CoverageFile::default();
        for candidate in self.candidates.values() {
            file.references.add(candidate.level);
            file.by_kind
                .entry(candidate.kind)
                .or_default()
                .add(candidate.level);
        }
        file
    }

    fn record_candidate(
        &mut self,
        span: (usize, usize),
        kind: CoverageSiteKind,
        level: CoverageLevel,
    ) {
        self.record_candidate_with_marker(span, kind, level, false);
    }

    fn record_candidate_with_marker(
        &mut self,
        span: (usize, usize),
        kind: CoverageSiteKind,
        level: CoverageLevel,
        explicit_unknown: bool,
    ) {
        self.candidates
            .entry(span)
            .and_modify(|existing| {
                if explicit_unknown {
                    existing.kind = CoverageSiteKind::ConstantReference;
                    existing.level = CoverageLevel::Unknown;
                    existing.explicit_unknown = true;
                    return;
                }
                if existing.explicit_unknown {
                    return;
                }
                if level < existing.level
                    || (level == existing.level
                        && site_kind_priority(kind) > site_kind_priority(existing.kind))
                {
                    existing.kind = kind;
                }
                existing.level = if existing.kind == CoverageSiteKind::ConstantReference
                    && kind == CoverageSiteKind::ConstantReference
                {
                    existing.level.max(level)
                } else {
                    merge_levels(existing.level, level)
                };
            })
            .or_insert(CoverageCandidate {
                kind,
                level,
                explicit_unknown,
            });
    }
}

#[derive(Default)]
pub struct CoverageAccumulator {
    analyzed_files: usize,
    declarations: CoverageCounts,
    references: CoverageCounts,
    by_kind: BTreeMap<CoverageSiteKind, CoverageCounts>,
}

impl CoverageAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Consume the compact per-file coverage counters. This deliberately takes
    /// them out of the regular analysis snapshot before the registry merge so
    /// a coverage scan does not retain a second copy of every source site.
    pub fn add_file(&mut self, snapshot: &mut FileAnalysisSnapshot) {
        let Some(file) = snapshot.coverage.take() else {
            return;
        };
        self.analyzed_files = self.analyzed_files.saturating_add(1);
        self.references.merge(&file.references);
        for (kind, counts) in file.by_kind {
            self.by_kind.entry(kind).or_default().merge(&counts);
        }
    }

    pub fn add_declarations(&mut self, registry: &TypeRegistry) {
        for class_name in registry.user_defined_class_names() {
            let Some(data) = registry.class_data_for(&class_name) else {
                continue;
            };

            for method in &data.methods {
                if method.rbs_file_source || method.synthetic_dsl_source {
                    continue;
                }
                let signature = registry.lookup_method_sig_exact(
                    &class_name,
                    method.name.as_ref(),
                    method.is_singleton,
                );

                for index in 0..method.param_infos.len() {
                    let level = signature
                        .as_ref()
                        .and_then(|signature| signature.params.get(index))
                        .map(|param| {
                            type_level(&param.param_type, &class_name, Some(method.name.as_ref()))
                        })
                        .unwrap_or(CoverageLevel::Unknown);
                    self.add_site(CoverageSiteKind::MethodParameter, level);
                }

                let level = signature
                    .as_ref()
                    .map(|signature| {
                        type_level(
                            &signature.return_type,
                            &class_name,
                            Some(method.name.as_ref()),
                        )
                    })
                    .unwrap_or(CoverageLevel::Unknown);
                self.add_site(CoverageSiteKind::MethodReturn, level);
            }

            for types in data.ivars.values() {
                let level = types
                    .iter()
                    .map(|ty| type_level(ty, &class_name, None))
                    .reduce(merge_levels)
                    .unwrap_or(CoverageLevel::Untyped);
                self.add_site(CoverageSiteKind::InstanceVariable, level);
            }
        }
    }

    pub fn report(&self, discovered_files: usize) -> CoverageReport {
        let by_kind = self
            .by_kind
            .iter()
            .map(|(kind, counts)| {
                (
                    kind.as_str().to_string(),
                    CoverageStats::from_counts(counts.clone()),
                )
            })
            .collect();

        CoverageReport {
            schema_version: COVERAGE_SCHEMA_VERSION,
            files: CoverageFileCounts {
                discovered: discovered_files,
                analyzed: self.analyzed_files,
            },
            declarations: CoverageStats::from_counts(self.declarations.clone()),
            references: CoverageStats::from_counts(self.references.clone()),
            by_kind,
        }
    }

    fn add_site(&mut self, kind: CoverageSiteKind, level: CoverageLevel) {
        self.declarations.add(level);
        self.by_kind.entry(kind).or_default().add(level);
    }
}

fn method_call_level(
    receiver_type: &Type,
    result_type: &Type,
    class_context: &str,
    method_context: Option<&str>,
) -> CoverageLevel {
    match type_level(receiver_type, class_context, method_context) {
        CoverageLevel::Typed => type_level(result_type, class_context, method_context),
        CoverageLevel::Unknown | CoverageLevel::Untyped => CoverageLevel::Unknown,
    }
}

fn type_level(ty: &Type, class_context: &str, method_context: Option<&str>) -> CoverageLevel {
    match ty {
        Type::Untyped | Type::Todo | Type::BlockReturnRef => CoverageLevel::Untyped,
        Type::ParamRef(_) | Type::KeywordParamRef(_) => {
            if method_context.is_some() {
                CoverageLevel::Untyped
            } else {
                CoverageLevel::Unknown
            }
        }
        Type::IvarRef(_) | Type::GlobalVariableRef(_) => {
            if class_context.is_empty() {
                CoverageLevel::Unknown
            } else {
                CoverageLevel::Untyped
            }
        }
        Type::MethodReturnRef(owner, _) => {
            if owner.is_empty() || is_unknown_name(owner.as_ref()) {
                CoverageLevel::Unknown
            } else {
                CoverageLevel::Untyped
            }
        }
        Type::ReceiverMethodRef(receiver, _) => {
            match type_level(receiver, class_context, method_context) {
                CoverageLevel::Unknown => CoverageLevel::Unknown,
                CoverageLevel::Untyped | CoverageLevel::Typed => CoverageLevel::Untyped,
            }
        }
        Type::Class(name) | Type::Singleton(name) => {
            if name.is_empty() || is_unknown_name(name.as_ref()) {
                CoverageLevel::Unknown
            } else {
                CoverageLevel::Typed
            }
        }
        Type::Intersection(parts) | Type::Union(parts) | Type::Tuple(parts) => parts
            .iter()
            .map(|part| type_level(part, class_context, method_context))
            .fold(CoverageLevel::Typed, merge_levels),
        Type::Array(Some(inner)) => type_level(inner, class_context, method_context),
        Type::Array(None) => CoverageLevel::Typed,
        Type::Hash(key, value) => [key.as_deref(), value.as_deref()]
            .into_iter()
            .flatten()
            .map(|part| type_level(part, class_context, method_context))
            .fold(CoverageLevel::Typed, merge_levels),
        Type::Record(fields) => fields
            .iter()
            .map(|field| type_level(&field.value, class_context, method_context))
            .fold(CoverageLevel::Typed, merge_levels),
        Type::Proc { return_type, .. } => type_level(return_type, class_context, method_context),
        Type::Generic { base, args } => {
            if base.is_empty() || is_unknown_name(base.as_ref()) {
                CoverageLevel::Unknown
            } else {
                args.iter()
                    .map(|arg| type_level(arg, class_context, method_context))
                    .fold(CoverageLevel::Typed, merge_levels)
            }
        }
        Type::PatternIndexRef(subject, _)
        | Type::PatternRestRef(subject)
        | Type::PatternTrailingRef(subject, _)
        | Type::PatternKeyRef(subject, _)
        | Type::PatternKeyRestRef(subject, _) => {
            match type_level(subject, class_context, method_context) {
                CoverageLevel::Unknown => CoverageLevel::Unknown,
                CoverageLevel::Untyped | CoverageLevel::Typed => CoverageLevel::Untyped,
            }
        }
        Type::LiteralInteger(_)
        | Type::LiteralFloat(_)
        | Type::LiteralString(_)
        | Type::LiteralSymbol(_)
        | Type::Integer
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
        | Type::SelfType
        | Type::InstanceType => CoverageLevel::Typed,
    }
}

fn merge_levels(left: CoverageLevel, right: CoverageLevel) -> CoverageLevel {
    match (left, right) {
        (CoverageLevel::Unknown, _) | (_, CoverageLevel::Unknown) => CoverageLevel::Unknown,
        (CoverageLevel::Untyped, _) | (_, CoverageLevel::Untyped) => CoverageLevel::Untyped,
        _ => CoverageLevel::Typed,
    }
}

fn site_kind_priority(kind: CoverageSiteKind) -> u8 {
    match kind {
        CoverageSiteKind::ConstantReference => 3,
        CoverageSiteKind::MethodCall => 2,
        CoverageSiteKind::Value => 1,
        CoverageSiteKind::MethodParameter
        | CoverageSiteKind::MethodReturn
        | CoverageSiteKind::InstanceVariable => 0,
    }
}

fn is_unknown_name(name: &str) -> bool {
    name == "Unknown" || name.starts_with("Unknown::")
}

fn looks_like_constant_reference(name: &str) -> bool {
    name.rsplit("::")
        .next()
        .and_then(|segment| segment.chars().next())
        .is_some_and(|ch| ch == '_' || ch.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::hover::HoverSnapshot;

    fn make_snapshot(start: usize, end: usize, target: HoverTarget) -> HoverSnapshot {
        HoverSnapshot {
            start,
            end,
            name: String::new(),
            target,
            class_context: "Object".to_string(),
            method_context: Some("main".to_string()),
        }
    }

    fn snapshot_file(snapshots: impl IntoIterator<Item = HoverSnapshot>) -> FileAnalysisSnapshot {
        let mut recorder = CoverageRecorder::default();
        for snapshot in snapshots {
            recorder.record_hover_snapshot(snapshot);
        }
        let mut file = FileAnalysisSnapshot::empty();
        file.coverage = Some(recorder.finish());
        file
    }

    #[test]
    fn type_level_classifies_nested_and_deferred_types() {
        assert_eq!(
            type_level(&Type::String, "Object", None),
            CoverageLevel::Typed
        );
        assert_eq!(
            type_level(&Type::Untyped, "Object", None),
            CoverageLevel::Untyped
        );
        assert_eq!(
            type_level(&Type::Array(Some(Box::new(Type::Untyped))), "Object", None,),
            CoverageLevel::Untyped
        );
        assert_eq!(
            type_level(
                &Type::Generic {
                    base: "Array".into(),
                    args: vec![Type::Class("Unknown".into())].into(),
                },
                "Object",
                None,
            ),
            CoverageLevel::Unknown
        );
        assert_eq!(
            type_level(&Type::ParamRef(0), "Object", Some("main")),
            CoverageLevel::Untyped
        );
    }

    #[test]
    fn unknown_receiver_makes_a_method_call_unknown() {
        let snapshot = make_snapshot(
            0,
            1,
            HoverTarget::MethodCall {
                receiver_type: Type::Untyped,
                result_type: Type::String,
            },
        );
        let mut recorder = CoverageRecorder::default();
        recorder.record_hover_snapshot(snapshot);
        let file = recorder.finish();
        assert_eq!(file.references.unknown, 1);
        assert_eq!(file.by_kind[&CoverageSiteKind::MethodCall].unknown, 1);
    }

    #[test]
    fn unknown_sites_win_when_spans_are_merged() {
        let mut recorder = CoverageRecorder::default();
        recorder.record_hover_snapshot(make_snapshot(0, 1, HoverTarget::Value(Type::String)));
        recorder.record_hover_snapshot(make_snapshot(0, 1, HoverTarget::Value(Type::Untyped)));
        recorder.record_unresolved_constant_site(UnresolvedConstantSite {
            start: 0,
            end: 1,
            name: "Unknown".to_string(),
            class_context: "Object".to_string(),
        });

        let file = recorder.finish();
        assert_eq!(file.references.total, 1);
        assert_eq!(file.references.unknown, 1);
        assert_eq!(file.references.typed, 0);
        assert_eq!(file.references.untyped, 0);
    }

    #[test]
    fn report_exposes_typed_and_tracking_rates() {
        let mut snapshot = snapshot_file([
            make_snapshot(0, 1, HoverTarget::Value(Type::String)),
            make_snapshot(1, 2, HoverTarget::Value(Type::Integer)),
            make_snapshot(2, 3, HoverTarget::Value(Type::Untyped)),
        ]);
        let mut recorder = CoverageRecorder::default();
        recorder.record_unresolved_constant_site(UnresolvedConstantSite {
            start: 3,
            end: 4,
            name: "Missing".to_string(),
            class_context: "Object".to_string(),
        });
        let mut file = recorder.finish();
        let values = snapshot.coverage.take().expect("coverage file");
        file.references.merge(&values.references);
        for (kind, counts) in values.by_kind {
            file.by_kind.entry(kind).or_default().merge(&counts);
        }
        snapshot.coverage = Some(file);

        let mut accumulator = CoverageAccumulator::new();
        accumulator.add_file(&mut snapshot);
        let report = accumulator.report(1);
        assert_eq!(
            report.references.counts,
            CoverageCounts {
                total: 4,
                typed: 2,
                untyped: 1,
                unknown: 1,
            }
        );
        assert_eq!(report.references.type_coverage_percent, 50.0);
        assert_eq!(report.references.tracking_coverage_percent, 75.0);
    }
}
