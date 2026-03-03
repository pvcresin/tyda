use ruby_prism::ParseResult;

use crate::rbs::inline::{RbsComment, is_standard_rbs_inline_comment};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorbetSelfBindComment {
    pub raw_type: String,
    pub end_offset: usize,
}

fn no_blank_line_between(bytes: &[u8]) -> bool {
    bytes.iter().filter(|&&b| b == b'\n').count() <= 1
}

/// Extracts Sorbet-compatible inline type comments (supports `#:` / `#|` / modifiers).
pub fn extract_sorbet_rbs_comments(parse_result: &ParseResult<'_>) -> Vec<RbsComment> {
    let mut result = Vec::new();
    for comment in parse_result.comments() {
        let text_bytes = comment.text();
        if text_bytes.starts_with(b"#:")
            || text_bytes.starts_with(b"#|")
            || is_sorbet_modifier_comment_bytes(text_bytes)
        {
            result.push(RbsComment {
                text: String::from_utf8_lossy(text_bytes).into_owned(),
                end_offset: comment.location().end_offset(),
            });
        }
    }
    result
}

pub fn extract_sorbet_self_bind_comments(
    parse_result: &ParseResult<'_>,
) -> Vec<SorbetSelfBindComment> {
    let mut result = Vec::new();
    for comment in parse_result.comments() {
        let text = String::from_utf8_lossy(comment.text());
        let Some(body) = text.strip_prefix("#:") else {
            continue;
        };
        let Some(rest) = body.trim_start().strip_prefix("self as ") else {
            continue;
        };
        let raw_type = rest.trim();
        if raw_type.is_empty() {
            continue;
        }
        result.push(SorbetSelfBindComment {
            raw_type: raw_type.to_string(),
            end_offset: comment.location().end_offset(),
        });
    }
    result
}

/// Sorbet RBS comment immediately preceding a `def` (handles `#|` continuation, skips `# @override`).
pub fn find_sorbet_rbs_comment(
    comments: &[RbsComment],
    def_start_offset: usize,
    source: &[u8],
) -> Option<Vec<String>> {
    if comments.is_empty() {
        return None;
    }
    let mut relevant: Vec<usize> = Vec::new();
    let mut cursor = def_start_offset;
    let mut idx = comments.partition_point(|comment| comment.end_offset <= cursor);
    while idx > 0 {
        idx -= 1;
        let comment = &comments[idx];
        if comment.end_offset > cursor {
            continue;
        }
        let between = &source[comment.end_offset..cursor];
        if !between.iter().all(|b| b.is_ascii_whitespace()) || !no_blank_line_between(between) {
            break;
        }
        let comment_start = comment.end_offset - comment.text.len();
        // A method-preceding annotation must occupy its own line (`attr_reader ... #: String` is inline and doesn't belong to a `def`).
        if !is_comment_at_line_start(source, comment_start) {
            break;
        }
        if is_standard_rbs_inline_comment(&comment.text) || comment.text.starts_with("#|") {
            relevant.push(idx);
            cursor = comment_start;
        } else if is_sorbet_modifier_comment(&comment.text) {
            cursor = comment_start;
        } else {
            break;
        }
    }

    if relevant.is_empty() {
        return None;
    }

    relevant.reverse();
    let merged = merge_sorbet_rbs_comment_lines(
        &relevant
            .iter()
            .map(|&index| comments[index].text.as_str())
            .collect::<Vec<_>>(),
    );
    if merged.is_empty() {
        None
    } else {
        Some(merged)
    }
}

fn is_comment_at_line_start(source: &[u8], comment_start: usize) -> bool {
    let mut idx = comment_start;
    while idx > 0 {
        let b = source[idx - 1];
        if b == b'\n' {
            return true;
        }
        if !b.is_ascii_whitespace() {
            return false;
        }
        idx -= 1;
    }
    true
}

pub fn find_sorbet_modifier_comments(
    comments: &[RbsComment],
    node_start_offset: usize,
    source: &[u8],
) -> Vec<String> {
    if comments.is_empty() {
        return Vec::new();
    }
    let mut relevant: Vec<usize> = Vec::new();
    let mut cursor = node_start_offset;
    let mut idx = comments.partition_point(|comment| comment.end_offset <= cursor);
    while idx > 0 {
        idx -= 1;
        let comment = &comments[idx];
        if comment.end_offset > cursor {
            continue;
        }
        let between = &source[comment.end_offset..cursor];
        if !between.iter().all(|b| b.is_ascii_whitespace()) || !no_blank_line_between(between) {
            break;
        }
        let comment_start = comment.end_offset - comment.text.len();
        if is_sorbet_modifier_comment(&comment.text) {
            relevant.push(idx);
            cursor = comment_start;
        } else if is_standard_rbs_inline_comment(&comment.text) || comment.text.starts_with("#|") {
            cursor = comment_start;
        } else {
            break;
        }
    }
    relevant.reverse();
    relevant
        .into_iter()
        .map(|index| comments[index].text.clone())
        .collect()
}

pub fn find_sorbet_self_bind_for_statement<'a>(
    comments: &'a [SorbetSelfBindComment],
    statement_start_offset: usize,
    source: &[u8],
) -> Option<&'a str> {
    let idx = comments.partition_point(|comment| comment.end_offset <= statement_start_offset);
    let comment = comments.get(idx.checked_sub(1)?)?;
    let between = &source[comment.end_offset..statement_start_offset];
    if !between.iter().all(|b| b.is_ascii_whitespace()) || !no_blank_line_between(between) {
        return None;
    }
    Some(comment.raw_type.as_str())
}

pub fn parse_requires_ancestor_modifier(text: &str) -> Option<String> {
    let rest = text.strip_prefix("# @requires_ancestor:")?.trim();
    (!rest.is_empty()).then(|| rest.trim_start_matches("::").to_string())
}

fn is_sorbet_modifier_comment(text: &str) -> bool {
    text.strip_prefix("# @")
        .and_then(|rest| {
            let ident = rest
                .split(|ch: char| ch == '(' || ch == ':' || ch.is_ascii_whitespace())
                .next()?;
            Some(matches!(
                ident,
                "override"
                    | "abstract"
                    | "final"
                    | "overridable"
                    | "sealed"
                    | "interface"
                    | "requires_ancestor"
            ))
        })
        .unwrap_or(false)
}

fn is_sorbet_modifier_comment_bytes(text: &[u8]) -> bool {
    std::str::from_utf8(text)
        .ok()
        .map(is_sorbet_modifier_comment)
        .unwrap_or(false)
}

fn merge_sorbet_rbs_comment_lines(lines: &[&str]) -> Vec<String> {
    let mut signatures: Vec<String> = Vec::new();
    for line in lines {
        if let Some(body) = line.strip_prefix("#|") {
            if let Some(last) = signatures.last_mut() {
                last.push(' ');
                last.push_str(body.trim_start());
            }
        } else if let Some(body) = line.strip_prefix("#:") {
            signatures.push(format!("#:{body}"));
        } else if is_standard_rbs_inline_comment(line) {
            signatures.push((*line).to_string());
        }
    }
    signatures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_sorbet_comment_extensions() {
        let parse_result = ruby_prism::parse(
            b"#: (String) ->\n#| Integer\n# @override\n# @rbs () -> Symbol\ndef foo(name)\nend\n",
        );

        let comments = extract_sorbet_rbs_comments(&parse_result);
        assert_eq!(
            comments
                .iter()
                .map(|comment| comment.text.as_str())
                .collect::<Vec<_>>(),
            vec!["#: (String) ->", "#| Integer", "# @override"]
        );
    }

    #[test]
    fn finds_sorbet_comment_with_continuation_and_modifier() {
        let source = b"#: (String) ->\n#| Integer\n# @override\ndef foo(name)\nend\n";
        let parse_result = ruby_prism::parse(source);
        let comments = extract_sorbet_rbs_comments(&parse_result);
        let program = parse_result.node().as_program_node().expect("program");
        let def = program.statements().body().iter().next().expect("def node");
        let def_start = def.location().start_offset();

        assert_eq!(
            find_sorbet_rbs_comment(&comments, def_start, source),
            Some(vec!["#: (String) -> Integer".to_string()])
        );
    }

    #[test]
    fn detects_parameterized_and_class_level_sorbet_modifiers() {
        assert!(is_sorbet_modifier_comment(
            "# @override(allow_incompatible: true)"
        ));
        assert!(is_sorbet_modifier_comment("# @requires_ancestor: ::Kernel"));
        assert!(is_sorbet_modifier_comment("# @sealed"));
        assert!(is_sorbet_modifier_comment("# @interface"));
    }

    #[test]
    fn blank_line_breaks_sorbet_comment_attachment() {
        let source = b"#: () -> Integer\n\n# @override\ndef foo\nend\n";
        let parse_result = ruby_prism::parse(source);
        let comments = extract_sorbet_rbs_comments(&parse_result);
        let program = parse_result.node().as_program_node().expect("program");
        let def = program.statements().body().iter().next().expect("def node");
        let def_start = def.location().start_offset();

        assert_eq!(find_sorbet_rbs_comment(&comments, def_start, source), None);
    }

    #[test]
    fn finds_sorbet_modifier_comments_before_class() {
        let source = b"# @requires_ancestor: ::Kernel\n#: [Elem]\nclass Box\nend\n";
        let parse_result = ruby_prism::parse(source);
        let comments = extract_sorbet_rbs_comments(&parse_result);
        let program = parse_result.node().as_program_node().expect("program");
        let class_node = program
            .statements()
            .body()
            .iter()
            .next()
            .expect("class node");
        let class_start = class_node.location().start_offset();

        assert_eq!(
            find_sorbet_modifier_comments(&comments, class_start, source),
            vec!["# @requires_ancestor: ::Kernel".to_string()]
        );
    }

    #[test]
    fn extracts_and_finds_sorbet_self_bind_comment() {
        let source = b"1.times do\n  #: self as Config\n  timeout\nend\n";
        let parse_result = ruby_prism::parse(source);
        let comments = extract_sorbet_self_bind_comments(&parse_result);
        let program = parse_result.node().as_program_node().expect("program");
        let block = program
            .statements()
            .body()
            .iter()
            .next()
            .and_then(|node| node.as_call_node())
            .and_then(|call| call.block())
            .and_then(|node| node.as_block_node())
            .expect("block");
        let body = block.body().expect("block body");
        let statement = body
            .as_statements_node()
            .expect("statements")
            .body()
            .iter()
            .next()
            .expect("statement");

        assert_eq!(comments.len(), 1);
        assert_eq!(
            find_sorbet_self_bind_for_statement(
                &comments,
                statement.location().start_offset(),
                source
            ),
            Some("Config")
        );
    }

    #[test]
    fn parses_requires_ancestor_modifier() {
        assert_eq!(
            parse_requires_ancestor_modifier("# @requires_ancestor: ::Some::Ancestor"),
            Some("Some::Ancestor".to_string())
        );
    }
}
