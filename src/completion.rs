//! Editor-independent completion shared by the LSP server and the Playground.
//!
//! `CompletionRequest::at` finds the completion site at a byte offset and masks the
//! partial token so the rest of the file still parses. Callers analyze the masked
//! source with `analysis_mode()`, then call `resolve` to get candidates.

use std::ops::Range;

use ruby_prism::{Node, Visit};

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
    Method {
        receiver: MethodReceiver,
        /// Implicit-receiver, `self.`, and `send` calls may reach private methods.
        include_private: bool,
    },
    Constant {
        namespace: String,
        prefix: String,
        class_context: String,
    },
    /// A bare identifier: locals, methods of `self`, constants, and keywords.
    Identifier { prefix: String, scope: LexicalScope },
}

enum MethodReceiver {
    /// The request source calls a probe method here, on the receiver to complete.
    Explicit {
        name_offset: usize,
    },
    Implicit(LexicalScope),
}

/// What a position sees lexically: the enclosing class, the kind of `self`, and locals.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LexicalScope {
    pub class_context: String,
    /// `self` is the class or module itself (class body, `def self.x`, `class << self`).
    pub singleton_self: bool,
    pub locals: Vec<String>,
}

impl LexicalScope {
    fn self_type(&self) -> Type {
        if self.class_context.is_empty() {
            return Type::Class(Sym::new("Object"));
        }
        let name = Sym::new(&self.class_context);
        if self.singleton_self {
            Type::Singleton(name)
        } else {
            Type::Class(name)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionEntryKind {
    Method,
    Class,
    Module,
    Constant,
    Variable,
    Keyword,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletionEntry {
    pub label: String,
    pub kind: CompletionEntryKind,
    pub detail: String,
    /// Class or module that defines a method candidate, shown next to the label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

const RUBY_KEYWORDS: [&str; 41] = [
    "BEGIN",
    "END",
    "__ENCODING__",
    "__FILE__",
    "__LINE__",
    "alias",
    "and",
    "begin",
    "break",
    "case",
    "class",
    "def",
    "defined?",
    "do",
    "else",
    "elsif",
    "end",
    "ensure",
    "false",
    "for",
    "if",
    "in",
    "module",
    "next",
    "nil",
    "not",
    "or",
    "redo",
    "rescue",
    "retry",
    "return",
    "self",
    "super",
    "then",
    "true",
    "undef",
    "unless",
    "until",
    "when",
    "while",
    "yield",
];

impl CompletionRequest {
    pub fn at(source: &str, byte_offset: usize) -> Option<Self> {
        send_method_name_completion_context(source, byte_offset)
            .or_else(|| dot_method_completion_context(source, byte_offset))
            .or_else(|| double_colon_constant_completion_context(source, byte_offset))
            .or_else(|| identifier_completion_context(source, byte_offset))
    }

    pub(crate) fn analysis_mode(&self) -> QueryAnalysisMode {
        match self.kind {
            CompletionKind::Method { .. } => QueryAnalysisMode::HoverSnapshots,
            CompletionKind::Constant { .. } | CompletionKind::Identifier { .. } => {
                QueryAnalysisMode::FileFactsOnly
            }
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
            CompletionKind::Method {
                receiver,
                include_private,
            } => {
                let completion = match receiver {
                    MethodReceiver::Explicit { name_offset } => {
                        query.method_completion_at_call(*name_offset, *include_private)?
                    }
                    MethodReceiver::Implicit(scope) => query
                        .method_completion_for_receiver_type(scope.self_type(), *include_private),
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
            CompletionKind::Identifier { prefix, scope } => Some(identifier_entries(
                &query,
                prefix,
                scope,
                output_parameter_names,
            )),
        }
    }
}

/// Ranks locals first, then methods and constants (constants first for a capitalized
/// prefix), then keywords. A local hides a method of the same name.
fn identifier_entries(
    query: &TypeQueryEngine<'_>,
    prefix: &str,
    scope: &LexicalScope,
    output_parameter_names: bool,
) -> Vec<CompletionEntry> {
    let capitalized = prefix.starts_with(|c: char| c.is_ascii_uppercase());
    let completion =
        query.identifier_completion(scope.self_type(), &scope.class_context, prefix, capitalized);

    let locals: Vec<CompletionEntry> = scope
        .locals
        .iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionEntry {
            label: name.clone(),
            kind: CompletionEntryKind::Variable,
            detail: format!("local variable {name}"),
            owner: None,
        })
        .collect();
    let methods = completion
        .methods
        .into_iter()
        .filter(|candidate| !scope.locals.contains(&candidate.name))
        .map(|candidate| method_entry(candidate, output_parameter_names));
    let constants = completion.constants.into_iter().map(constant_entry);
    let keywords = RUBY_KEYWORDS
        .iter()
        .filter(|keyword| keyword.starts_with(prefix))
        .map(|keyword| CompletionEntry {
            label: keyword.to_string(),
            kind: CompletionEntryKind::Keyword,
            detail: "keyword".to_string(),
            owner: None,
        });

    let mut entries = locals;
    if capitalized {
        entries.extend(constants);
        entries.extend(methods);
    } else {
        entries.extend(methods);
        entries.extend(constants);
    }
    entries.extend(keywords);
    entries
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
        owner: Some(candidate.owner_class),
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
        owner: None,
    }
}

fn dot_method_completion_context(source: &str, pos_offset: usize) -> Option<CompletionRequest> {
    if pos_offset > source.len() || !source.is_char_boundary(pos_offset) {
        return None;
    }
    let bytes = source.as_bytes();
    let line_start = line_start_before(bytes, pos_offset);

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
        kind: CompletionKind::Method {
            receiver: MethodReceiver::Explicit {
                name_offset: prefix_start,
            },
            include_private: receiver_is_self(bytes, dot_offset),
        },
    })
}

/// Whether the receiver before the `.` at `dot_offset` (or its `&.`) is `self`.
fn receiver_is_self(bytes: &[u8], dot_offset: usize) -> bool {
    let mut end = dot_offset;
    if end > 0 && bytes[end - 1] == b'&' {
        end -= 1;
    }
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let Some(start) = end.checked_sub(4) else {
        return false;
    };
    &bytes[start..end] == b"self"
        && (start == 0
            || !matches!(bytes[start - 1], b'.' | b':' | b'@' | b'$' | b'_')
                && !bytes[start - 1].is_ascii_alphanumeric())
}

fn line_start_before(bytes: &[u8], offset: usize) -> usize {
    bytes[..offset]
        .iter()
        .rposition(|b| *b == b'\n')
        .map(|idx| idx + 1)
        .unwrap_or(0)
}

/// A bare identifier such as `pu|` or `Fl|`, outside comments, strings, and `def` names.
fn identifier_completion_context(source: &str, pos_offset: usize) -> Option<CompletionRequest> {
    if pos_offset > source.len() || !source.is_char_boundary(pos_offset) {
        return None;
    }
    let bytes = source.as_bytes();
    let line_start = line_start_before(bytes, pos_offset);
    let mut prefix_start = pos_offset;
    while prefix_start > line_start && is_ruby_constant_completion_char(bytes[prefix_start - 1]) {
        prefix_start -= 1;
    }
    let prefix = &source[prefix_start..pos_offset];
    if !prefix.starts_with(|c: char| c == '_' || c.is_ascii_alphabetic()) {
        return None;
    }
    if prefix_start > line_start
        && matches!(
            bytes[prefix_start - 1],
            b'.' | b':' | b'@' | b'$' | b'?' | b'!' | b'&'
        )
    {
        return None;
    }
    let before = source[line_start..prefix_start].trim_end();
    if before == "def" || before.ends_with(" def") || before.ends_with(";def") {
        return None;
    }

    let mut completion_source = bytes.to_vec();
    for byte in &mut completion_source[prefix_start..pos_offset] {
        *byte = b' ';
    }
    let completion_source = String::from_utf8(completion_source).ok()?;
    let (scope, in_code) = lexical_scope_at(&completion_source, prefix_start);
    if !in_code {
        return None;
    }
    Some(CompletionRequest {
        source: completion_source,
        replace_range: prefix_start..pos_offset,
        kind: CompletionKind::Identifier {
            prefix: prefix.to_string(),
            scope,
        },
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
        // `send` and `__send__` bypass visibility; `public_send` and `try` do not.
        let include_private =
            matches!(method_name, "send" | "__send__") || receiver_is_self(bytes, method_start - 1);
        return Some(CompletionRequest {
            source: with_probe_call_name(source, method_start..pos_offset),
            replace_range: prefix_start..pos_offset,
            kind: CompletionKind::Method {
                receiver: MethodReceiver::Explicit {
                    name_offset: method_start,
                },
                include_private,
            },
        });
    }

    let mut completion_source = bytes.to_vec();
    for byte in &mut completion_source[method_start..pos_offset] {
        *byte = b' ';
    }
    let completion_source = String::from_utf8(completion_source).ok()?;
    let scope = lexical_scope_at(&completion_source, method_start).0;
    Some(CompletionRequest {
        source: completion_source,
        replace_range: prefix_start..pos_offset,
        kind: CompletionKind::Method {
            receiver: MethodReceiver::Implicit(scope),
            include_private: true,
        },
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
    let (scope, _) = lexical_scope_at(source, byte_offset);
    (!scope.class_context.is_empty()).then_some(scope.class_context)
}

/// Returns the scope and whether the offset is in code, not in a comment or string content.
pub(crate) fn lexical_scope_at(source: &str, byte_offset: usize) -> (LexicalScope, bool) {
    let parse_result = ruby_prism::parse(source.as_bytes());
    let in_comment = parse_result.comments().any(|comment| {
        let loc = comment.location();
        loc.start_offset() <= byte_offset && byte_offset <= loc.end_offset()
    });
    let mut collector = LexicalScopeCollector {
        byte_offset,
        current: LexicalScope::default(),
        in_singleton_class: false,
        best: LexicalScope::default(),
        in_string: false,
    };
    collector.visit(&parse_result.node());
    (collector.best, !in_comment && !collector.in_string)
}

struct LexicalScopeCollector {
    byte_offset: usize,
    current: LexicalScope,
    in_singleton_class: bool,
    /// The scope of the innermost node that contains the offset.
    best: LexicalScope,
    in_string: bool,
}

impl LexicalScopeCollector {
    fn contains(&self, node_loc: &ruby_prism::Location<'_>) -> bool {
        node_loc.start_offset() <= self.byte_offset && self.byte_offset <= node_loc.end_offset()
    }

    /// Runs `visit` inside a scope; a hard scope (`def`, `class`) drops outer locals.
    fn with_scope(
        &mut self,
        update: impl FnOnce(&mut LexicalScope, &mut bool),
        locals: ruby_prism::ConstantList<'_>,
        hard_scope: bool,
        visit: impl FnOnce(&mut Self),
    ) {
        let saved = self.current.clone();
        let saved_singleton_class = self.in_singleton_class;
        update(&mut self.current, &mut self.in_singleton_class);
        if hard_scope {
            self.current.locals.clear();
        }
        for local in locals.iter() {
            let name = String::from_utf8_lossy(local.as_slice()).to_string();
            if !self.current.locals.contains(&name) {
                self.current.locals.push(name);
            }
        }
        self.best = self.current.clone();
        visit(self);
        self.current = saved;
        self.in_singleton_class = saved_singleton_class;
    }

    fn enter_namespace(&mut self, constant_path: &Node<'_>) -> Option<String> {
        let raw_name = constant_path_name_from_node(constant_path)?;
        let parent =
            (!self.current.class_context.is_empty()).then_some(&self.current.class_context);
        Some(qualify_lexical_constant_name(parent, &raw_name))
    }
}

impl<'pr> Visit<'pr> for LexicalScopeCollector {
    fn visit_program_node(&mut self, node: &ruby_prism::ProgramNode<'pr>) {
        self.with_scope(
            |_, _| {},
            node.locals(),
            true,
            |this| ruby_prism::visit_program_node(this, node),
        );
    }

    fn visit_class_node(&mut self, node: &ruby_prism::ClassNode<'pr>) {
        if !self.contains(&node.location()) {
            return;
        }
        let Some(full_name) = self.enter_namespace(&node.constant_path()) else {
            return;
        };
        self.with_scope(
            |scope, in_singleton_class| {
                scope.class_context = full_name;
                scope.singleton_self = true;
                *in_singleton_class = false;
            },
            node.locals(),
            true,
            |this| ruby_prism::visit_class_node(this, node),
        );
    }

    fn visit_module_node(&mut self, node: &ruby_prism::ModuleNode<'pr>) {
        if !self.contains(&node.location()) {
            return;
        }
        let Some(full_name) = self.enter_namespace(&node.constant_path()) else {
            return;
        };
        self.with_scope(
            |scope, in_singleton_class| {
                scope.class_context = full_name;
                scope.singleton_self = true;
                *in_singleton_class = false;
            },
            node.locals(),
            true,
            |this| ruby_prism::visit_module_node(this, node),
        );
    }

    fn visit_singleton_class_node(&mut self, node: &ruby_prism::SingletonClassNode<'pr>) {
        if !self.contains(&node.location()) {
            return;
        }
        self.with_scope(
            |scope, in_singleton_class| {
                scope.singleton_self = true;
                *in_singleton_class = true;
            },
            node.locals(),
            true,
            |this| ruby_prism::visit_singleton_class_node(this, node),
        );
    }

    fn visit_def_node(&mut self, node: &ruby_prism::DefNode<'pr>) {
        if !self.contains(&node.location()) {
            return;
        }
        let singleton = node.receiver().is_some() || self.in_singleton_class;
        self.with_scope(
            |scope, _| scope.singleton_self = singleton,
            node.locals(),
            true,
            |this| ruby_prism::visit_def_node(this, node),
        );
    }

    fn visit_block_node(&mut self, node: &ruby_prism::BlockNode<'pr>) {
        if !self.contains(&node.location()) {
            return;
        }
        self.with_scope(
            |_, _| {},
            node.locals(),
            false,
            |this| ruby_prism::visit_block_node(this, node),
        );
    }

    fn visit_lambda_node(&mut self, node: &ruby_prism::LambdaNode<'pr>) {
        if !self.contains(&node.location()) {
            return;
        }
        self.with_scope(
            |_, _| {},
            node.locals(),
            false,
            |this| ruby_prism::visit_lambda_node(this, node),
        );
    }

    fn visit_string_node(&mut self, node: &ruby_prism::StringNode<'pr>) {
        let content = node.content_loc();
        if content.start_offset() <= self.byte_offset && self.byte_offset <= content.end_offset() {
            self.in_string = true;
        }
    }

    fn visit_x_string_node(&mut self, node: &ruby_prism::XStringNode<'pr>) {
        let content = node.content_loc();
        if content.start_offset() <= self.byte_offset && self.byte_offset <= content.end_offset() {
            self.in_string = true;
        }
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

#[cfg(test)]
mod tests {
    use super::receiver_is_self;

    #[test]
    fn receiver_is_self_reads_only_a_bare_self_token() {
        for (source, expected) in [
            ("self.", true),
            ("self&.", true),
            ("self\n  .", true),
            ("x = self.", true),
            ("myself.", false),
            ("@self.", false),
            ("obj.self.", false),
            ("self_.", false),
            (".", false),
        ] {
            let dot_offset = source.rfind('.').expect("dot");
            assert_eq!(
                receiver_is_self(source.as_bytes(), dot_offset),
                expected,
                "{source:?}"
            );
        }
    }
}
