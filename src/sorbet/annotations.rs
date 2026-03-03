use std::collections::HashMap;
use std::path::Path;

use ruby_prism::{Node, ParseResult};

use crate::project_markers::{ProjectMarker, has_project_marker_in_ancestors};
use crate::rbs::import::resolve_type_aliases;
use crate::rbs::inline::{RbsComment, extract_rbs_inline_comments, parse_rbs_shorthand};
use crate::types::Type;

pub fn sorbet_comment_mode(project_root: Option<&Path>, file_path: Option<&str>) -> bool {
    project_root.is_some_and(has_sorbet_config_in_ancestors)
        || file_path
            .map(Path::new)
            .is_some_and(has_sorbet_config_in_ancestors)
}

fn has_sorbet_config_in_ancestors(path: &Path) -> bool {
    has_project_marker_in_ancestors(path, ProjectMarker::SorbetConfig)
}

pub fn extract_annotation_comments(
    parse_result: &ParseResult<'_>,
    sorbet_mode: bool,
) -> Vec<RbsComment> {
    if sorbet_mode {
        super::comments::extract_sorbet_rbs_comments(parse_result)
    } else {
        extract_rbs_inline_comments(parse_result)
    }
}

pub fn find_method_annotations(
    comments: &[RbsComment],
    def_start_offset: usize,
    source: &[u8],
) -> Option<Vec<String>> {
    super::comments::find_sorbet_rbs_comment(comments, def_start_offset, source)
}

pub fn extract_sig_source(node: &Node<'_>) -> Option<String> {
    let call_node = node.as_call_node()?;
    if call_node.name().as_slice() == b"sig" && call_node.block().is_some() {
        Some(String::from_utf8_lossy(node.location().as_slice()).to_string())
    } else {
        None
    }
}

pub fn sig_sources_to_rbs_lines(sig_sources: &[String]) -> Vec<String> {
    sig_sources
        .iter()
        .filter_map(|sig_source| super::sig::sig_source_to_rbs(sig_source))
        .map(|rbs| format!("#: {rbs}"))
        .collect()
}

pub fn sig_source_return_type(sig_source: &str) -> Option<Type> {
    let rbs = super::sig::sig_source_to_rbs(sig_source)?;
    let rbs_str = format!("#: () -> {rbs}");
    parse_rbs_shorthand(&rbs_str).map(|result| result.return_type)
}

pub fn extract_sorbet_comment_type_aliases(
    source: &str,
    existing_aliases: &HashMap<String, Type>,
) -> HashMap<String, Type> {
    let raw_aliases = extract_sorbet_raw_type_aliases(source);
    resolve_type_aliases(&raw_aliases, existing_aliases)
}

fn extract_sorbet_raw_type_aliases(source: &str) -> HashMap<String, crate::rbs::ir::RbsType> {
    let mut aliases = HashMap::new();
    let mut scope_stack: Vec<String> = Vec::new();
    let mut pending: Option<(String, String)> = None;

    for line in source.lines() {
        let trimmed = line.trim();

        if let Some((_qualified_name, rhs)) = pending.as_mut() {
            if let Some(rest) = trimmed.strip_prefix("#|") {
                let continuation = rest.trim();
                if !continuation.is_empty() {
                    if !rhs.is_empty() {
                        rhs.push(' ');
                    }
                    rhs.push_str(continuation);
                }
                continue;
            }

            finalize_pending_alias(&mut aliases, pending.take());
        }

        if let Some(rest) = trimmed.strip_prefix("#:") {
            let body = rest.trim_start();
            if let Some(rest) = body.strip_prefix("type ")
                && let Some((lhs, rhs)) = rest.split_once('=')
            {
                let alias_name = lhs.trim().split('[').next().unwrap_or("").trim();
                if alias_name.is_empty() {
                    continue;
                }
                let qualified_name =
                    qualify_decl_name(alias_name, scope_stack.last().map(String::as_str));
                let rhs = rhs.trim();
                if rhs.is_empty() {
                    pending = Some((qualified_name, String::new()));
                } else {
                    finalize_pending_alias(&mut aliases, Some((qualified_name, rhs.to_string())));
                }
            }
            continue;
        }

        let code = line.split('#').next().unwrap_or("").trim();
        if code.is_empty() {
            continue;
        }

        if let Some(rest) = code.strip_prefix("class ") {
            if !rest.starts_with("<<") {
                let raw_name = rest.split(['<', ' ']).next().unwrap_or("").trim();
                if !raw_name.is_empty() {
                    let qualified =
                        qualify_decl_name(raw_name, scope_stack.last().map(String::as_str));
                    scope_stack.push(qualified);
                }
            }
            continue;
        }

        if let Some(rest) = code.strip_prefix("module ") {
            let raw_name = rest.split(' ').next().unwrap_or("").trim();
            if !raw_name.is_empty() {
                let qualified = qualify_decl_name(raw_name, scope_stack.last().map(String::as_str));
                scope_stack.push(qualified);
            }
            continue;
        }

        if code == "end" {
            scope_stack.pop();
        }
    }

    finalize_pending_alias(&mut aliases, pending.take());
    aliases
}

fn finalize_pending_alias(
    aliases: &mut HashMap<String, crate::rbs::ir::RbsType>,
    pending: Option<(String, String)>,
) {
    let Some((qualified_name, rhs)) = pending else {
        return;
    };
    let rhs = rhs.trim();
    if rhs.is_empty() {
        return;
    }
    if let Ok(rbs_type) = rbs_sys::parse_type(rhs) {
        aliases.insert(qualified_name, crate::rbs::ir::RbsType::from(&rbs_type));
    }
}

fn qualify_decl_name(name: &str, current_scope: Option<&str>) -> String {
    let bare = name.trim().trim_start_matches("::");
    match current_scope {
        Some(scope) if !bare.contains("::") => format!("{scope}::{bare}"),
        _ => bare.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_markers::{ProjectMarker, clear_project_marker_cache};

    fn reset_sorbet_cache() {
        clear_project_marker_cache(ProjectMarker::SorbetConfig);
    }

    #[test]
    fn extracts_sig_source_from_sig_call() {
        let parse_result = ruby_prism::parse(b"sig { returns(String) }\ndef name; end\n");
        let program = parse_result.node().as_program_node().expect("program");
        let sig_node = program.statements().body().iter().next().expect("sig node");

        assert_eq!(
            extract_sig_source(&sig_node),
            Some("sig { returns(String) }".to_string())
        );
    }

    #[test]
    fn converts_sig_sources_to_rbs_lines() {
        assert_eq!(
            sig_sources_to_rbs_lines(&["sig { returns(String) }".to_string()]),
            vec!["#: -> String".to_string()]
        );
    }

    #[test]
    fn detects_sorbet_comment_mode_from_project_root() {
        reset_sorbet_cache();
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("sorbet")).expect("mkdir sorbet");
        std::fs::write(dir.path().join("sorbet/config"), ".\n").expect("write sorbet/config");

        assert!(sorbet_comment_mode(Some(dir.path()), None));
    }

    #[test]
    fn detects_sorbet_comment_mode_from_file_path_ancestor() {
        reset_sorbet_cache();
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("sorbet")).expect("mkdir sorbet");
        std::fs::create_dir_all(dir.path().join("app/models")).expect("mkdir app/models");
        std::fs::write(dir.path().join("sorbet/config"), ".\n").expect("write sorbet/config");

        let file_path = dir.path().join("app/models/user.rb");
        assert!(sorbet_comment_mode(
            None,
            Some(file_path.to_str().expect("utf8 path"))
        ));
    }

    #[test]
    fn does_not_detect_sorbet_comment_mode_without_sorbet_config() {
        reset_sorbet_cache();
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("app/models")).expect("mkdir app/models");
        let file_path = dir.path().join("app/models/user.rb");

        assert!(!sorbet_comment_mode(
            Some(dir.path()),
            Some(file_path.to_str().expect("utf8 path"))
        ));
    }

    #[test]
    fn standard_mode_uses_only_standard_rbs_comments() {
        let parse_result = ruby_prism::parse(
            b"#: () -> String\n# @rbs () -> Symbol\n#| Integer\n# @override\ndef foo\nend\n",
        );
        let comments = extract_annotation_comments(&parse_result, false);
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].text, "#: () -> String");
        assert_eq!(comments[1].text, "# @rbs () -> Symbol");
    }

    #[test]
    fn sorbet_mode_does_not_use_standard_atrbs_comments() {
        let parse_result = ruby_prism::parse(
            b"#: () -> String\n# @rbs () -> Symbol\n#| Integer\n# @override\ndef foo\nend\n",
        );
        let comments = extract_annotation_comments(&parse_result, true);
        assert_eq!(
            comments
                .iter()
                .map(|comment| comment.text.as_str())
                .collect::<Vec<_>>(),
            vec!["#: () -> String", "#| Integer", "# @override"]
        );
    }

    #[test]
    fn extracts_multiline_scoped_sorbet_type_aliases() {
        let source = "\
#: type top_alias = Integer

class Box
  #: type elemish =
  #| Integer |
  #| String

  class Inner
    #: type nested = elemish
  end
end
";
        let aliases = extract_sorbet_comment_type_aliases(source, &HashMap::new());

        assert_eq!(aliases.get("top_alias"), Some(&Type::Integer));
        assert_eq!(
            aliases.get("Box::elemish"),
            Some(&Type::Union(vec![Type::Integer, Type::String]))
        );
        assert_eq!(
            aliases.get("Box::Inner::nested"),
            Some(&Type::Union(vec![Type::Integer, Type::String]))
        );
    }
}
