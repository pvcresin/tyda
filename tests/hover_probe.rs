//! Hover regression tests: every assertion here pins a pattern where Tyda
//! should statically resolve the hovered name. These mirror realistic
//! patterns seen in rubygems / rack / rake and must not regress.

use std::path::PathBuf;
use tyda::analysis::hover_at;
use tyda::rbs::stdlib_loader::LazyRbsLoader;

fn loader() -> LazyRbsLoader {
    let core_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rbs/core");
    LazyRbsLoader::new(core_dir)
}

fn hover(source: &str, line: usize, column: usize) -> Option<(String, String)> {
    let loader = loader();
    hover_at(source, None, &loader, "probe.rb", line, column).map(|h| (h.name, h.ty.to_string()))
}

fn hover_with_display(
    source: &str,
    line: usize,
    column: usize,
) -> Option<(String, String, Option<String>)> {
    let loader = loader();
    hover_at(source, None, &loader, "probe.rb", line, column).map(|h| {
        (
            h.name,
            h.ty.to_string(),
            h.display_rbs.map(|display| display.to_string()),
        )
    })
}

fn assert_hover(source: &str, line: usize, column: usize, name: &str, ty: &str) {
    let result = hover(source, line, column);
    match result {
        Some((got_name, got_ty)) => {
            assert_eq!(got_name, name, "name mismatch at ({line}:{column})");
            assert_eq!(got_ty, ty, "type mismatch at ({line}:{column})");
        }
        None => panic!("expected hover at ({line}:{column}) for {name}"),
    }
}

fn assert_no_hover(source: &str, line: usize, column: usize) {
    let result = hover(source, line, column);
    assert!(
        result.is_none(),
        "expected no hover at ({line}:{column}), got {result:?}"
    );
}

fn assert_hover_display(
    source: &str,
    line: usize,
    column: usize,
    name: &str,
    ty: &str,
    display: &str,
) {
    let result = hover_with_display(source, line, column);
    match result {
        Some((got_name, got_ty, got_display)) => {
            assert_eq!(got_name, name, "name mismatch at ({line}:{column})");
            assert_eq!(got_ty, ty, "type mismatch at ({line}:{column})");
            assert_eq!(
                got_display.as_deref(),
                Some(display),
                "display mismatch at ({line}:{column})"
            );
        }
        None => panic!("expected hover at ({line}:{column}) for {name}"),
    }
}

#[test]
fn local_variable_literal() {
    let source = "class A\n  def foo\n    x = 42\n    x\n  end\nend\n";
    assert_hover(source, 3, 4, "x", "42");
    assert_hover(source, 4, 4, "x", "42");
}

#[test]
fn local_variable_write_without_read_has_hover() {
    let source = "class A\n  def foo\n    x = 42\n  end\nend\n";
    assert_hover(source, 3, 4, "x", "42");
}

#[test]
fn local_variable_from_method_return() {
    let source = "class Foo\n  def self.make = 42\nend\n\nclass Bar\n  def run\n    x = Foo.make\n    x\n  end\nend\n";
    assert_hover(source, 8, 4, "x", "42");
}

#[test]
fn ivar_read_returns_initialized_type() {
    let source = "class Foo\n  def initialize\n    @x = 42\n  end\n  def get\n    @x\n  end\nend\n";
    assert_hover(source, 6, 4, "@x", "42");
}

#[test]
fn ivar_write_and_initialize_param_have_hover() {
    let source = concat!(
        "class User\n",
        "  #: (String) -> void\n",
        "  def initialize(name)\n",
        "    @name = name\n",
        "  end\n",
        "\n",
        "  def name = @name\n",
        "end\n",
    );
    assert_hover(source, 3, 17, "name", "String");
    assert_hover(source, 4, 4, "@name", "String");
    assert_hover(source, 7, 13, "@name", "String");
    assert_hover_display(source, 7, 6, "name", "String", "-> String");
}

#[test]
fn method_param_hover_from_caller() {
    let source = "class Foo\n  def greet(name)\n    name\n  end\nend\n\nFoo.new.greet(\"hi\")\n";
    // hover over `name` reference inside method body
    assert_hover(source, 3, 4, "name", "String");
}

#[test]
fn constant_definition_hover() {
    let source = "class Foo\n  CONST = 42\nend\n";
    // hover over CONST at line 2 col 2
    assert_hover(source, 2, 2, "CONST", "42");
}

#[test]
fn constant_reference_hover() {
    let source = "class Foo\n  CONST = 42\n  def x = CONST\nend\n";
    // hover over CONST reference inside the method body (line 3 col 12)
    assert_hover(source, 3, 12, "CONST", "42");
}

#[test]
fn nested_constant_path_hover() {
    let source = "module M\n  class C\n    CONST = 1\n  end\nend\n\nM::C::CONST\n";
    assert_hover(source, 7, 0, "M", "singleton(M)");
    assert_no_hover(source, 7, 1);
    assert_no_hover(source, 7, 2);
    assert_hover(source, 7, 3, "M::C", "singleton(M::C)");
    assert_no_hover(source, 7, 4);
    assert_no_hover(source, 7, 5);
    // hover over CONST in the namespaced reference
    assert_hover(source, 7, 6, "M::C::CONST", "1");
}

#[test]
fn class_definition_name_is_singleton() {
    let source = "class Foo\nend\n";
    // hover over "Foo" in class declaration
    assert_hover(source, 1, 6, "Foo", "singleton(Foo)");
}

#[test]
fn class_definition_name_with_namespace() {
    let source = "module Gem\nend\n\nclass Gem::Version\nend\n";
    assert_hover(source, 4, 6, "Gem", "singleton(Gem)");
    assert_no_hover(source, 4, 9);
    assert_no_hover(source, 4, 10);
    // hover over "Version" part
    assert_hover(source, 4, 11, "Gem::Version", "singleton(Gem::Version)");
}

#[test]
fn module_definition_name_is_singleton() {
    let source = "module Helpers\nend\n";
    // hover over "Helpers"
    assert_hover(source, 1, 7, "Helpers", "singleton(Helpers)");
}

#[test]
fn nested_class_inside_module_is_qualified() {
    let source = "module M\n  class C\n  end\nend\n";
    // hover over "C"
    assert_hover(source, 2, 8, "M::C", "singleton(M::C)");
}

#[test]
fn absolute_path_class_declaration_method_resolves() {
    // `class ::Foo::Bar` is registered as `Foo::Bar` with the leading `::` stripped.
    // As long as the registered name is canonical, the instance method call resolves.
    let source = concat!(
        "class ::Foo::Bar\n",
        "  def greet = \"hi\"\n",
        "end\n",
        "\n",
        "Foo::Bar.new.greet\n",
    );
    // On line 5, `Foo::Bar.new.greet`, `greet` starts at column 13.
    assert_hover(source, 5, 13, "greet", "\"hi\"");
}

#[test]
fn superclass_reference_is_singleton() {
    let source = "class Parent\nend\n\nclass Child < Parent\nend\n";
    // hover over "Parent" in `class Child < Parent`
    assert_hover(source, 4, 14, "Parent", "singleton(Parent)");
}

#[test]
fn attr_symbol_arguments_show_generated_method_type() {
    let source = concat!(
        "class Foo\n",
        "  attr_reader :foo, :bar\n",
        "  attr_accessor :age\n",
        "  attr_writer :token\n",
        "\n",
        "  def initialize\n",
        "    @foo = 1\n",
        "    @bar = \"b\"\n",
        "    @age = 20\n",
        "  end\n",
        "end\n",
    );
    assert_hover_display(source, 2, 15, "foo", "1", "-> 1");
    assert_hover_display(source, 2, 22, "bar", "\"b\"", "-> \"b\"");
    assert_hover_display(source, 3, 17, "age", "20", "-> 20");
    assert_hover_display(
        source,
        4,
        15,
        "token=",
        "untyped",
        "(untyped token) -> untyped",
    );
}

#[test]
fn class_constant_reference_is_singleton() {
    let source = "class Foo\nend\n\nFoo.new\n";
    // hover over `Foo` in the reference
    assert_hover(source, 4, 0, "Foo", "singleton(Foo)");
}

#[test]
fn attr_reader_cross_class_access() {
    let source = concat!(
        "class Foo\n",
        "  attr_reader :x\n",
        "  def initialize(x)\n",
        "    @x = x\n",
        "  end\n",
        "end\n",
        "\n",
        "Foo.new(42).x\n",
    );
    // hover over `x` method call (after Foo.new(42).)
    assert_hover(source, 8, 12, "x", "42");
}

#[test]
fn singleton_method_call_hover() {
    let source = "class Foo\n  def self.make = 42\nend\n\nFoo.make\n";
    // hover over `make` on the call
    assert_hover(source, 5, 4, "make", "42");
}

#[test]
fn module_function_method_call_hover() {
    let source = "module M\n  module_function\n  def hello = \"hi\"\nend\n\nM.hello\n";
    assert_hover(source, 6, 2, "hello", "\"hi\"");
}

#[test]
fn chained_method_call_inside_method_body() {
    let source = concat!(
        "class Inner\n",
        "  attr_reader :v\n",
        "  def initialize(v)\n",
        "    @v = v\n",
        "  end\n",
        "end\n",
        "\n",
        "class Outer\n",
        "  attr_reader :inner\n",
        "  def initialize(inner)\n",
        "    @inner = inner\n",
        "  end\n",
        "end\n",
        "\n",
        "class Driver\n",
        "  def run\n",
        "    o = Outer.new(Inner.new(42))\n",
        "    o.inner.v\n",
        "  end\n",
        "end\n",
    );
    // line 18 is `    o.inner.v`
    assert_hover(source, 18, 4, "o", "Outer");
    assert_hover(source, 18, 6, "inner", "Inner");
    assert_hover(source, 18, 12, "v", "42");
}

#[test]
fn struct_constant_field_access() {
    let source = concat!(
        "class Holder\n",
        "  Tuple = Struct.new(:a, :b)\n",
        "  def make\n",
        "    t = Tuple.new(\"x\", 1)\n",
        "    t.a\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 5, 4, "t", "Holder::Tuple");
    assert_hover(source, 5, 6, "a", "\"x\"");
}

#[test]
fn self_inside_instance_method_is_class_type() {
    let source = "class Foo\n  def describe\n    self\n  end\nend\n";
    assert_hover(source, 3, 4, "self", "Foo");
}

#[test]
fn self_class_constant_path_hover_uses_receiver_tokens() {
    let source = concat!(
        "class Sample\n",
        "  CONST = 1\n",
        "  def baz = self.class::CONST\n",
        "end\n",
    );
    assert_hover(source, 3, 12, "self", "Sample");
    assert_hover(source, 3, 17, "class", "singleton(Sample)");
    assert_no_hover(source, 3, 22);
    assert_no_hover(source, 3, 23);
    assert_hover(source, 3, 24, "Sample::CONST", "1");
}

#[test]
fn self_inside_singleton_method_is_singleton_type() {
    let source = "class Foo\n  def self.build\n    self.new\n  end\n  def initialize = nil\nend\n";
    assert_hover(source, 3, 4, "self", "singleton(Foo)");
}

#[test]
fn rubygems_version_chain() {
    // Simplified rubygems pattern: attr_reader on a sibling class reached via
    // a factory singleton method should resolve to the ivar literal at the
    // call site.
    let source = concat!(
        "class Gem::Version\n",
        "  attr_reader :version\n",
        "  def initialize(v)\n",
        "    @version = v.to_s\n",
        "  end\n",
        "  def self.create(v) = new(v)\n",
        "end\n",
        "\n",
        "class Client\n",
        "  def run\n",
        "    v = Gem::Version.create(\"1.0\")\n",
        "    v.version\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 12, 4, "v", "Gem::Version");
    assert_hover(source, 12, 6, "version", "String");
}

#[test]
fn super_call_resolves_parent_method() {
    let source = concat!(
        "class A\n",
        "  def greet\n",
        "    \"hi\"\n",
        "  end\n",
        "end\n",
        "\n",
        "class B < A\n",
        "  def greet\n",
        "    super\n",
        "  end\n",
        "end\n",
    );
    // hover at the `super` keyword
    assert_hover(source, 9, 4, "super", "\"hi\"");
}

#[test]
fn class_variable_hover_reports_union_of_writes() {
    let source =
        "class A\n  @@count = 0\n  def self.inc\n    @@count += 1\n    @@count\n  end\nend\n";
    // hover on `@@count` read on line 5
    assert_hover(source, 5, 4, "@@count", "0 | 1");
}

#[test]
fn guard_narrowed_parameter_is_non_nil() {
    let source = concat!(
        "class A\n",
        "  def run(x)\n",
        "    return if x.nil?\n",
        "    x\n",
        "  end\n",
        "end\n",
        "\n",
        "A.new.run(\"hello\")\n",
    );
    assert_hover(source, 4, 4, "x", "String");
}

#[test]
fn endless_method_receiver_chain() {
    let source = "class A\n  def version = \"1.0\"\nend\n\nA.new.version\n";
    assert_hover(source, 5, 6, "version", "\"1.0\"");
}

#[test]
fn rake_module_singleton_chain_hover() {
    let source = concat!(
        "module Rake\n",
        "  class Application\n",
        "    attr_reader :original_dir\n",
        "    def initialize\n",
        "      @original_dir = Dir.pwd\n",
        "    end\n",
        "  end\n",
        "  class << self\n",
        "    def application\n",
        "      @application ||= Rake::Application.new\n",
        "    end\n",
        "    def original_dir\n",
        "      application.original_dir\n",
        "    end\n",
        "  end\n",
        "end\n",
    );
    // Inside `Rake.original_dir`, the bare `application` call resolves to
    // Rake::Application and `.original_dir` on it resolves to String via
    // the cross-class attr_reader fallback.
    assert_hover(source, 13, 6, "application", "Rake::Application");
    assert_hover(source, 13, 18, "original_dir", "String");
}

#[test]
fn non_last_local_variable_read_has_hover() {
    // Before the process_statement fallback emitted snapshots for expression
    // statements, a bare `x` in a non-final position produced no snapshot and
    // token-based fallback returned `__todo__`.
    let source = "class A\n  def f\n    x = 10\n    x\n    y = 20\n    y\n  end\nend\n";
    assert_hover(source, 4, 4, "x", "10");
    assert_hover(source, 6, 4, "y", "20");
}

#[test]
fn non_last_ivar_read_has_hover() {
    let source = concat!(
        "class Foo\n",
        "  def initialize\n",
        "    @x = 42\n",
        "  end\n",
        "  def run\n",
        "    @x\n",
        "    @x\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 6, 4, "@x", "42");
}

#[test]
fn non_last_constant_read_has_hover() {
    let source = "class Foo\n  X = 1\n  def run\n    X\n    X\n  end\nend\n";
    assert_hover(source, 4, 4, "X", "1");
}

#[test]
fn multi_assign_from_literal_tuple() {
    let source = "class A\n  def run\n    a, b = 1, \"hi\"\n    a\n    b\n  end\nend\n";
    assert_hover(source, 4, 4, "a", "1");
    assert_hover(source, 5, 4, "b", "\"hi\"");
}

#[test]
fn multi_assign_from_method_return() {
    let source = concat!(
        "class A\n",
        "  def pair = [1, 2]\n",
        "  def run\n",
        "    a, b = pair\n",
        "    a\n",
        "    b\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 5, 4, "a", "1");
    assert_hover(source, 6, 4, "b", "2");
}

#[test]
fn rescue_reference_variable_is_exception_type() {
    let source = concat!(
        "class A\n",
        "  def run\n",
        "    begin\n",
        "    rescue StandardError => e\n",
        "      e\n",
        "    end\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 5, 6, "e", "StandardError");
}

#[test]
fn non_last_implicit_self_method_call_hover() {
    // Before the deferred-MethodReturnRef snapshot fix, an implicit-self
    // method call in a non-last position had no hover snapshot; the
    // token-based fallback returned `__todo__`.
    let source = concat!(
        "class A\n",
        "  def greet = \"hi\"\n",
        "  def run\n",
        "    greet\n",
        "    greet\n",
        "  end\n",
        "end\n",
        "A.new.run\n",
    );
    assert_hover(source, 4, 4, "greet", "\"hi\"");
    assert_hover(source, 5, 4, "greet", "\"hi\"");
}

#[test]
fn inherited_instance_factory_follows_subclass() {
    // An inherited factory annotated `#: () -> instance` resolves to the receiver's subclass.
    let source = concat!(
        "class Builder\n",
        "  #: () -> instance\n",
        "  def self.build = new\n",
        "end\n",
        "class Sub < Builder\n",
        "end\n",
        "class Client\n",
        "  def run\n",
        "    x = Sub.build\n",
        "    y = Builder.build\n",
        "    x\n",
        "    y\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 11, 4, "x", "Sub");
    assert_hover(source, 12, 4, "y", "Builder");
}

#[test]
fn instance_return_on_instance_method_is_receiver_instance() {
    // An instance method annotated `#: () -> instance` returns the receiver's instance type.
    let source = concat!(
        "class Node\n",
        "  #: () -> instance\n",
        "  def dup_node = self\n",
        "end\n",
        "class Leaf < Node\n",
        "end\n",
        "class Client\n",
        "  def run\n",
        "    n = Leaf.new.dup_node\n",
        "    n\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 10, 4, "n", "Leaf");
}

#[test]
fn self_factory_still_returns_singleton() {
    // The existing behavior of `#: () -> self` is unchanged (singleton receiver -> singleton).
    let source = concat!(
        "class Builder\n",
        "  #: () -> self\n",
        "  def self.chain = self\n",
        "end\n",
        "class Sub < Builder\n",
        "end\n",
        "class Client\n",
        "  def run\n",
        "    x = Sub.chain\n",
        "    x\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 10, 4, "x", "singleton(Sub)");
}

#[test]
fn unannotated_new_factory_follows_subclass() {
    // An unannotated inherited factory `def self.build = new` also resolves to the receiver's subclass
    // (via inference: the stored sig type is swapped to `instance`, then resolved to the receiver at the call site).
    let source = concat!(
        "class Builder\n",
        "  def self.build = new\n",
        "end\n",
        "class Sub < Builder\n",
        "end\n",
        "class Client\n",
        "  def run\n",
        "    x = Sub.build\n",
        "    y = Builder.build\n",
        "    x\n",
        "    y\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 10, 4, "x", "Sub");
    assert_hover(source, 11, 4, "y", "Builder");
}

#[test]
fn unannotated_new_factory_block_form_follows_subclass() {
    // Also applies to the block form: a regular `def` with an explicit `return new`.
    let source = concat!(
        "class Builder\n",
        "  def self.build\n",
        "    return new\n",
        "  end\n",
        "end\n",
        "class Sub < Builder\n",
        "end\n",
        "class Client\n",
        "  def run\n",
        "    x = Sub.build\n",
        "    x\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 10, 4, "x", "Sub");
}

#[test]
fn unannotated_factory_of_other_class_stays_that_class() {
    // `Foo.new` (instantiating a different class) is not affected; it stays the instantiated class regardless of the receiver.
    let source = concat!(
        "class Widget\n",
        "end\n",
        "class Builder\n",
        "  def self.build = Widget.new\n",
        "end\n",
        "class Sub < Builder\n",
        "end\n",
        "class Client\n",
        "  def run\n",
        "    x = Sub.build\n",
        "    x\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 11, 4, "x", "Widget");
}

#[test]
fn unannotated_factory_branch_stays_eager() {
    // A mixed form where only one branch is `new` (e.g. `new` and `nil`) is not affected -> as before,
    // the owner is determined eagerly (it doesn't resolve to the receiver's subclass).
    let source = concat!(
        "class Builder\n",
        "  def self.build(flag)\n",
        "    if flag\n",
        "      new\n",
        "    else\n",
        "      nil\n",
        "    end\n",
        "  end\n",
        "end\n",
        "class Sub < Builder\n",
        "end\n",
        "class Client\n",
        "  def run\n",
        "    x = Sub.build(true)\n",
        "    x\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 14, 4, "x", "Builder?");
}

#[test]
fn sorbet_attached_class_factory_follows_subclass() {
    // Sorbet's `T.attached_class` is an instance of the receiver's class. An inherited factory
    // resolves to the receiver's subclass (fixes the old incorrect `self` = singleton behavior).
    let source = concat!(
        "class Builder\n",
        "  extend T::Sig\n",
        "  sig { returns(T.attached_class) }\n",
        "  def self.build = new\n",
        "end\n",
        "class Sub < Builder\n",
        "end\n",
        "class Client\n",
        "  def run\n",
        "    x = Sub.build\n",
        "    y = Builder.build\n",
        "    x\n",
        "    y\n",
        "  end\n",
        "end\n",
    );
    assert_hover(source, 12, 4, "x", "Sub");
    assert_hover(source, 13, 4, "y", "Builder");
}

#[test]
fn instance_eval_bare_call_resolves_against_receiver() {
    // A bare call inside `x.instance_eval { m }`'s block resolves as a method on the receiver `x`
    // (because self switches to the receiver).
    let source = concat!(
        "class Widget\n",
        "  def size = 42\n",
        "end\n",
        "class Client\n",
        "  def probe\n",
        "    w = Widget.new\n",
        "    w.instance_eval { size }\n",
        "  end\n",
        "end\n",
    );
    // `size` resolves as Widget#size, and its return type is what shows up in hover.
    assert_hover(source, 7, 22, "size", "42");
}

#[test]
fn instance_exec_binds_block_params_and_switches_self() {
    // `x.instance_exec(arg) { |p| ... }` switches self to x and binds arg to p.
    let source = concat!(
        "class Widget\n",
        "  def size = 42\n",
        "end\n",
        "class Client\n",
        "  def probe\n",
        "    Widget.new.instance_exec(10) { |n| size + n }\n",
        "  end\n",
        "end\n",
    );
    // The block param `n` is bound to the argument 10.
    assert_hover(source, 6, 46, "n", "10");
}
