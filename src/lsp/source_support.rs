use std::collections::HashSet;

use ruby_prism::Node;
use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent, Url};

use crate::types::{MethodSig, SourceLocation, Type};

pub(super) fn uri_to_path(uri: &Url) -> String {
    uri.to_file_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| uri.path().to_string())
}

pub(super) fn lsp_position_to_byte_offset(source: &str, position: Position) -> Option<usize> {
    let target_line = position.line as usize;
    let target_col = position.character as usize;
    let mut line = 0usize;
    let mut col = 0usize;

    for (idx, ch) in source.char_indices() {
        if line == target_line && col == target_col {
            return Some(idx);
        }
        if ch == '\n' {
            if line == target_line {
                return Some(idx);
            }
            line += 1;
            col = 0;
            continue;
        }
        col += ch.len_utf16();
        if line == target_line && col > target_col {
            return None;
        }
    }

    if line == target_line && col == target_col {
        Some(source.len())
    } else {
        None
    }
}

pub(super) fn byte_offset_to_lsp_position(source: &str, target_offset: usize) -> Position {
    let clamped = target_offset.min(source.len());
    let mut line = 0u32;
    let mut col = 0u32;
    for (idx, ch) in source.char_indices() {
        if idx >= clamped {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf16() as u32;
        }
    }
    Position::new(line, col)
}

pub(super) fn apply_content_changes(
    current_source: &str,
    changes: &[TextDocumentContentChangeEvent],
) -> Option<String> {
    let mut source = current_source.to_string();
    for change in changes {
        if let Some(range) = change.range {
            let start = lsp_position_to_byte_offset(&source, range.start)?;
            let end = lsp_position_to_byte_offset(&source, range.end)?;
            if start > end || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
                return None;
            }
            source.replace_range(start..end, &change.text);
        } else {
            source = change.text.clone();
        }
    }
    Some(source)
}

pub(super) fn split_source_lines(source: &str) -> Vec<&str> {
    source.lines().collect()
}

fn def_header_from_line(trimmed: &str) -> Option<(usize, &str)> {
    if let Some(rest) = trimmed.strip_prefix("def ") {
        Some((4, rest))
    } else {
        let wrapper_def_offset = trimmed.find(" def ")?;
        let header_offset = wrapper_def_offset + " def ".len();
        Some((header_offset, trimmed.get(header_offset..)?))
    }
}

fn source_method_identity_from_line(line_text: &str) -> Option<(String, bool, u32)> {
    let trimmed = line_text.trim_start();
    let indent = line_text.len().saturating_sub(trimmed.len()) as u32;
    let (header_offset, after_def) = def_header_from_line(trimmed)?;
    let header = after_def.trim_start();
    let header_offset = header_offset + (after_def.len().saturating_sub(header.len()));
    let header_end = header
        .find(|ch: char| ch.is_whitespace() || matches!(ch, '(' | ';' | '='))
        .unwrap_or(header.len());
    let receiver_and_name = header.get(..header_end)?.trim_end_matches('{').trim();
    if receiver_and_name.is_empty() {
        return None;
    }
    let method_name = receiver_and_name
        .rsplit('.')
        .next()
        .unwrap_or(receiver_and_name)
        .trim();
    if method_name.is_empty() {
        return None;
    }
    let method_offset_in_header = header.rfind(method_name)? as u32;
    let method_column = indent + header_offset as u32 + method_offset_in_header;
    let is_singleton = receiver_and_name.contains('.');
    Some((method_name.to_string(), is_singleton, method_column))
}

fn previous_non_empty_line<'a>(lines: &'a [&'a str], mut line: usize) -> Option<&'a str> {
    while line > 0 {
        line -= 1;
        let trimmed = lines.get(line)?.trim();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

pub(super) fn source_line_has_direct_annotation(lines: &[&str], line: usize) -> bool {
    if let Some(current) = lines.get(line).map(|line| line.trim())
        && current.contains("#:")
    {
        return true;
    }
    let Some(previous) = previous_non_empty_line(lines, line) else {
        return false;
    };
    if previous.starts_with("#:") || previous.starts_with("# @rbs") {
        return true;
    }
    if previous.starts_with("sig")
        && (previous == "sig"
            || previous.starts_with("sig ")
            || previous.starts_with("sig(")
            || previous.starts_with("sig {")
            || previous.starts_with("sig do"))
    {
        return true;
    }
    if previous == "end" {
        for candidate in lines[..line].iter().rev().map(|line| line.trim()) {
            if candidate.is_empty() {
                continue;
            }
            if candidate.starts_with("sig do")
                || candidate == "sig"
                || candidate.starts_with("sig ")
                || candidate.starts_with("sig(")
            {
                return true;
            }
            break;
        }
    }
    false
}

pub(super) fn fallback_code_lens_methods_from_source(
    source: &str,
    covered_lines: &HashSet<usize>,
) -> Vec<(String, MethodSig)> {
    let lines = split_source_lines(source);
    lines
        .iter()
        .enumerate()
        .filter_map(|(line_idx, line_text)| {
            if covered_lines.contains(&line_idx) {
                return None;
            }
            if !source_line_supports_signature_comment_from_lines(&lines, line_idx) {
                return None;
            }
            if source_line_has_direct_annotation(&lines, line_idx) {
                return None;
            }
            let (method_name, is_singleton, def_column) =
                source_method_identity_from_line(line_text)?;
            Some((
                String::new(),
                MethodSig {
                    name: method_name,
                    params: Vec::new(),
                    return_type: Type::Untyped,
                    block: None,
                    sorbet_modifier_comments: Vec::new(),
                    is_singleton,
                    rbs_annotated: false,
                    rbs_inline_annotated: false,
                    sig_annotated: false,
                    rbs_file_source: false,
                    synthetic_dsl_source: false,
                    overloads: Vec::new(),
                    loc: Some(SourceLocation {
                        line: line_idx as u32 + 1,
                        column: def_column,
                    }),
                    is_private: false,
                },
            ))
        })
        .collect()
}

pub(super) fn method_definition_name_offset(source: &str, method: &MethodSig) -> Option<usize> {
    let loc = method.loc?;
    let def_start =
        super::line_col_to_offset(source.as_bytes(), loc.line as usize, loc.column as usize)?;
    let line_end = source.as_bytes()[def_start..]
        .iter()
        .position(|b| *b == b'\n')
        .map(|idx| def_start + idx)
        .unwrap_or(source.len());
    let header = source.get(def_start..line_end)?;
    let rel = header.find(&method.name)?;
    Some(def_start + rel)
}

pub(super) fn method_name_offset_for_definition_line(
    source: &str,
    byte_offset: usize,
) -> Option<usize> {
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
    let line = source.get(line_start..line_end)?;
    let trimmed = line.trim_start();
    let indent = line.len().saturating_sub(trimmed.len());
    let (header_offset, after_def) = def_header_from_line(trimmed)?;
    let header = after_def.trim_start();
    let header_offset = header_offset + (after_def.len().saturating_sub(header.len()));
    let header_end = header
        .find(|ch: char| ch.is_whitespace() || matches!(ch, '(' | ';' | '='))
        .unwrap_or(header.len());
    let receiver_and_name = header.get(..header_end)?.trim_end_matches('{').trim();
    let method_name = receiver_and_name.rsplit('.').next()?.trim();
    let method_rel_start = header.rfind(method_name)?;
    Some(line_start + indent + header_offset + method_rel_start)
}

pub(super) fn code_lens_range_for_method(source: &str, sig: &MethodSig) -> Option<Range> {
    let start = method_definition_name_offset(source, sig).or_else(|| {
        let loc = sig.loc?;
        super::line_col_to_offset(source.as_bytes(), loc.line as usize, loc.column as usize)
    })?;
    let end = start
        .checked_add(sig.name.len())
        .filter(|end| *end <= source.len() && source.is_char_boundary(*end))
        .unwrap_or(start);
    Some(Range::new(
        byte_offset_to_lsp_position(source, start),
        byte_offset_to_lsp_position(source, end),
    ))
}

pub(super) fn offset_to_line_col(source: &[u8], target_offset: usize) -> (usize, usize) {
    let clamped = target_offset.min(source.len());
    let mut line = 1usize;
    let mut col = 0usize;
    for &b in &source[..clamped] {
        if b == b'\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub(in crate::lsp) struct DotMethodCompletionContext {
    pub(in crate::lsp) source: String,
    pub(in crate::lsp) receiver_offset: usize,
    pub(in crate::lsp) replace_range: Range,
}

pub(in crate::lsp) enum SendMethodNameReceiver {
    Explicit { receiver_offset: usize },
    Implicit { class_context: String },
}

pub(in crate::lsp) struct SendMethodNameCompletionContext {
    pub(in crate::lsp) source: String,
    pub(in crate::lsp) receiver: SendMethodNameReceiver,
    pub(in crate::lsp) replace_range: Range,
}

pub(in crate::lsp) struct ConstantPathCompletionContext {
    pub(in crate::lsp) source: String,
    pub(in crate::lsp) namespace: String,
    pub(in crate::lsp) prefix: String,
    pub(in crate::lsp) class_context: String,
    pub(in crate::lsp) replace_range: Range,
}

pub(in crate::lsp) struct RbsCommentTypeDefinitionContext {
    pub(in crate::lsp) type_name: String,
    pub(in crate::lsp) class_context: String,
}

pub(super) fn dot_method_completion_context(
    source: &str,
    pos: Position,
) -> Option<DotMethodCompletionContext> {
    let pos_offset = lsp_position_to_byte_offset(source, pos)?;
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
    let receiver_offset = dot_offset.checked_sub(1)?;
    if !source.is_char_boundary(dot_offset)
        || !source.is_char_boundary(pos_offset)
        || !source.is_char_boundary(receiver_offset)
    {
        return None;
    }

    let mut completion_source = bytes.to_vec();
    for byte in &mut completion_source[dot_offset..pos_offset] {
        *byte = b' ';
    }
    let completion_source = String::from_utf8(completion_source).ok()?;
    let replace_range = Range::new(
        byte_offset_to_lsp_position(source, dot_offset + 1),
        byte_offset_to_lsp_position(source, pos_offset),
    );
    Some(DotMethodCompletionContext {
        source: completion_source,
        receiver_offset,
        replace_range,
    })
}

pub(super) fn send_method_name_completion_context(
    source: &str,
    pos: Position,
) -> Option<SendMethodNameCompletionContext> {
    let pos_offset = lsp_position_to_byte_offset(source, pos)?;
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
        .and_then(|dot_offset| (bytes.get(dot_offset) == Some(&b'.')).then_some(dot_offset));
    let (receiver, mask_start) = if let Some(dot_offset) = explicit_dot {
        let mask_start = if dot_offset > line_start && bytes.get(dot_offset - 1) == Some(&b'&') {
            dot_offset - 1
        } else {
            dot_offset
        };
        let receiver_end = mask_start;
        if receiver_end == line_start || !source.is_char_boundary(receiver_end) {
            return None;
        }
        (
            SendMethodNameReceiver::Explicit {
                receiver_offset: receiver_end - 1,
            },
            mask_start,
        )
    } else {
        let mut completion_source = bytes.to_vec();
        for byte in &mut completion_source[method_start..pos_offset] {
            *byte = b' ';
        }
        let completion_source = String::from_utf8(completion_source).ok()?;
        let class_context =
            lexical_class_context_at(&completion_source, pos_offset).unwrap_or_default();
        let replace_range = Range::new(
            byte_offset_to_lsp_position(source, prefix_start),
            byte_offset_to_lsp_position(source, pos_offset),
        );
        return Some(SendMethodNameCompletionContext {
            source: completion_source,
            receiver: SendMethodNameReceiver::Implicit { class_context },
            replace_range,
        });
    };

    let mut completion_source = bytes.to_vec();
    for byte in &mut completion_source[mask_start..pos_offset] {
        *byte = b' ';
    }
    let completion_source = String::from_utf8(completion_source).ok()?;
    let replace_range = Range::new(
        byte_offset_to_lsp_position(source, prefix_start),
        byte_offset_to_lsp_position(source, pos_offset),
    );
    Some(SendMethodNameCompletionContext {
        source: completion_source,
        receiver,
        replace_range,
    })
}

fn is_ruby_method_completion_char(byte: u8) -> bool {
    byte == b'_' || byte == b'?' || byte == b'!' || byte == b'=' || byte.is_ascii_alphanumeric()
}

pub(super) fn double_colon_constant_completion_context(
    source: &str,
    pos: Position,
) -> Option<ConstantPathCompletionContext> {
    let pos_offset = lsp_position_to_byte_offset(source, pos)?;
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
    let replace_range = Range::new(
        byte_offset_to_lsp_position(source, prefix_start),
        byte_offset_to_lsp_position(source, pos_offset),
    );

    Some(ConstantPathCompletionContext {
        source: completion_source,
        namespace,
        prefix,
        class_context,
        replace_range,
    })
}

fn is_ruby_constant_completion_char(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

fn is_ruby_constant_path_char(byte: u8) -> bool {
    byte == b':' || is_ruby_constant_completion_char(byte)
}

pub(super) fn rbs_comment_type_definition_context(
    source: &str,
    byte_offset: usize,
) -> Option<RbsCommentTypeDefinitionContext> {
    let bytes = source.as_bytes();
    if byte_offset > bytes.len() {
        return None;
    }
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
    if !source.is_char_boundary(line_start)
        || !source.is_char_boundary(byte_offset)
        || !source.is_char_boundary(line_end)
    {
        return None;
    }

    let line = &source[line_start..line_end];
    let local_offset = byte_offset - line_start;
    let body_start = rbs_comment_body_start(line)?;
    if local_offset < body_start {
        return None;
    }

    let (token_start, token_end) = constant_path_token_span(line.as_bytes(), local_offset)?;
    if token_start < body_start {
        return None;
    }
    let type_name = normalize_rbs_type_name_token(&line[token_start..token_end])?;
    let class_context = lexical_class_context_at(source, byte_offset).unwrap_or_default();
    Some(RbsCommentTypeDefinitionContext {
        type_name,
        class_context,
    })
}

fn rbs_comment_body_start(line: &str) -> Option<usize> {
    if let Some(marker_start) = line
        .as_bytes()
        .windows(2)
        .position(|window| window == b"#:" || window == b"#|")
    {
        return Some(marker_start + 2);
    }
    let hash = line.find('#')?;
    let after_hash = &line[hash + 1..];
    let ws_len = after_hash
        .len()
        .saturating_sub(after_hash.trim_start().len());
    let marker_start = hash + 1 + ws_len;
    line[marker_start..]
        .starts_with("@rbs")
        .then_some(marker_start + "@rbs".len())
}

fn constant_path_token_span(bytes: &[u8], local_offset: usize) -> Option<(usize, usize)> {
    let mut cursor = local_offset.min(bytes.len());
    if cursor == bytes.len() || !is_ruby_constant_path_char(*bytes.get(cursor)?) {
        if cursor == 0 || !is_ruby_constant_path_char(bytes[cursor - 1]) {
            return None;
        }
        cursor -= 1;
    }

    let mut start = cursor;
    while start > 0 && is_ruby_constant_path_char(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = cursor;
    while end < bytes.len() && is_ruby_constant_path_char(bytes[end]) {
        end += 1;
    }
    (start < end).then_some((start, end))
}

fn normalize_rbs_type_name_token(token: &str) -> Option<String> {
    let body = if let Some(rest) = token.strip_prefix("::") {
        if rest.starts_with(':') {
            return None;
        }
        rest
    } else {
        token
    };
    if body.is_empty() || body.ends_with("::") {
        return None;
    }
    body.split("::")
        .all(is_valid_ruby_constant_name)
        .then(|| token.to_string())
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

fn is_valid_ruby_constant_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    first.is_ascii_uppercase() && bytes.all(is_ruby_constant_completion_char)
}

fn lexical_class_context_at(source: &str, byte_offset: usize) -> Option<String> {
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

pub(super) fn insert_signature_comment(source: &str, line: u32, rbs_text: &str) -> Option<String> {
    let target_line = line as usize;
    let lines: Vec<&str> = source.lines().collect();
    let def_line = lines.get(target_line)?;
    let trimmed = def_line.trim_start();
    let indent = &def_line[..def_line.len().saturating_sub(trimmed.len())];
    let inserted = format!("{indent}#: {rbs_text}");

    let mut out = String::new();
    for (idx, existing) in lines.iter().enumerate() {
        if idx == target_line {
            out.push_str(&inserted);
            out.push('\n');
        }
        out.push_str(existing);
        out.push('\n');
    }
    Some(out)
}

pub(super) fn source_line_supports_signature_comment_from_lines(
    lines: &[&str],
    line: usize,
) -> bool {
    let Some(line_text) = lines.get(line) else {
        return false;
    };
    let trimmed = line_text.trim_start();
    def_header_from_line(trimmed).is_some()
}

#[cfg(test)]
pub(super) fn source_line_supports_signature_comment(source: &str, line: usize) -> bool {
    let lines = split_source_lines(source);
    source_line_supports_signature_comment_from_lines(&lines, line)
}
