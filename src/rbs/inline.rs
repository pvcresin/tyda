use crate::rbs::ir as rbs_ir;
use std::collections::HashMap;

use ruby_prism::ParseResult;

use crate::rbs::convert::convert_rbs_type;
use crate::rbs::import::{convert_imported_rbs_type, resolve_imported_method_types_aliases};
use crate::types::{ParamKind, Type};

#[derive(Debug, Clone)]
pub struct RbsComment {
    pub text: String,
    pub end_offset: usize,
}

pub struct RbsShorthandResult {
    pub param_types: Vec<(Type, ParamKind)>,
    pub return_type: Type,
    pub method_types: Vec<rbs_ir::MethodType>,
    pub block_type: Option<Type>,
}

#[derive(Clone)]
pub struct RbsParamAnnotation {
    pub name: String,
    pub ty: Type,
    pub is_block: bool,
    pub block_param_types: Vec<Type>,
    pub block_return_type: Option<Type>,
}

fn no_blank_line_between(bytes: &[u8]) -> bool {
    bytes.iter().filter(|&&b| b == b'\n').count() <= 1
}

fn is_inline_comment_at_line_start(source: &[u8], comment_start: usize) -> bool {
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

pub(crate) fn is_standard_rbs_inline_comment(text: &str) -> bool {
    if text.starts_with("#:") {
        return true;
    }
    let Some(body) = text.strip_prefix('#') else {
        return false;
    };
    let body = body.trim_start();
    let Some(rest) = body.strip_prefix("@rbs") else {
        return false;
    };
    rest.is_empty() || rest.chars().next().is_some_and(char::is_whitespace)
}

fn is_standard_rbs_inline_comment_bytes(text: &[u8]) -> bool {
    std::str::from_utf8(text)
        .ok()
        .is_some_and(is_standard_rbs_inline_comment)
}

#[derive(Debug, Clone, PartialEq)]
pub enum InlineAssertion {
    Explicit(Type),
    NonNil,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineAssertionComment {
    pub start_offset: usize,
    pub end_offset: usize,
    pub assertion: InlineAssertion,
}

pub fn extract_inline_assertion_comments(
    parse_result: &ParseResult<'_>,
) -> Vec<InlineAssertionComment> {
    let mut result = Vec::new();
    for comment in parse_result.comments() {
        let text_bytes = comment.text();
        if !text_bytes.starts_with(b"#:") {
            continue;
        }
        let text = String::from_utf8_lossy(text_bytes);
        let Some(body) = text.strip_prefix("#:") else {
            continue;
        };
        let Some(assertion) = parse_inline_assertion(body) else {
            continue;
        };
        let end_offset = comment.location().end_offset();
        let start_offset = end_offset - text_bytes.len();
        result.push(InlineAssertionComment {
            start_offset,
            end_offset,
            assertion,
        });
    }
    result
}

pub fn find_inline_assertion_for_node<'a>(
    comments: &'a [InlineAssertionComment],
    source: &[u8],
    node_start_offset: usize,
    node_end_offset: usize,
) -> Option<&'a InlineAssertion> {
    let line_end = source[node_end_offset..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| node_end_offset + p)
        .unwrap_or(source.len());
    let idx = comments.partition_point(|comment| comment.start_offset < node_end_offset);
    for comment in &comments[idx..] {
        if comment.start_offset > line_end {
            break;
        }
        let between = &source[node_end_offset..comment.start_offset];
        if !between.contains(&b'\n') {
            return Some(&comment.assertion);
        }
    }

    if !source[node_start_offset..node_end_offset].contains(&b'\n') {
        return None;
    }
    let first_line_end = source[node_start_offset..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| node_start_offset + p)
        .unwrap_or(source.len());
    let idx = comments.partition_point(|comment| comment.start_offset < node_start_offset);
    for comment in &comments[idx..] {
        if comment.start_offset > first_line_end {
            break;
        }
        if source[node_start_offset..comment.start_offset]
            .windows(2)
            .any(|window| window == b"<<")
        {
            return Some(&comment.assertion);
        }
    }
    None
}

fn parse_inline_assertion(body: &str) -> Option<InlineAssertion> {
    let body = body.trim();
    if body.is_empty() || body.starts_with("type ") {
        return None;
    }
    if let Some(rest) = body.strip_prefix("as ") {
        let rest = rest.trim();
        if rest == "!nil" {
            return Some(InlineAssertion::NonNil);
        }
        if rest.starts_with("self as ") {
            return None;
        }
        return parse_inline_assertion_type(rest).map(InlineAssertion::Explicit);
    }
    if body == "absurd" {
        return Some(InlineAssertion::Explicit(Type::Bot));
    }
    parse_inline_assertion_type(body).map(InlineAssertion::Explicit)
}

fn parse_inline_assertion_type(type_str: &str) -> Option<Type> {
    let rbs_type = rbs_ir::RbsType::from(&rbs_sys::parse_type(type_str).ok()?);
    Some(convert_rbs_type(&rbs_type))
}

pub fn extract_rbs_inline_comments(parse_result: &ParseResult<'_>) -> Vec<RbsComment> {
    let mut result = Vec::new();
    for comment in parse_result.comments() {
        let text_bytes = comment.text();
        if is_standard_rbs_inline_comment_bytes(text_bytes) {
            result.push(RbsComment {
                text: String::from_utf8_lossy(text_bytes).into_owned(),
                end_offset: comment.location().end_offset(),
            });
        }
    }
    result
}

pub fn find_rbs_inline_comment(
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
        // Only a `#:` at the start of a line counts as an annotation right before `def` (same-line inline attaches to the statement).
        if !is_inline_comment_at_line_start(source, comment_start) {
            break;
        }
        if is_standard_rbs_inline_comment(&comment.text) {
            relevant.push(idx);
            cursor = comment_start;
        } else {
            break;
        }
    }

    if relevant.is_empty() {
        return None;
    }

    relevant.reverse();
    Some(
        relevant
            .into_iter()
            .map(|index| comments[index].text.clone())
            .collect(),
    )
}

pub fn parse_rbs_shorthand(text: &str) -> Option<RbsShorthandResult> {
    parse_rbs_shorthand_with_aliases(text, &HashMap::new(), None)
}

pub fn parse_rbs_return_annotation_with_aliases(
    text: &str,
    type_aliases: &HashMap<String, Type>,
    current_scope: Option<&str>,
) -> Option<Type> {
    let body = text.strip_prefix("#")?.trim_start();
    let rest = body.strip_prefix("@rbs")?.trim_start();
    let return_type = rest.strip_prefix("return:")?.trim_start();
    let return_type = return_type
        .split_once("--")
        .map(|(ty, _comment)| ty)
        .unwrap_or(return_type)
        .trim();
    if return_type.is_empty() {
        return None;
    }
    let rbs_type = rbs_ir::RbsType::from(&rbs_sys::parse_type(return_type).ok()?);
    Some(convert_imported_rbs_type(
        &rbs_type,
        type_aliases,
        current_scope,
    ))
}

pub fn parse_rbs_inline_type_annotation_with_aliases(
    text: &str,
    type_aliases: &HashMap<String, Type>,
    current_scope: Option<&str>,
) -> Option<Type> {
    let body = text.strip_prefix("#:")?.trim();
    if body.is_empty() || body.starts_with("type ") || body.starts_with("as ") {
        return None;
    }
    if matches!(body, "absurd" | "!nil") {
        return None;
    }
    let type_str = strip_rbs_comment_tail(body);
    if type_str.is_empty() {
        return None;
    }
    let rbs_type = rbs_ir::RbsType::from(&rbs_sys::parse_type(type_str).ok()?);
    Some(convert_imported_rbs_type(
        &rbs_type,
        type_aliases,
        current_scope,
    ))
}

pub fn parse_rbs_param_annotation_with_aliases(
    text: &str,
    type_aliases: &HashMap<String, Type>,
    current_scope: Option<&str>,
) -> Option<RbsParamAnnotation> {
    let body = text.strip_prefix("#")?.trim_start();
    let rest = body.strip_prefix("@rbs")?.trim_start();
    if rest.starts_with("return:") {
        return None;
    }
    let (raw_name, raw_type) = rest.split_once(':')?;
    let (name, is_block) = parse_rbs_param_annotation_name(raw_name)?;
    let raw_type = strip_rbs_comment_tail(raw_type.trim());
    if raw_type.is_empty() {
        return None;
    }

    if is_block {
        let method_type = rbs_ir::MethodType::from(&rbs_sys::parse_method_type(raw_type).ok()?);
        let function_type = &method_type.function_type;
        let block_param_types = function_type
            .required_positionals
            .iter()
            .chain(function_type.optional_positionals.iter())
            .chain(function_type.trailing_positionals.iter())
            .map(|param| convert_imported_rbs_type(&param.type_, type_aliases, current_scope))
            .collect::<Vec<_>>();
        let return_type =
            convert_imported_rbs_type(&function_type.return_type, type_aliases, current_scope);
        let ty = Type::Proc {
            return_type: Box::new(return_type.clone()),
            param_count: rbs_function_type_param_count(function_type),
        };
        return Some(RbsParamAnnotation {
            name,
            ty,
            is_block,
            block_param_types,
            block_return_type: Some(return_type),
        });
    }

    let rbs_type = rbs_ir::RbsType::from(&rbs_sys::parse_type(raw_type).ok()?);
    Some(RbsParamAnnotation {
        name,
        ty: convert_imported_rbs_type(&rbs_type, type_aliases, current_scope),
        is_block,
        block_param_types: Vec::new(),
        block_return_type: None,
    })
}

pub fn parse_rbs_shorthand_with_aliases(
    text: &str,
    type_aliases: &HashMap<String, Type>,
    current_scope: Option<&str>,
) -> Option<RbsShorthandResult> {
    let body = text.strip_prefix("#")?.trim_start();

    let all_method_types =
        rbs_ir::method_types_from_rbs(&rbs_sys::parse_inline_all_overloads(body).ok()?);
    if all_method_types.is_empty() {
        return None;
    }

    let method_types =
        resolve_imported_method_types_aliases(&all_method_types, type_aliases, current_scope);
    let first = &method_types[0];
    let param_types =
        convert_function_params_with_aliases(&first.function_type, type_aliases, current_scope);
    let return_type = convert_imported_rbs_type(
        &first.function_type.return_type,
        type_aliases,
        current_scope,
    );
    let block_type = first.block.as_ref().map(|block| {
        let function_type = &block.function_type;
        let param_count = function_type.required_positionals.len()
            + function_type.optional_positionals.len()
            + function_type.required_keywords.len()
            + function_type.optional_keywords.len();
        let ret =
            convert_imported_rbs_type(&function_type.return_type, type_aliases, current_scope);
        Type::Proc {
            return_type: Box::new(ret),
            param_count,
        }
    });

    Some(RbsShorthandResult {
        param_types,
        return_type,
        method_types,
        block_type,
    })
}

fn convert_function_params_with_aliases(
    ft: &rbs_ir::FunctionType,
    type_aliases: &HashMap<String, Type>,
    current_scope: Option<&str>,
) -> Vec<(Type, ParamKind)> {
    let mut params = Vec::new();
    for p in &ft.required_positionals {
        params.push((
            convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            ParamKind::Required,
        ));
    }
    for p in &ft.optional_positionals {
        params.push((
            convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            ParamKind::Optional,
        ));
    }
    if let Some(ref p) = ft.rest_positionals {
        params.push((
            convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            ParamKind::Rest,
        ));
    }
    for p in &ft.trailing_positionals {
        params.push((
            convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            ParamKind::Required,
        ));
    }
    for (_, p) in &ft.required_keywords {
        params.push((
            convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            ParamKind::KeywordRequired,
        ));
    }
    for (_, p) in &ft.optional_keywords {
        params.push((
            convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            ParamKind::KeywordOptional,
        ));
    }
    if let Some(ref p) = ft.rest_keywords {
        params.push((
            convert_imported_rbs_type(&p.type_, type_aliases, current_scope),
            ParamKind::DoubleRest,
        ));
    }
    params
}

fn strip_rbs_comment_tail(type_str: &str) -> &str {
    type_str
        .split_once("--")
        .map(|(ty, _comment)| ty)
        .unwrap_or(type_str)
        .trim()
}

fn parse_rbs_param_annotation_name(raw_name: &str) -> Option<(String, bool)> {
    let mut name = raw_name.trim();
    let is_block = if let Some(rest) = name.strip_prefix('&') {
        name = rest.trim_start();
        true
    } else {
        false
    };
    if !is_block {
        name = name
            .strip_prefix("**")
            .or_else(|| name.strip_prefix('*'))
            .unwrap_or(name);
    }
    let name = name.trim();
    if name.is_empty()
        || name.starts_with('@')
        || name.starts_with('$')
        || name.contains('.')
        || name.contains("::")
        || name == "return"
    {
        return None;
    }
    if !name
        .chars()
        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return None;
    }
    Some((name.to_string(), is_block))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_standard_rbs_inline_comments() {
        let parse_result = ruby_prism::parse(
            b"#: () -> String\n# @rbs (Integer) -> String\n#| Integer\n# @override\ndef foo\nend\n",
        );

        let comments = extract_rbs_inline_comments(&parse_result);
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].text, "#: () -> String");
        assert_eq!(comments[1].text, "# @rbs (Integer) -> String");
    }

    #[test]
    fn finds_adjacent_standard_rbs_overloads() {
        let source = b"#: () -> String\n# @rbs (Integer) -> Integer\ndef foo(x = 1)\nend\n";
        let parse_result = ruby_prism::parse(source);
        let comments = extract_rbs_inline_comments(&parse_result);
        let program = parse_result.node().as_program_node().expect("program");
        let def = program.statements().body().iter().next().expect("def node");
        let def_start = def.location().start_offset();

        assert_eq!(
            find_rbs_inline_comment(&comments, def_start, source),
            Some(vec![
                "#: () -> String".to_string(),
                "# @rbs (Integer) -> Integer".to_string(),
            ])
        );
    }

    #[test]
    fn parses_standard_atrbs_method_type() {
        let shorthand =
            parse_rbs_shorthand("# @rbs (String, Integer) -> bool").expect("@rbs shorthand");

        assert_eq!(shorthand.param_types.len(), 2);
        assert_eq!(shorthand.return_type, Type::Bool);
    }

    #[test]
    fn parses_empty_tuple_rbs_comment_variants() {
        let compact = parse_rbs_shorthand("#: -> []").expect("compact empty tuple");
        let canonical = parse_rbs_shorthand("#: -> [ ]").expect("canonical empty tuple");

        assert_eq!(compact.return_type, Type::Tuple(Vec::new()));
        assert_eq!(canonical.return_type, Type::Tuple(Vec::new()));
    }

    #[test]
    fn parses_standard_atrbs_return_annotation() {
        let return_type = parse_rbs_return_annotation_with_aliases(
            "# @rbs return: String? -- maybe present",
            &HashMap::new(),
            None,
        )
        .expect("@rbs return");

        assert_eq!(return_type.to_string(), "String?");
    }

    #[test]
    fn parses_standard_atrbs_param_annotation() {
        let annotation =
            parse_rbs_param_annotation_with_aliases("# @rbs name: String", &HashMap::new(), None)
                .expect("@rbs param");

        assert_eq!(annotation.name, "name");
        assert_eq!(annotation.ty, Type::String);
        assert!(!annotation.is_block);
    }

    #[test]
    fn parses_standard_atrbs_block_annotation() {
        let annotation = parse_rbs_param_annotation_with_aliases(
            "# @rbs &block: (String) -> void",
            &HashMap::new(),
            None,
        )
        .expect("@rbs block");

        assert_eq!(annotation.name, "block");
        assert!(annotation.is_block);
        assert_eq!(annotation.block_param_types, vec![Type::String]);
        assert_eq!(annotation.block_return_type, Some(Type::Void));
    }

    #[test]
    fn parses_inline_return_type_annotation() {
        let return_type = parse_rbs_inline_type_annotation_with_aliases(
            "#: Array[String] -- names",
            &HashMap::new(),
            None,
        )
        .expect("#: return");

        assert_eq!(return_type.to_string(), "Array[String]");
    }

    #[test]
    fn blank_line_breaks_standard_rbs_comment_attachment() {
        let source = b"#: () -> Integer\n\ndef foo\nend\n";
        let parse_result = ruby_prism::parse(source);
        let comments = extract_rbs_inline_comments(&parse_result);
        let program = parse_result.node().as_program_node().expect("program");
        let def = program.statements().body().iter().next().expect("def node");
        let def_start = def.location().start_offset();

        assert_eq!(find_rbs_inline_comment(&comments, def_start, source), None);
    }

    #[test]
    fn extracts_sorbet_inline_assertion_comments() {
        let source = b"x = 1 #: Integer\ny = x #: as String\nz = y #: as !nil\nw = z #: absurd\n";
        let parse_result = ruby_prism::parse(source);

        let comments = extract_inline_assertion_comments(&parse_result);

        assert_eq!(
            comments
                .iter()
                .map(|comment| comment.assertion.clone())
                .collect::<Vec<_>>(),
            vec![
                InlineAssertion::Explicit(Type::Integer),
                InlineAssertion::Explicit(Type::String),
                InlineAssertion::NonNil,
                InlineAssertion::Explicit(Type::Bot),
            ]
        );
    }

    #[test]
    fn finds_same_line_inline_assertion_for_node() {
        let source = b"x = foo #: as Integer\n";
        let parse_result = ruby_prism::parse(source);
        let comments = extract_inline_assertion_comments(&parse_result);
        let program = parse_result.node().as_program_node().expect("program");
        let write = program
            .statements()
            .body()
            .iter()
            .next()
            .expect("write node");

        assert_eq!(
            find_inline_assertion_for_node(
                &comments,
                source,
                write.location().start_offset(),
                write.location().end_offset()
            ),
            Some(&InlineAssertion::Explicit(Type::Integer))
        );
    }

    #[test]
    fn finds_heredoc_inline_assertion_on_first_line() {
        let source = b"x = <<~MSG #: Integer\n  hello\nMSG\n";
        let parse_result = ruby_prism::parse(source);
        let comments = extract_inline_assertion_comments(&parse_result);
        let program = parse_result.node().as_program_node().expect("program");
        let write = program
            .statements()
            .body()
            .iter()
            .next()
            .expect("write node");

        assert_eq!(
            find_inline_assertion_for_node(
                &comments,
                source,
                write.location().start_offset(),
                write.location().end_offset()
            ),
            Some(&InlineAssertion::Explicit(Type::Integer))
        );
    }
}
