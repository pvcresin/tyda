use std::fs;
use std::path::{Path, PathBuf};

const MAX_HEADING_LEN: usize = 90;
const BANNED_HEADING_TERMS: &[&str] = &[
    "TypeProf", "Ruby LSP", "Rubydex", "GitLab", "Mastodon", "upstream", "flow/",
];

#[test]
fn scenario_headings_are_simple_english() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let mut files = Vec::new();
    collect_markdown_files(&root, &mut files);

    let mut failures = Vec::new();

    for path in files {
        let content = fs::read_to_string(&path).expect("scenario file should be readable");
        for (idx, line) in content.lines().enumerate() {
            let Some(heading) = line.strip_prefix("# ").or_else(|| line.strip_prefix("## ")) else {
                continue;
            };

            if !heading.is_ascii() {
                failures.push(format!(
                    "{}:{} uses non-ASCII heading",
                    path.display(),
                    idx + 1
                ));
            }
            if heading.len() > MAX_HEADING_LEN {
                failures.push(format!(
                    "{}:{} heading is longer than {MAX_HEADING_LEN} chars",
                    path.display(),
                    idx + 1
                ));
            }
            for term in BANNED_HEADING_TERMS {
                if heading.contains(term) {
                    failures.push(format!(
                        "{}:{} heading contains source-specific term `{term}`",
                        path.display(),
                        idx + 1
                    ));
                }
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn scenario_markdown_has_no_freeform_prose() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let mut files = Vec::new();
    collect_markdown_files(&root, &mut files);

    let mut failures = Vec::new();

    for path in files {
        let content = fs::read_to_string(&path).expect("scenario file should be readable");
        let mut in_fence = false;

        for (idx, line) in content.lines().enumerate() {
            if line.starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence || line.trim().is_empty() {
                continue;
            }
            if idx == 0 && line.starts_with("# ") {
                continue;
            }
            if line.starts_with("## ") || line.starts_with("### ") {
                continue;
            }
            if line.starts_with('`') && line.ends_with('`') {
                continue;
            }

            failures.push(format!(
                "{}:{} contains prose outside a code block",
                path.display(),
                idx + 1
            ));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn scenario_code_blocks_have_only_semantic_comments() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let mut files = Vec::new();
    collect_markdown_files(&root, &mut files);

    let mut failures = Vec::new();

    for path in files {
        let content = fs::read_to_string(&path).expect("scenario file should be readable");
        let mut in_fence = false;

        for (idx, line) in content.lines().enumerate() {
            if line.starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            if !in_fence {
                continue;
            }

            let trimmed = line.trim_start();
            if !trimmed.starts_with('#') || is_semantic_comment(trimmed) {
                continue;
            }

            failures.push(format!(
                "{}:{} contains non-semantic code comment",
                path.display(),
                idx + 1
            ));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn is_semantic_comment(line: &str) -> bool {
    line.starts_with("#:")
        || line.starts_with("#|")
        || line.starts_with("# @")
        || line.starts_with("# frozen_string_literal:")
        || line.starts_with("# shareable_constant_value:")
        || line.starts_with("# typed:")
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("scenario directory should be readable") {
        let path = entry.expect("scenario entry should be readable").path();
        if path.is_dir() {
            collect_markdown_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}
