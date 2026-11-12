//! Editor-independent completion shared by the LSP server and the Playground.
//!
//! `CompletionRequest::at` finds the completion site at a byte offset and masks the
//! partial token so the rest of the file still parses. Callers analyze the masked
//! source with `analysis_mode()`, then call `resolve` to get candidates.

use std::ops::Range;

use ruby_prism::Node;

use crate::analysis::QueryAnalysisMode;
use crate::inference::FileAnalysisSnapshot;
use crate::prism_compat::LocationExt;
use crate::query::TypeQueryEngine;
use crate::rbs::display::{format_method_sig_for_lens_with_names, user_facing_type};
use crate::rbs::stdlib_loader::LazyRbsLoader;
use crate::registry::{
    ConstantCompletionCandidate, ConstantCompletionKind, MethodCompletionCandidate, TypeRegistry,
};
use crate::sym::Sym;
use crate::types::Type;

pub struct CompletionRequest {
    /// Source with the partial token masked by spaces.
    pub source: String,
    /// Byte range in the original source that a chosen item replaces.
    pub replace_range: Range<usize>,
    kind: CompletionKind,
}

enum CompletionKind {
    Method(MethodReceiver),
    Constant {
        namespace: String,
        prefix: String,
        class_context: String,
    },
}

enum MethodReceiver {
    /// The request source calls a probe method here, on the receiver to complete.
    Explicit {
        name_offset: usize,
    },
    Implicit {
        class_context: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionEntryKind {
    Method,
    Class,
    Module,
    Constant,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletionEntry {
    pub label: String,
    pub kind: CompletionEntryKind,
    pub detail: String,
}

impl CompletionRequest {
    pub fn at(source: &str, byte_offset: usize) -> Option<Self> {
        send_method_name_completion_context(source, byte_offset)
            .or_else(|| dot_method_completion_context(source, byte_offset))
            .or_else(|| double_colon_constant_completion_context(source, byte_offset))
    }

    pub(crate) fn analysis_mode(&self) -> QueryAnalysisMode {
        match self.kind {
            CompletionKind::Method(_) => QueryAnalysisMode::HoverSnapshots,
            CompletionKind::Constant { .. } => QueryAnalysisMode::FileFactsOnly,
        }
    }

    /// Returns `None` when the receiver type is unknown. Entries keep ranking order.
    pub fn resolve(
        &self,
        analysis: &FileAnalysisSnapshot,
        stdlib_loader: &LazyRbsLoader,
        external_registry: Option<&TypeRegistry>,
        output_parameter_names: bool,
    ) -> Option<Vec<CompletionEntry>> {
        let query = TypeQueryEngine::new(analysis, &self.source, stdlib_loader)
            .with_external_registry(external_registry);
        match &self.kind {
            CompletionKind::Method(receiver) => {
                let completion = match receiver {
                    MethodReceiver::Explicit { name_offset } => {
                        query.method_completion_at_call(*name_offset)?
                    }
                    MethodReceiver::Implicit { class_context } => {
                        let class_name = if class_context.is_empty() {
                            "Object"
                        } else {
                            class_context
                        };
                        query.method_completion_for_receiver_type(Type::Class(Sym::new(class_name)))
                    }
                };
                Some(
                    completion
                        .candidates
                        .into_iter()
                        .map(|candidate| method_entry(candidate, output_parameter_names))
                        .collect(),
                )
            }
            CompletionKind::Constant {
                namespace,
                prefix,
                class_context,
            } => Some(
                query
                    .constant_completion_candidates(namespace, class_context, prefix)
                    .into_iter()
                    .map(constant_entry)
                    .collect(),
            ),
        }
    }
}

fn method_entry(
    candidate: MethodCompletionCandidate,
    output_parameter_names: bool,
) -> CompletionEntry {
    let separator = if candidate.is_singleton { "." } else { "#" };
    let detail = format!(
        "{}{}{} : {}",
        candidate.owner_class,
        separator,
        candidate.name,
        format_method_sig_for_lens_with_names(&candidate.sig, output_parameter_names)
    );
    CompletionEntry {
        label: candidate.name,
        kind: CompletionEntryKind::Method,
        detail,
    }
}

fn constant_entry(candidate: ConstantCompletionCandidate) -> CompletionEntry {
    let (kind, detail) = match candidate.kind {
        ConstantCompletionKind::Class => (
            CompletionEntryKind::Class,
            format!("class {}", candidate.full_name),
        ),
        ConstantCompletionKind::Module => (
            CompletionEntryKind::Module,
            format!("module {}", candidate.full_name),
        ),
        ConstantCompletionKind::Constant => {
            let detail = candidate
                .const_type
                .as_ref()
                .map(|ty| format!("{} : {}", candidate.full_name, user_facing_type(ty)))
                .unwrap_or(candidate.full_name);
            (CompletionEntryKind::Constant, detail)
        }
    };
    CompletionEntry {
        label: candidate.name,
        kind,
        detail,
    }
}

fn dot_method_completion_context(source: &str, pos_offset: usize) -> Option<CompletionRequest> {
    if pos_offset > source.len() || !source.is_char_boundary(pos_offset) {
        return None;
    }
    let bytes = source.as_bytes();
    let line_start = bytes[..pos_offset]
        .iter()
        .rposition(|b| *b == b'\n')
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let mut prefix_start = pos_offset;
    while prefix_start > line_start && is_ruby_method_completion_char(bytes[prefix_start - 1]) {
        prefix_start -= 1;
    }
    let dot_offset = prefix_start.checked_sub(1)?;
    if bytes.get(dot_offset) != Some(&b'.') {
        return None;
    }
    if dot_offset > line_start && bytes.get(dot_offset - 1) == Some(&b'.') {
        return None;
    }
    Some(CompletionRequest {
        source: with_probe_call_name(source, prefix_start..pos_offset),
        replace_range: prefix_start..pos_offset,
        kind: CompletionKind::Method(MethodReceiver::Explicit {
            name_offset: prefix_start,
        }),
    })
}

fn send_method_name_completion_context(
    source: &str,
    pos_offset: usize,
) -> Option<CompletionRequest> {
    if pos_offset > source.len() || !source.is_char_boundary(pos_offset) {
        return None;
    }
    let bytes = source.as_bytes();
    let line_start = bytes[..pos_offset]
        .iter()
        .rposition(|b| *b == b'\n')
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let mut prefix_start = pos_offset;
    while prefix_start > line_start && is_ruby_method_completion_char(bytes[prefix_start - 1]) {
        prefix_start -= 1;
    }
    let literal_start = prefix_start.checked_sub(1)?;
    if !matches!(
        bytes.get(literal_start),
        Some(b':') | Some(b'"') | Some(b'\'')
    ) {
        return None;
    }

    let open_paren = bytes[..literal_start].iter().rposition(|b| *b == b'(')?;
    if bytes[open_paren + 1..literal_start]
        .iter()
        .any(|b| !b.is_ascii_whitespace())
    {
        return None;
    }

    let mut method_end = open_paren;
    while method_end > line_start && bytes[method_end - 1].is_ascii_whitespace() {
        method_end -= 1;
    }
    let mut method_start = method_end;
    while method_start > line_start && is_ruby_method_completion_char(bytes[method_start - 1]) {
        method_start -= 1;
    }
    let method_name = &source[method_start..method_end];
    if !matches!(
        method_name,
        "send" | "public_send" | "__send__" | "try" | "try!"
    ) {
        return None;
    }

    if !source.is_char_boundary(method_start)
        || !source.is_char_boundary(method_end)
        || !source.is_char_boundary(prefix_start)
        || !source.is_char_boundary(pos_offset)
    {
        return None;
    }

    let explicit_dot = method_start
        .checked_sub(1)
        .is_some_and(|dot_offset| bytes.get(dot_offset) == Some(&b'.'));
    if explicit_dot {
        return Some(CompletionRequest {
            source: with_probe_call_name(source, method_start..pos_offset),
            replace_range: prefix_start..pos_offset,
            kind: CompletionKind::Method(MethodReceiver::Explicit {
                name_offset: method_start,
            }),
        });
    }

    let mut completion_source = bytes.to_vec();
    for byte in &mut completion_source[method_start..pos_offset] {
        *byte = b' ';
    }
    let completion_source = String::from_utf8(completion_source).ok()?;
    let class_context =
        lexical_class_context_at(&completion_source, pos_offset).unwrap_or_default();
    Some(CompletionRequest {
        source: completion_source,
        replace_range: prefix_start..pos_offset,
        kind: CompletionKind::Method(MethodReceiver::Implicit { class_context }),
    })
}

/// Replaces the partial name with a method nobody defines, so the engine infers the receiver
/// the same way as for any call, including literals, parentheses, and leading-dot chains.
fn with_probe_call_name(source: &str, name_range: Range<usize>) -> String {
    const PROBE_METHOD_NAME: &str = "__tyda_completion_probe__";
    let mut probe_source = String::with_capacity(source.len() + PROBE_METHOD_NAME.len());
    probe_source.push_str(&source[..name_range.start]);
    probe_source.push_str(PROBE_METHOD_NAME);
    probe_source.push_str(&source[name_range.end..]);
    probe_source
}

fn is_ruby_method_completion_char(byte: u8) -> bool {
    byte == b'_' || byte == b'?' || byte == b'!' || byte == b'=' || byte.is_ascii_alphanumeric()
}

fn double_colon_constant_completion_context(
    source: &str,
    pos_offset: usize,
) -> Option<CompletionRequest> {
    if pos_offset > source.len() || !source.is_char_boundary(pos_offset) {
        return None;
    }
    let bytes = source.as_bytes();
    let line_start = bytes[..pos_offset]
        .iter()
        .rposition(|b| *b == b'\n')
        .map(|idx| idx + 1)
        .unwrap_or(0);

    let mut prefix_start = pos_offset;
    while prefix_start > line_start && is_ruby_constant_completion_char(bytes[prefix_start - 1]) {
        prefix_start -= 1;
    }

    let double_colon_start = prefix_start.checked_sub(2)?;
    if bytes.get(double_colon_start..prefix_start)? != b"::" {
        return None;
    }

    let mut owner_start = double_colon_start;
    while owner_start > line_start && is_ruby_constant_path_char(bytes[owner_start - 1]) {
        owner_start -= 1;
    }

    if !source.is_char_boundary(owner_start)
        || !source.is_char_boundary(double_colon_start)
        || !source.is_char_boundary(prefix_start)
        || !source.is_char_boundary(pos_offset)
    {
        return None;
    }

    let namespace =
        normalize_constant_completion_namespace(&source[owner_start..double_colon_start])?;
    let prefix = source[prefix_start..pos_offset].to_string();
    let mut completion_source = bytes.to_vec();
    for byte in &mut completion_source[double_colon_start..pos_offset] {
        *byte = b' ';
    }
    let completion_source = String::from_utf8(completion_source).ok()?;
    let class_context =
        lexical_class_context_at(&completion_source, pos_offset).unwrap_or_default();
    let replace_range = prefix_start..pos_offset;

    Some(CompletionRequest {
        source: completion_source,
        replace_range,
        kind: CompletionKind::Constant {
            namespace,
            prefix,
            class_context,
        },
    })
}

pub(crate) fn is_ruby_constant_completion_char(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

pub(crate) fn is_ruby_constant_path_char(byte: u8) -> bool {
    byte == b':' || is_ruby_constant_completion_char(byte)
}

fn normalize_constant_completion_namespace(raw_namespace: &str) -> Option<String> {
    if raw_namespace.is_empty() {
        return Some(String::new());
    }
    let namespace = raw_namespace.strip_prefix("::").unwrap_or(raw_namespace);
    if namespace.is_empty() {
        return Some("::".to_string());
    }
    namespace
        .split("::")
        .all(is_valid_ruby_constant_name)
        .then(|| raw_namespace.to_string())
}

pub(crate) fn is_valid_ruby_constant_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    first.is_ascii_uppercase() && bytes.all(is_ruby_constant_completion_char)
}

pub(crate) fn lexical_class_context_at(source: &str, byte_offset: usize) -> Option<String> {
    let parse_result = ruby_prism::parse(source.as_bytes());
    let root = parse_result.node();
    let mut stack = Vec::new();
    let mut best = None;
    collect_lexical_class_context(&root, byte_offset, &mut stack, &mut best);
    best
}

fn collect_lexical_class_context(
    node: &Node<'_>,
    byte_offset: usize,
    stack: &mut Vec<String>,
    best: &mut Option<String>,
) {
    let loc = node.location();
    if byte_offset < loc.start_offset() || byte_offset > loc.end_offset() {
        return;
    }

    if let Some(program) = node.as_program_node() {
        for child in program.statements().body().iter() {
            collect_lexical_class_context(&child, byte_offset, stack, best);
        }
        return;
    }

    if let Some(statements) = node.as_statements_node() {
        for child in statements.body().iter() {
            collect_lexical_class_context(&child, byte_offset, stack, best);
        }
        return;
    }

    if let Some(class_node) = node.as_class_node() {
        if let Some(raw_name) = constant_path_name_from_node(&class_node.constant_path()) {
            let full_name = qualify_lexical_constant_name(stack.last(), &raw_name);
            stack.push(full_name.clone());
            *best = Some(full_name);
            if let Some(body) = class_node.body() {
                collect_lexical_class_context(&body, byte_offset, stack, best);
            }
            stack.pop();
        }
        return;
    }

    if let Some(module_node) = node.as_module_node()
        && let Some(raw_name) = constant_path_name_from_node(&module_node.constant_path())
    {
        let full_name = qualify_lexical_constant_name(stack.last(), &raw_name);
        stack.push(full_name.clone());
        *best = Some(full_name);
        if let Some(body) = module_node.body() {
            collect_lexical_class_context(&body, byte_offset, stack, best);
        }
        stack.pop();
    }
}

fn constant_path_name_from_node(node: &Node<'_>) -> Option<String> {
    if let Some(read) = node.as_constant_read_node() {
        return Some(String::from_utf8_lossy(read.name().as_slice()).to_string());
    }
    let path = node.as_constant_path_node()?;
    let child = String::from_utf8_lossy(path.name()?.as_slice()).to_string();
    path.parent()
        .map(|parent| {
            let parent_name = constant_path_name_from_node(&parent)?;
            if parent_name.is_empty() {
                Some(child.clone())
            } else {
                Some(format!("{parent_name}::{child}"))
            }
        })
        .unwrap_or_else(|| Some(format!("::{child}")))
}

fn qualify_lexical_constant_name(parent: Option<&String>, raw_name: &str) -> String {
    if raw_name.starts_with("::") || raw_name.contains("::") {
        raw_name.trim_start_matches("::").to_string()
    } else if let Some(parent) = parent
        && !parent.is_empty()
    {
        format!("{parent}::{raw_name}")
    } else {
        raw_name.to_string()
    }
}
