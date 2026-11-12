use std::path::PathBuf;

use tyda::analysis::{PlaygroundCompletion, playground_complete};
use tyda::completion::CompletionEntryKind;
use tyda::rbs::stdlib_loader::LazyRbsLoader;

const CARET: &str = "<caret>";

fn loader() -> LazyRbsLoader {
    let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
    LazyRbsLoader::new(core_dir)
}

/// Completes at the `<caret>` marker, which is removed from the source.
fn complete(marked: &str, rbs: &str) -> Option<PlaygroundCompletion> {
    let offset = marked.find(CARET).expect("missing cursor marker");
    let source = marked.replacen(CARET, "", 1);
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
    let completion = complete("def ids = [1, 2, 3]\nids.<caret>\n", "").expect("completion");
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
    let completion = complete("name = \"tyda\"\nname.<caret>\n", "").expect("completion");
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
    let completion = complete("name = \"tyda\"\nname.up<caret>\n", "").expect("completion");
    assert!(labels(&completion).contains(&"upcase"));
    assert_eq!((completion.start_line, completion.start_column), (1, 5));
    assert_eq!((completion.end_line, completion.end_column), (1, 7));
}

#[test]
fn completes_methods_declared_in_user_rbs() {
    let completion = complete(
        "class User\nend\n\nUser.new.<caret>\n",
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
        "module Shapes\n  class Circle; end\n  class Square; end\nend\n\nShapes::<caret>\n",
        "",
    )
    .expect("completion");
    let labels = labels(&completion);
    assert!(labels.contains(&"Circle"), "{labels:?}");
    assert!(labels.contains(&"Square"), "{labels:?}");
}

#[test]
fn counts_columns_in_utf16_units() {
    let completion = complete("s = \"日本🍣\"; s.<caret>\n", "").expect("completion");
    assert!(labels(&completion).contains(&"upcase"));
    assert_eq!(
        completion.start_column,
        "s = \"日本🍣\"; s.".encode_utf16().count() as u32
    );
}

#[test]
fn returns_none_outside_completion_sites() {
    assert!(complete("x = 1<caret>\n", "").is_none());
    assert!(playground_complete("x = 1\n", "", &loader(), "stdin.rb", 5, 0).is_none());
}

fn item<'a>(
    completion: &'a PlaygroundCompletion,
    label: &str,
) -> &'a tyda::completion::CompletionEntry {
    completion
        .items
        .iter()
        .find(|item| item.label == label)
        .unwrap_or_else(|| panic!("missing {label}: {:?}", labels(completion)))
}

const USER: &str = "class User\n  def initialize(name)\n    @name = name\n  end\n\n  def foo = 1\n\n  private def secret = 2\nend\n";

#[test]
fn completes_methods_on_a_call_with_arguments_or_a_block() {
    let completion = complete(&format!("{USER}User.new(1).<caret>\n"), "").expect("completion");
    assert_eq!(item(&completion, "foo").detail, "User#foo : -> 1");

    let completion = complete("[1].map { |v| v.to_s }.<caret>\n", "").expect("completion");
    assert!(item(&completion, "join").detail.starts_with("Array#join"));

    let completion = complete("[1].map do |v|\n  v\nend.<caret>\n", "").expect("completion");
    assert!(labels(&completion).contains(&"join"));

    let completion = complete("s = \"a\"\ns&.up<caret>\n", "").expect("completion");
    assert!(labels(&completion).contains(&"upcase"));
}

#[test]
fn hides_private_methods_from_an_explicit_receiver() {
    let completion = complete(&format!("{USER}User.new.<caret>\n"), "").expect("completion");
    let labels = labels(&completion);
    for private in ["secret", "initialize", "puts", "require"] {
        assert!(!labels.contains(&private), "{private}: {labels:?}");
    }
    assert!(labels.contains(&"foo"), "{labels:?}");

    let completion = complete(&format!("{USER}User.<caret>\n"), "").expect("completion");
    assert!(!labels_of(&completion).contains(&"initialize"));
    assert!(labels_of(&completion).contains(&"new"));
}

#[test]
fn lists_private_methods_through_self_and_send() {
    let source =
        "class User\n  def call\n    self.<caret>\n  end\n\n  private def secret = 2\nend\n";
    let completion = complete(source, "").expect("completion");
    assert!(labels(&completion).contains(&"secret"));

    let completion =
        complete(&format!("{USER}User.new.send(:se<caret>\n"), "").expect("completion");
    assert!(labels(&completion).contains(&"secret"));

    let completion =
        complete(&format!("{USER}User.new.public_send(:se<caret>\n"), "").expect("completion");
    assert!(!labels(&completion).contains(&"secret"));
}

#[test]
fn completes_module_functions_without_class_methods() {
    let completion = complete("Math.<caret>\n", "").expect("completion");
    let labels = labels(&completion);
    assert!(labels.contains(&"sqrt"), "{labels:?}");
    assert!(labels.contains(&"const_get"), "{labels:?}");
    for hidden in ["new", "superclass", "module_function"] {
        assert!(!labels.contains(&hidden), "{hidden}: {labels:?}");
    }
}

#[test]
fn hides_methods_declared_private_in_rbs() {
    let rbs = "class Service\n  def self.build: () -> Service\n  def run: () -> void\n  alias start run\n\n  private\n\n  def helper: () -> void\n  attr_reader cache: Integer\n  alias assist helper\n  def self.cached: () -> Service\n  public def open: () -> void\n\n  public\n\n  def close: () -> void\nend\n";
    let completion = complete("Service.build.<caret>\n", rbs).expect("completion");
    let names = labels(&completion);
    for shown in ["run", "start", "open", "close"] {
        assert!(names.contains(&shown), "{shown}: {names:?}");
    }
    for hidden in ["helper", "cache", "assist", "raise", "fail"] {
        assert!(!names.contains(&hidden), "{hidden}: {names:?}");
    }

    let completion = complete("Service.<caret>\n", rbs).expect("completion");
    let names = labels(&completion);
    for shown in ["build", "cached", "new"] {
        assert!(names.contains(&shown), "{shown}: {names:?}");
    }
    for hidden in ["inherited", "module_function", "raise"] {
        assert!(!names.contains(&hidden), "{hidden}: {names:?}");
    }

    let source = "class Service\n  def call\n    self.<caret>\n  end\nend\n";
    let completion = complete(source, rbs).expect("completion");
    let names = labels(&completion);
    for shown in ["helper", "assist", "run"] {
        assert!(names.contains(&shown), "{shown}: {names:?}");
    }
}

#[test]
fn completes_constants_declared_in_rbs_after_double_colon() {
    let completion = complete("Math::<caret>\n", "").expect("completion");
    assert_eq!(item(&completion, "PI").detail, "Math::PI : Float");
    let completion = complete("File::SEP<caret>\n", "").expect("completion");
    assert!(labels(&completion).contains(&"SEPARATOR"));
}

#[test]
fn completes_locals_and_methods_of_self_for_a_bare_identifier() {
    let source = "class User\n  def foo = 1\n\n  def call(force)\n    found = 1\n    [1].each { |fruit| f<caret> }\n  end\n\n  private def format_name = 1\nend\n";
    let completion = complete(source, "").expect("completion");
    let labels = labels(&completion);
    assert_eq!(&labels[..3], &["force", "found", "fruit"], "{labels:?}");
    assert_eq!(
        item(&completion, "found").kind,
        CompletionEntryKind::Variable
    );
    assert_eq!(item(&completion, "foo").detail, "User#foo : -> 1");
    assert!(labels.contains(&"format_name"), "{labels:?}");
    assert_eq!(item(&completion, "for").kind, CompletionEntryKind::Keyword);
    assert_eq!((completion.start_line, completion.start_column), (5, 23));

    let completion = complete("pu<caret>\n", "").expect("completion");
    assert_eq!(item(&completion, "puts").owner.as_deref(), Some("Kernel"));

    let completion = complete("class User\n  att<caret>\nend\n", "").expect("completion");
    assert!(labels_of(&completion).contains(&"attr_reader"));
}

#[test]
fn completes_constants_from_code_and_rbs_for_a_bare_identifier() {
    let source = "module App\n  Limit = 3\n  class User\n    def call = Li<caret>\n  end\nend\n";
    let completion = complete(source, "").expect("completion");
    assert_eq!(item(&completion, "Limit").detail, "App::Limit : 3");

    let completion = complete("Fl<caret>\n", "").expect("completion");
    let float = completion
        .items
        .iter()
        .find(|item| item.label == "Float" && item.kind == CompletionEntryKind::Class)
        .expect("Float class");
    assert_eq!(float.detail, "class Float");
    assert_eq!(item(&completion, "Float").kind, CompletionEntryKind::Class);

    let completion = complete("AR<caret>\n", "").expect("completion");
    assert_eq!(item(&completion, "ARGV").detail, "ARGV : Array[String]");

    let completion = complete(&format!("{USER}Us<caret>\n"), "").expect("completion");
    assert_eq!(item(&completion, "User").kind, CompletionEntryKind::Class);
}

#[test]
fn skips_bare_identifiers_in_comments_strings_and_def_names() {
    assert!(complete("# pu<caret>\n", "").is_none());
    assert!(complete("x = \"pu<caret>\"\n", "").is_none());
    assert!(complete("def pu<caret>\nend\n", "").is_none());
    assert!(complete("@na<caret>\n", "").is_none());
}

fn labels_of(completion: &PlaygroundCompletion) -> Vec<&str> {
    labels(completion)
}
