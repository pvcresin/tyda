use std::path::PathBuf;

use tyda::analysis::{PlaygroundCompletion, playground_complete};
use tyda::completion::CompletionEntryKind;
use tyda::rbs::stdlib_loader::LazyRbsLoader;

fn loader() -> LazyRbsLoader {
    let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
    LazyRbsLoader::new(core_dir)
}

/// Completes at the `|` marker, which is removed from the source.
fn complete(marked: &str, rbs: &str) -> Option<PlaygroundCompletion> {
    let offset = marked.find('|').expect("missing cursor marker");
    let source = marked.replacen('|', "", 1);
    let before = &source[..offset];
    let line = before.matches('\n').count();
    let column = before
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .encode_utf16()
        .count();
    playground_complete(&source, rbs, &loader(), "stdin.rb", line, column)
}

fn labels(completion: &PlaygroundCompletion) -> Vec<&str> {
    completion
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect()
}

#[test]
fn completes_methods_of_inferred_receiver_after_dot() {
    let completion = complete("def ids = [1, 2, 3]\nids.|\n", "").expect("completion");
    let labels = labels(&completion);
    assert!(labels.contains(&"map"), "{labels:?}");
    assert!(labels.contains(&"each"), "{labels:?}");
    let map = completion
        .items
        .iter()
        .find(|item| item.label == "map")
        .unwrap();
    assert_eq!(map.kind, CompletionEntryKind::Method);
    assert!(map.detail.starts_with("Array#map : "), "{}", map.detail);
    assert_eq!((completion.start_line, completion.start_column), (1, 4));
    assert_eq!((completion.end_line, completion.end_column), (1, 4));
}

#[test]
fn lists_receiver_methods_before_inherited_and_operator_methods() {
    let completion = complete("name = \"tyda\"\nname.|\n", "").expect("completion");
    let first = &completion.items[0];
    assert_eq!(first.owner.as_deref(), Some("String"));
    assert!(
        first
            .label
            .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
    );
    let owners: Vec<&str> = completion
        .items
        .iter()
        .filter_map(|item| item.owner.as_deref())
        .collect();
    let last_string = owners.iter().rposition(|owner| *owner == "String").unwrap();
    let first_kernel = owners.iter().position(|owner| *owner == "Kernel").unwrap();
    assert!(last_string < first_kernel, "{owners:?}");
}

#[test]
fn replaces_partial_method_name_after_dot() {
    let completion = complete("name = \"tyda\"\nname.up|\n", "").expect("completion");
    assert!(labels(&completion).contains(&"upcase"));
    assert_eq!((completion.start_line, completion.start_column), (1, 5));
    assert_eq!((completion.end_line, completion.end_column), (1, 7));
}

#[test]
fn completes_methods_declared_in_user_rbs() {
    let completion = complete(
        "class User\nend\n\nUser.new.|\n",
        "class User\n  def name: () -> String\nend\n",
    )
    .expect("completion");
    let name = completion
        .items
        .iter()
        .find(|item| item.label == "name")
        .expect("name from RBS");
    assert_eq!(name.detail, "User#name : -> String");
}

#[test]
fn completes_nested_constants_after_double_colon() {
    let completion = complete(
        "module Shapes\n  class Circle; end\n  class Square; end\nend\n\nShapes::|\n",
        "",
    )
    .expect("completion");
    let labels = labels(&completion);
    assert!(labels.contains(&"Circle"), "{labels:?}");
    assert!(labels.contains(&"Square"), "{labels:?}");
}

#[test]
fn counts_columns_in_utf16_units() {
    let completion = complete("s = \"日本🍣\"; s.|\n", "").expect("completion");
    assert!(labels(&completion).contains(&"upcase"));
    assert_eq!(
        completion.start_column,
        "s = \"日本🍣\"; s.".encode_utf16().count() as u32
    );
}

#[test]
fn returns_none_outside_completion_sites() {
    assert!(complete("x = 1|\n", "").is_none());
    assert!(playground_complete("x = 1\n", "", &loader(), "stdin.rb", 5, 0).is_none());
}
