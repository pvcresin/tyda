use std::path::PathBuf;

use tyda::analysis::analyze_source_with_lazy_rbs;
use tyda::rbs::render::{RenderOptions, render_rbs_with_options};
use tyda::rbs::stdlib_loader::LazyRbsLoader;

fn render(source: &str) -> String {
    let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
    let loader = LazyRbsLoader::new(core_dir);
    let registry = analyze_source_with_lazy_rbs(source, None, &loader);
    render_rbs_with_options(&registry, RenderOptions::default())
}

fn assert_rbs_contains(source: &str, expected: &str) {
    let actual = render(source);
    assert!(
        actual.contains(expected),
        "expected RBS to contain:\n{expected}\n\nactual RBS:\n{actual}\n\nsource:\n{source}"
    );
}

fn assert_rbs_not_contains(source: &str, unexpected: &str) {
    let actual = render(source);
    assert!(
        !actual.contains(unexpected),
        "expected RBS not to contain:\n{unexpected}\n\nactual RBS:\n{actual}\n\nsource:\n{source}"
    );
}

#[test]
fn late_constant_declarations_refresh_methods_across_class_body_branches() {
    let declarations = [
        r#"VALUE = "late""#,
        r#"if true
    VALUE = "late"
  end"#,
        r#"unless false
    VALUE = "late"
  end"#,
        r#"if false
    OTHER = "other"
  else
    VALUE = "late"
  end"#,
        r#"begin
    VALUE = "late"
  end"#,
        r#"begin
    OTHER = "other"
  rescue StandardError
    VALUE = "late"
  end"#,
        r#"begin
    OTHER = "other"
  ensure
    VALUE = "late"
  end"#,
        r#"case :value
  when :value
    VALUE = "late"
  end"#,
        r#"case [1]
  in [1]
    VALUE = "late"
  end"#,
        r#"if true
    begin
      VALUE = "late"
    end
  end"#,
    ];

    for declaration in declarations {
        let source = format!(
            r#"class Box
  def value
    VALUE
  end

  {declaration}
end
"#
        );
        assert_rbs_contains(&source, r#"VALUE: "late""#);
        assert_rbs_contains(&source, r#"def value: -> "late""#);
    }
}

#[test]
fn late_multi_write_targets_refresh_methods() {
    let class_body_declarations = [
        (r#"VALUE, OTHER = ["direct", "other"]"#, "direct"),
        (r#"(VALUE, OTHER) = ["nested", "other"]"#, "nested"),
        (
            r#"GROUP, (VALUE, OTHER) = ["group", ["inner", "other"]]"#,
            "inner",
        ),
        (r#"*OTHER, VALUE = ["other", "right"]"#, "right"),
    ];

    for (declaration, expected) in class_body_declarations {
        let source = format!(
            r#"class Box
  def value
    VALUE
  end

  {declaration}
end
"#
        );
        assert_rbs_contains(&source, &format!(r#"VALUE: "{expected}""#));
        assert_rbs_contains(&source, &format!(r#"def value: -> "{expected}""#));
    }

    assert_rbs_contains(
        r#"class Box
  def value
    VALUE
  end
end

Box::VALUE, Box::OTHER = ["path", "other"]
"#,
        r#"def value: -> "path""#,
    );
}

#[test]
fn static_const_set_refreshes_methods_without_dynamic_name_guessing() {
    let class_body_declarations = [
        (r#"const_set(:VALUE, "symbol")"#, "symbol"),
        (r#"const_set("VALUE", "string")"#, "string"),
        (r#"send(:const_set, :VALUE, "send")"#, "send"),
        (r#"public_send(:const_set, :VALUE, "public")"#, "public"),
        (r#"self.const_set(:VALUE, "self")"#, "self"),
        (
            r#"value = "local"
  const_set(:VALUE, value)"#,
            "local",
        ),
        (
            r#"value = "branch"
  if true
    const_set(:VALUE, value)
  end"#,
            "branch",
        ),
    ];

    for (declaration, expected) in class_body_declarations {
        let source = format!(
            r#"class Box
  def value
    self.class::VALUE
  end

  {declaration}
end
"#
        );
        assert_rbs_contains(&source, &format!(r#"VALUE: "{expected}""#));
        assert_rbs_contains(&source, &format!(r#"def value: -> "{expected}""#));
    }

    assert_rbs_contains(
        r#"class Box
  def value
    VALUE
  end
end

Box.const_set(:VALUE, "path")
"#,
        r#"def value: -> "path""#,
    );

    assert_rbs_contains(
        r#"class Box
  def value
    VALUE
  end
end

Object.const_set(:VALUE, "top")
"#,
        r#"def value: -> "top""#,
    );

    assert_rbs_not_contains(
        r#"class Box
  const_name = "VALUE"
  const_set(const_name, "dynamic")
end
"#,
        "VALUE:",
    );
}

#[test]
fn branch_scope_keeps_ruby_class_scope_boundaries() {
    assert_rbs_contains(
        r#"class Box
  value = "branch"
  if true
    const_set(:VALUE, value)
  end
end
"#,
        r#"VALUE: "branch""#,
    );

    assert_rbs_not_contains(
        r#"class Box
  value = "outer"
  class << self
    const_set(:VALUE, value)
  end
end
"#,
        r#"VALUE: "outer""#,
    );
}
