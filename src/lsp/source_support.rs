use std::collections::HashSet;

use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent, Url};

use crate::completion::{
    is_ruby_constant_path_char, is_valid_ruby_constant_name, lexical_class_context_at,
};
use crate::text_position::{byte_offset_to_utf16_position, utf16_position_to_byte_offset};
use crate::types::{MethodSig, SourceLocation, Type};

pub(super) fn uri_to_path(uri: &Url) -> String {
    uri.to_file_path()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| uri.path().to_string())
}

pub(super) fn lsp_position_to_byte_offset(source: &str, position: Position) -> Option<usize> {
    utf16_position_to_byte_offset(source, position.line as usize, position.character as usize)
}

pub(super) fn byte_offset_to_lsp_position(source: &str, target_offset: usize) -> Position {
    let (line, character) = byte_offset_to_utf16_position(source, target_offset);
    Position::new(line, character)
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
                    type_params: Vec::new(),
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

pub(in crate::lsp) struct RbsCommentTypeDefinitionContext {
    pub(in crate::lsp) type_name: String,
    pub(in crate::lsp) class_context: String,
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
