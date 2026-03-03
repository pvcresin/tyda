# Ruby / RBS Input / Method Resolution

## Track RBS method return type

### update

```rbs
class A
  def foo: (Integer x, Integer y) -> Integer
end
```

```ruby
class A
  def foo(x, y) = x + y
end
def foo
  x = A.new
  x.foo(1, 2)
end
```

### result

```rbs
class A
  def foo: (Integer x, Integer y) -> Integer
end

class Object
  def foo: -> Integer
end
```

## Resolve RBS method type parameter from argument

### update

```rbs
class User
  def fun_b: [T] (T t) -> T
end
```

```ruby
def generic_value
  alice = User.new
  alice.fun_b("test")
end
```

### result

```rbs
class Object
  def generic_value: -> "test"
end
```

## Resolve RBS method type parameter from keyword argument

### update

```rbs
class User
  def fun_c: [T] (value: T) -> T
end
```

```ruby
def generic_keyword_value
  alice = User.new
  alice.fun_c(value: "test")
end
```

### result

```rbs
class Object
  def generic_keyword_value: -> "test"
end
```

## Resolve RBS method type parameter from trailing argument

### update

```rbs
class User
  def fun_d: [T] (*Integer values, T last) -> T
end
```

```ruby
def generic_trailing_value
  alice = User.new
  alice.fun_d(1, 2, "test")
end
```

### result

```rbs
class Object
  def generic_trailing_value: -> "test"
end
```

## Resolve RBS class type relative to receiver

### update

```rbs
class User
  def own_class: -> class
end
```

```ruby
def rbs_class_type_value
  User.new.own_class.new
end
```

### result

```rbs
class Object
  def rbs_class_type_value: -> User
end
```

## Resolve RBS use single clause

### update

```rbs
use Types::Item

module Types
  class Item
    def name: -> String
  end
end

class User
  def item: -> Item
end
```

```ruby
def rbs_use_single_value
  User.new.item.name
end
```

### result

```rbs
class Object
  def rbs_use_single_value: -> String
end
```

## Resolve RBS use aliased clause

### update

```rbs
use Types::Item as Entry

module Types
  class Item
    def name: -> String
  end
end

class User
  def item: -> Entry
end
```

```ruby
def rbs_use_aliased_value
  User.new.item.name
end
```

### result

```rbs
class Object
  def rbs_use_aliased_value: -> String
end
```

## Resolve RBS use wildcard clause for known names

### update

```rbs
use Types::*

module Types
  class Item
    def name: -> String
  end
end

class User
  def item: -> Item
end
```

```ruby
def rbs_use_wildcard_value
  User.new.item.name
end
```

### result

```rbs
class Object
  def rbs_use_wildcard_value: -> String
end
```

## Resolve RBS proc with untyped parameters

### update

```rbs
class User
  def callback: -> ^(?) -> String
end
```

```ruby
def rbs_untyped_proc_value
  User.new.callback.call(1, "a")
end
```

### result

```rbs
class Object
  def rbs_untyped_proc_value: -> String
end
```

## Resolve RBS class alias declaration

### update

```rbs
module Types
  class Item
    def name: -> String
  end
end

class Entry = Types::Item

class User
  def item: -> Entry
end
```

```ruby
def rbs_class_alias_value
  User.new.item.name
end
```

### result

```rbs
Entry: singleton(Types::Item)

class Object
  def rbs_class_alias_value: -> String
end
```

## Resolve RBS module alias declaration

### update

```rbs
module Types
  module Named
    def label: -> String
  end
end

module Alias = Types::Named

class User
  include Alias
end
```

```ruby
def rbs_module_alias_value
  User.new.label
end
```

### result

```rbs
Alias: singleton(Types::Named)

class Object
  def rbs_module_alias_value: -> String
end
```

## Bind RBS block self type

### update

```rbs
class Context
  def title: -> String
end

class User
  def with_context: [T] () { () [self: Context] -> T } -> T
end
```

```ruby
def rbs_block_self_value
  User.new.with_context { title }
end
```

### result

```rbs
class Object
  def rbs_block_self_value: -> String
end
```

## Resolve RBS proc self return

### update

```rbs
class Context
end

class User
  def build_context: [T] (T context) -> ^() [self: T] -> self
end
```

```ruby
def rbs_proc_self_return
  User.new.build_context(Context.new).call
end
```

### result

```rbs
class Object
  def rbs_proc_self_return: -> Context
end
```

## Match RBS proc self parameter

### update

```rbs
class Context
end

class User
  def accept_context: (^() [self: Context] -> self callback) -> String
                    | (^() -> Integer callback) -> Integer
end
```

```ruby
def rbs_proc_self_parameter
  context = Context.new
  User.new.accept_context(-> { context })
end
```

### result

```rbs
class Object
  def rbs_proc_self_parameter: -> String
end
```

## Bind RBS optional block positional parameter

### update

```rbs
class User
  def with_optional_block: [T] () { (String first, ?Integer count) -> T } -> T
end
```

```ruby
def rbs_block_optional_value
  User.new.with_optional_block { |first, count| count }
end
```

### result

```rbs
class Object
  def rbs_block_optional_value: -> Integer
end
```

## Bind RBS rest and trailing block parameters

### update

```rbs
class User
  def with_trailing_block: [T] () { (*Integer values, String last) -> T } -> T
end
```

```ruby
def rbs_block_trailing_value
  User.new.with_trailing_block { |a, b, last| last }
end
```

### result

```rbs
class Object
  def rbs_block_trailing_value: -> String
end
```

## Bind RBS keyword block parameter

### update

```rbs
class User
  def with_keyword_block: [T] () { (name: String) -> T } -> T
end
```

```ruby
def rbs_block_keyword_value
  User.new.with_keyword_block { |name:| name }
end
```

### result

```rbs
class Object
  def rbs_block_keyword_value: -> String
end
```

## Respect RBS required block on no-block calls

### update

```rbs
class RbsBlockPresenceSource
  def required: () { () -> String } -> String
  def optional: () ?{ () -> String } -> Symbol
  def mixed: () { () -> String } -> String
           | () -> Integer
end
```

```ruby
def rbs_block_presence_values
  source = RbsBlockPresenceSource.new
  [
    source.required,
    source.optional,
    source.mixed,
    source.required { "x" }
  ]
end
```

### result

```rbs
class Object
  def rbs_block_presence_values: -> [untyped, Symbol, Integer, String]
end
```

## Required block overload with matching arg is skipped

### update

```rbs
class RbsRequiredBlockArgSource
  def pick: (String value) { () -> String } -> String
          | (Integer value) -> Integer
end
```

```ruby
def rbs_required_block_arg_values
  source = RbsRequiredBlockArgSource.new
  [
    source.pick("x"),
    source.pick(1),
    source.pick("x") { "y" }
  ]
end
```

### result

```rbs
class Object
  def rbs_required_block_arg_values: -> [untyped, Integer, String]
end
```

## Symbol block overload uses call args

### update

```rbs
class RbsSymbolBlockOverloadSource
  def cast: (String value) { (Integer item) -> Integer } -> Integer
          | (Integer value) { (String item) -> String } -> String
end
```

```ruby
def rbs_symbol_block_overload_values
  source = RbsSymbolBlockOverloadSource.new
  [
    source.cast("x", &:itself),
    source.cast(1, &:itself)
  ]
end
```

### result

```rbs
class Object
  def rbs_symbol_block_overload_values: -> [Integer, String]
end
```

## Optional block generic overload works without block

### update

```rbs
class RbsOptionalBlockGenericSource
  def wrap: [T] (T value) ?{ (T item) -> void } -> Array[T]
end
```

```ruby
def rbs_optional_block_generic_values
  source = RbsOptionalBlockGenericSource.new
  [
    source.wrap("x"),
    source.wrap(1) { |item| item.to_s }
  ]
end
```

### result

```rbs
class Object
  def rbs_optional_block_generic_values: -> [Array["x"], Array[1]]
end
```

## Substitute RBS method type parameter inside record return

### update

```rbs
class User
  def fun_e: [T] (T value) -> { value: T }
end
```

```ruby
def generic_record_value
  alice = User.new
  alice.fun_e("test")[:value]
end
```

### result

```rbs
class Object
  def generic_record_value: -> "test"
end
```

## Resolve RBS untyped function with arbitrary arguments

### update

```rbs
class RbsUntypedFunctionSource
  def value: (?) -> String
end
```

```ruby
def rbs_untyped_function_values
  source = RbsUntypedFunctionSource.new
  [
    source.value,
    source.value(1, "x"),
    source.value(name: "x"),
    source.value(1, name: "x")
  ]
end
```

### result

```rbs
class Object
  def rbs_untyped_function_values: -> [String, String, String, String]
end
```

## Choose exact RBS literal overload

### update

```rbs
class RbsChooser
  def pick: (:text) -> String
          | (:count) -> Integer
end
```

```ruby
def rbs_pick_count(x) = x.pick(:count)

rbs_pick_count(RbsChooser.new)
```

### result

```rbs
class Object
  def rbs_pick_count: (RbsChooser x) -> Integer
end
```

## Method chain from RBS definition

### update

```rbs
class A
  def foo: (Integer x) -> String
  def bar: (String y) -> Integer
end
```

```ruby
class A
  def foo(x) = x.to_s
  def bar(y) = y.to_i
end
def foo
  x = A.new
  x.foo(42).length
end
```

### result

```rbs
class A
  def foo: (Integer x) -> String
  def bar: (String y) -> Integer
end

class Object
  def foo: -> Integer
end
```

## Call another stdlib method from RBS return type

### update

```rbs
class A
  def foo: (Integer x) -> String
end
```

```ruby
class A
  def foo(x) = x.to_s
end
def foo
  x = A.new
  x.foo(42).upcase
end
```

### result

```rbs
class A
  def foo: (Integer x) -> String
end

class Object
  def foo: -> String
end
```

## Track inferred method return without RBS

### update

```ruby
class A
  def foo(x) = "x"
end
def foo
  x = A.new
  x.foo("y")
end
```

### result

```rbs
class A
  def foo: (String x) -> "x"
end

class Object
  def foo: -> "x"
end
```

## Infer method returning Integer

### update

```ruby
class A
  def foo = 42
end
def foo
  x = A.new
  x.foo
end
```

### result

```rbs
class A
  def foo: -> 42
end

class Object
  def foo: -> 42
end
```

## Chain stdlib methods from inferred return

### update

```ruby
class Greeter
  def greet(name) = "Hello"
end
def test
  g = Greeter.new
  g.greet("world").length
end
```

### result

```rbs
class Greeter
  def greet: (String name) -> "Hello"
end

class Object
  def test: -> 5
end
```

## Resolve class with multiple methods

### update

```rbs
class Converter
  def to_string: (Integer n) -> String
  def to_int: (String s) -> Integer
end
```

```ruby
class Converter
  def to_string(n) = n.to_s
  def to_int(s) = s.to_i
end
def test_string
  c = Converter.new
  c.to_string(42)
end
def test_int
  c = Converter.new
  c.to_int("42")
end
```

### result

```rbs
class Converter
  def to_string: (Integer n) -> String
  def to_int: (String s) -> Integer
end

class Object
  def test_string: -> String
  def test_int: -> Integer
end
```

## Type chain from RBS Int to Float

### update

```rbs
class Converter
  def to_int: (String s) -> Integer
end
```

```ruby
class Converter
  def to_int(s) = s.to_i
end
def test
  c = Converter.new
  c.to_int("42").to_f
end
```

### result

```rbs
class Converter
  def to_int: (String s) -> Integer
end

class Object
  def test: -> Float
end
```

## Inherit omitted overload return from parent class

### update

```rbs
class A
  def foo: -> Integer
end

class B < A
  def foo: ...
end
```

```ruby
def foo = B.new.foo
```

### result

```rbs
class Object
  def foo: -> Integer
end
```

## Resolve singleton RBS alias with same instance method name

### update

```rbs
class RbsAliasPair
  def value: -> String
  def self.value: -> Integer
  alias self.count self.value
end
```

```ruby
def rbs_singleton_alias_value
  RbsAliasPair.count
end
```

### result

```rbs
class Object
  def rbs_singleton_alias_value: -> Integer
end
```

## Resolve instance RBS alias with same singleton method name

### update

```rbs
class RbsAliasSource
  def self.value: -> Integer
  def value: -> String
  alias label value
end
```

```ruby
def rbs_instance_alias_value
  RbsAliasSource.new.label
end
```

### result

```rbs
class Object
  def rbs_instance_alias_value: -> String
end
```

## Resolve RBS alias from superclass method

### update

```rbs
class RbsAliasParent
  def value: -> String
end

class RbsAliasChild < RbsAliasParent
  alias label value
end
```

```ruby
def rbs_super_alias_value
  RbsAliasChild.new.label
end
```

### result

```rbs
class Object
  def rbs_super_alias_value: -> String
end
```

## Resolve RBS alias from included module method

### update

```rbs
module RbsAliasModule
  def label: -> Symbol
end

class RbsAliasConsumer
  include RbsAliasModule
  alias tag label
end
```

```ruby
def rbs_mixin_alias_value
  RbsAliasConsumer.new.tag
end
```

### result

```rbs
class Object
  def rbs_mixin_alias_value: -> Symbol
end
```
