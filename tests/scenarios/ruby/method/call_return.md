# Ruby / Method / Call Return

## String-like call alone does not force String

### update

```ruby
class A
  def foo(x) = x.strip[0, 10]
end
```

### result

```rbs
class A
  def foo: (untyped x) -> untyped
end
```

## Return another method return value

### update

```ruby
def foo = 1
def bar = foo
```

### result

```rbs
class Object < BasicObject
  def foo: -> 1
  def bar: -> 1
end
```

## Chained method call

### update

```ruby
def first = "hello"
def second = first
def third = second
```

### result

```rbs
class Object < BasicObject
  def first: -> "hello"
  def second: -> "hello"
  def third: -> "hello"
end
```

## Method call inside class

### update

```ruby
class A
  def foo = 42
  def bar = foo
end
```

### result

```rbs
class A
  def foo: -> 42
  def bar: -> 42
end
```

## Keep Object methods untyped on an untyped receiver

### update

```ruby
class Checker
  def check_nil(x) = x.nil?
  def check_equal(x, y) = x == y
  def object_identity(x) = x.object_id
  def to_string(x) = x.to_s
  def to_integer(x) = x.to_i
  def check_pred(x) = x.valid?
end
```

### result

```rbs
class Checker
  def check_nil: (untyped x) -> untyped
  def check_equal: (untyped x, untyped y) -> untyped
  def object_identity: (untyped x) -> untyped
  def to_string: (untyped x) -> untyped
  def to_integer: (untyped x) -> untyped
  def check_pred: (untyped x) -> untyped
end
```

## Dispatch nil? per arm on a union with an untyped member

### update

```ruby
class UntypedUnionPredicate
  #: (untyped flag) -> untyped
  def dynamic_value(flag); end

  def nil_predicate(flag)
    value = flag ? "known" : dynamic_value(flag)
    value.nil?
  end

  def value_type(flag)
    value = flag ? "known" : dynamic_value(flag)
    value
  end

  def unknown_predicate(flag)
    value = flag ? "known" : dynamic_value(flag)
    value.valid?
  end
end
```

### result

```rbs
class UntypedUnionPredicate
  def dynamic_value: (untyped flag) -> untyped
  def nil_predicate: (untyped flag) -> untyped
  def value_type: (untyped flag) -> untyped
  def unknown_predicate: (untyped flag) -> untyped
end
```

## Dispatch a call per arm when the receiver union has an untyped arm

### update

```ruby
class UntypedArmTarget
  def label = :label
  def with_block = yield
end

class UntypedArmCaller
  #: () -> untyped
  def dynamic_value; end

  def receiver
    @receiver ||= (dynamic_value ? dynamic_value : UntypedArmTarget.new)
  end

  def method_receiver = receiver.label

  def ivar_receiver
    @receiver.label
  end

  def local_receiver(flag)
    value = flag ? dynamic_value : UntypedArmTarget.new
    value.label
  end

  def block_call(flag)
    value = flag ? dynamic_value : UntypedArmTarget.new
    value.with_block { :from_block }
  end

  def safe_navigation(flag)
    value = flag ? dynamic_value : UntypedArmTarget.new
    value&.label
  end

  def nil_arm_stays_unknown(flag)
    value = flag ? dynamic_value : nil
    value.label
  end

  def typed_arms_only(flag)
    value = flag ? UntypedArmTarget.new : :symbol
    value.to_s
  end
end
```

### result

```rbs
class UntypedArmCaller
  def dynamic_value: -> untyped
  def receiver: -> untyped
  def method_receiver: -> untyped
  def ivar_receiver: -> untyped
  def local_receiver: (untyped flag) -> untyped
  def block_call: (untyped flag) -> untyped
  def safe_navigation: (untyped flag) -> untyped
  def nil_arm_stays_unknown: (untyped flag) -> untyped
  def typed_arms_only: (untyped flag) -> String
end

class UntypedArmTarget
  def label: -> :label
  def with_block: { () -> :from_block } -> :from_block
end
```

## Keep source-defined nil predicates unknown on an untyped union

### update

```ruby
class CustomNilPredicate
  def nil? = :sentinel
end

class UntypedUnionOverride
  #: () -> untyped
  def dynamic_value; end

  def nil_predicate(flag)
    value = flag ? CustomNilPredicate.new : dynamic_value
    value.nil?
  end
end
```

### result

```rbs
class CustomNilPredicate
  def nil?: -> :sentinel
end

class UntypedUnionOverride
  def dynamic_value: -> untyped
  def nil_predicate: (untyped flag) -> untyped
end
```

## Keep conversion methods untyped on an untyped receiver

### update

```ruby
class Conv
  def sym(x) = x.to_sym
  def len(x) = x.length
  def siz(x) = x.size
  def cnt(x) = x.count
  def flt(x) = x.to_f
  def arr(x) = x.to_a
  def neg(x) = !x
end
```

### result

```rbs
class Conv
  def sym: (untyped x) -> untyped
  def len: (untyped x) -> untyped
  def siz: (untyped x) -> untyped
  def cnt: (untyped x) -> untyped
  def flt: (untyped x) -> untyped
  def arr: (untyped x) -> untyped
  def neg: (untyped x) -> untyped
end
```

## Keep non-core methods untyped on an untyped receiver

### update

```ruby
def unresolved_validity(x) = x.valid?
```

### result

```rbs
class Object < BasicObject
  def unresolved_validity: (untyped x) -> untyped
end
```

## Keep overridden core methods untyped on an untyped receiver

### update

```ruby
class CustomEquality
  def ==(_other) = "not a boolean"
end

def unresolved_equality(x) = x == 1
```

### result

```rbs
class CustomEquality
  def ==: (untyped _other) -> "not a boolean"
end

class Object < BasicObject
  def unresolved_equality: (untyped x) -> untyped
end
```

## Keep source-overridden conversions untyped on an unresolved receiver

### update

```ruby
class CustomConversion
  def to_s = :custom
end

def unresolved_to_string(value) = value.to_s
```

### result

```rbs
class CustomConversion
  def to_s: -> :custom
end

class Object < BasicObject
  def unresolved_to_string: (untyped value) -> untyped
end
```

## Keep source-overridden date conversions untyped on an unresolved receiver

### update

```ruby
class CustomDateConversion
  def to_date = :custom
end

def unresolved_to_date(value) = value.to_date

def custom_date_conversion_union(flag)
  value = if flag
    CustomDateConversion.new
  else
    "2024-01-01"
  end
  value.to_date
end
```

### result

```rbs
class CustomDateConversion
  def to_date: -> :custom
end

class Object < BasicObject
  def unresolved_to_date: (untyped value) -> untyped
  def custom_date_conversion_union: (untyped flag) -> :custom
end
```

## Resolve Kernel conversion methods as bare calls

### update

```ruby
class A
  def to_i = Integer("42")
  def to_f = Float("3.14")
  def to_s_str = String(42)
  def to_k_via_singleton = Kernel.Integer("42")
end
```

### result

```rbs
class A
  def to_i: -> 42
  def to_f: -> Float
  def to_s_str: -> String
  def to_k_via_singleton: -> Integer
end
```

## Resolve super return from parent method

### update

```ruby
class Parent
  def greet = "hello"
end

class Child < Parent
  def greet
    super + " world"
  end
end
```

### result

```rbs
class Child < Parent
  def greet: -> String
end

class Parent
  def greet: -> "hello"
end
```

## super inside def self.new returns class instance

### update

```ruby
class Cached
  def initialize(key) = @key = key

  def self.new(key)
    super
  end
end
```

### result

```rbs
class Cached
  def initialize: (untyped key) -> void
  def self.new: (untyped key) -> Cached
end
```

## Enumerable inject and reduce with symbol proc

### update

```ruby
class A
  def self.a = [1,2,3].inject(:+)
  def self.b = [1,2,3].inject(0, :+)
  def self.c = [1.0,2.0].inject(:+)
  def self.d = [1,2,3].reduce(:*)
end
```

### result

```rbs
class A
  def self.a: -> Integer
  def self.b: -> Integer
  def self.c: -> Float
  def self.d: -> Integer
end
```

## Kernel rand(Integer) returns Integer

### update

```ruby
class A
  def self.rand0 = rand
  def self.rand_int = rand(10)
end
```

### result

```rbs
class A
  def self.rand0: -> Float
  def self.rand_int: -> Integer
end
```

## super positional arg reaches parent initialize param

### update

```ruby
class Base
  attr_reader :command, :summary

  def initialize(command, summary = nil)
    @command = command
    @summary = summary
  end
end

class Push < Base
  def initialize
    super "push", "Push a gem"
  end
end

Push.new
```

### result

```rbs
class Base
  def command: -> String
  def summary: -> String
  def initialize: (String command, ?String? summary) -> void
end

class Push < Base
  def initialize: -> void
end
```

## Unknown count results stay untyped

### update

```ruby
class UnknownBuilder
  #: () -> untyped
  def self.make; end
end

def unknown_count = UnknownBuilder.make.count
```

### result

```rbs
class Object < BasicObject
  def unknown_count: -> untyped
end

class UnknownBuilder
  def self.make: -> untyped
end
```

## Abstract method arguments do not hide callee returns

### update

```ruby
class ResultBuilder
  def self.build(value) = :built
  def self.build_with_keyword(value:) = :built
end

class AbstractSource
  def value = raise NotImplementedError
  def build = ResultBuilder.build(value)
  def build_with_keyword = ResultBuilder.build_with_keyword(value: value)
  def value_as_string = value.to_s
  def build_after_raise = ResultBuilder.build(raise "stop")
  def build_after_keyword_raise = ResultBuilder.build_with_keyword(value: raise("stop"))
end
```

### result

```rbs
class AbstractSource
  def value: -> bot
  def build: -> :built
  def build_with_keyword: -> :built
  def value_as_string: -> untyped
  def build_after_raise: -> bot
  def build_after_keyword_raise: -> bot
end

class ResultBuilder
  def self.build: (untyped value) -> :built
  def self.build_with_keyword: (value: untyped) -> :built
end
```

## Unreachable local arguments stay bot beside other calls

### update

```ruby
class BotArgumentFactory
  def self.build(value, decoration) = :built
end

class BotArgumentBoundary
  def decoration = :decorated

  def unreachable_call
    value = raise "stop"
    BotArgumentFactory.build(value, decoration)
  end
end
```

### result

```rbs
class BotArgumentBoundary
  def decoration: -> :decorated
  def unreachable_call: -> bot
end

class BotArgumentFactory
  def self.build: (untyped value, untyped decoration) -> :built
end
```

## Calls with arguments on ivar receivers resolve after the ivar settles

### update

```ruby
class SettledIvarLines
  attr_reader :ranges

  def initialize(ranges)
    @lines = ["a", "b"]
    @ranges = ranges
  end

  def line_width = @lines[0].size
  def indent
    line = @lines[0]
    line.size - line.lstrip.size
  end
  def multiple_via_ivar? = @ranges.size > 1
  def multiple_via_reader? = ranges.size > 1
  def first_range = ranges[0]
end

SettledIvarLines.new([0..1])
```

### result

```rbs
class SettledIvarLines
  def ranges: -> Array[Range[Integer]]
  def initialize: (Array[Range[Integer]] ranges) -> void
  def line_width: -> Integer
  def indent: -> Integer
  def multiple_via_ivar?: -> bool
  def multiple_via_reader?: -> bool
  def first_range: -> Range[Integer]?
end
```

## A block call does not return the block-less Enumerator overload

### update

```ruby
class BlockOverloadReturn
  def unresolved_block
    ["a"].flat_map { |item| unknown(item) }
  end

  def yielded_block
    ["a"].flat_map { |item| yield item }
  end

  def without_block
    ["a"].flat_map
  end
end
```

### result

```rbs
class BlockOverloadReturn
  def unresolved_block: -> Array[untyped]
  def yielded_block: { (untyped) -> untyped } -> Array[untyped]
  def without_block: -> Enumerator["a", Array[untyped]]
end
```

## A length read through a parameter keeps the nil of IO#read

### update

```ruby
class ReadLength
  def read_part(file, len)
    file.read(len)
  end

  def run
    read_part(File.open("x"), 3)
  end
end
```

### result

```rbs
class ReadLength
  def read_part: (File file, Integer len) -> String?
  def run: -> String?
end
```
