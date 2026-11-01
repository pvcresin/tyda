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

## Infer Object method on untyped receiver

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
  def check_nil: (untyped x) -> bool
  def check_equal: (untyped x, untyped y) -> bool
  def object_identity: (untyped x) -> Integer
  def to_string: (untyped x) -> String
  def to_integer: (untyped x) -> Integer
  def check_pred: (untyped x) -> untyped
end
```

## Keep a known nil predicate on a union with an untyped member

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
  def nil_predicate: (untyped flag) -> bool
  def value_type: (untyped flag) -> (untyped | "known")
  def unknown_predicate: (untyped flag) -> untyped
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

## Infer common conversion methods on untyped receiver

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
  def sym: (untyped x) -> Symbol
  def len: (untyped x) -> untyped
  def siz: (untyped x) -> untyped
  def cnt: (untyped x) -> untyped
  def flt: (untyped x) -> Float
  def arr: (untyped x) -> Array
  def neg: (untyped x) -> bool
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
  def custom_date_conversion_union: (untyped flag) -> (:custom | Date)
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
  def value_as_string: -> String
  def build_after_raise: -> bot
  def build_after_keyword_raise: -> bot
end

class ResultBuilder
  def self.build: (untyped value) -> :built
  def self.build_with_keyword: (value: untyped) -> :built
end
```
