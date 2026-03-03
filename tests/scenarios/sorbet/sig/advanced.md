# Sorbet / Sig / Advanced

## Basic sig do end

```ruby
class Foo
  sig do
    params(x: Integer, y: String)
    .returns(String)
  end
  def convert(x, y) = "#{x}: #{y}"
end
```

### result

```rbs
class Foo
  def convert: (Integer x, String y) -> String
end
```

## sig do...end void

```ruby
class Foo
  sig do
    void
  end
  def setup
  end
end
```

### result

```rbs
class Foo
  def setup: -> void
end
```

## sig do end with multi-line params

```ruby
class Foo
  sig do
    params(
      name: String,
      age: Integer,
      email: String,
    )
    .returns(String)
  end
  def register(name, age, email) = "#{name} (#{age})"
end
```

### result

```rbs
class Foo
  def register: (String name, Integer age, String email) -> String
end
```

## override modifier

```ruby
class Parent
  sig { returns(String) }
  def name = "parent"
end

class Child < Parent
  sig { override.returns(String) }
  def name = "child"
end
```

### result

```rbs
class Child < Parent
  def name: -> String
end

class Parent
  def name: -> String
end
```

## overridable modifier

```ruby
class Base
  sig { overridable.params(x: Integer).returns(String) }
  def process(x) = x.to_s
end
```

### result

```rbs
class Base
  def process: (Integer x) -> String
end
```

## abstract modifier

```ruby
class Shape
  sig { abstract.returns(Float) }
  def area; end
end
```

### result

```rbs
class Shape
  def area: -> Float
end
```

## override and overridable chain

```ruby
class Middle
  sig { override.overridable.params(x: String).returns(Integer) }
  def process(x) = x.length
end
```

### result

```rbs
class Middle
  def process: (String x) -> Integer
end
```

## checked modifier

```ruby
class Perf
  sig { checked(:never).params(data: String).returns(Integer) }
  def fast_parse(data) = data.length
end
```

### result

```rbs
class Perf
  def fast_parse: (String data) -> Integer
end
```

## on_failure generated and final keep only type info

```ruby
class SigModifiers
  sig { on_failure(:soft_notify).params(x: String).returns(Integer) }
  def soft_parse(x) = x.length

  sig { generated.returns(String) }
  def generated_name = "generated"

  sig { final.returns(Integer) }
  def answer = 42
end
```

### result

```rbs
class SigModifiers
  def soft_parse: (String x) -> Integer
  def generated_name: -> String
  def answer: -> Integer
end
```

## Single type_parameter

```ruby
class Container
  sig { type_parameters(:U).params(x: T.type_parameter(:U)).returns(T.type_parameter(:U)) }
  def identity(x) = x
end
```

### result

```rbs
class Container
  def identity: (U x) -> U
end
```

## Multiple type_parameters

```ruby
class Transform
  sig { type_parameters(:A, :B).params(x: T.type_parameter(:A), y: T.type_parameter(:B)).returns(T.type_parameter(:B)) }
  def swap(x, y) = y
end
```

### result

```rbs
class Transform
  def swap: (A x, B y) -> B
end
```

## type_parameters sig do...end

```ruby
class Wrapper
  sig do
    type_parameters(:U)
      .params(
        blk: T.proc.returns(T.type_parameter(:U))
      )
      .returns(T.type_parameter(:U))
  end
  def with_timer(&blk) = yield
end
```

### result

```rbs
class Wrapper
  def with_timer: (?Proc &blk) -> U
end
```

## Nested T.nilable and T::Array

```ruby
class Finder
  sig { params(items: T::Array[String]).returns(T.nilable(String)) }
  def find_first(items) = items.first
end
```

### result

```rbs
class Finder
  def find_first: (Array[String] items) -> String?
end
```

## T.any with three or more types

```ruby
class Parser
  sig { params(input: T.any(String, Integer, Symbol)).returns(String) }
  def parse(input) = input.to_s
end
```

### result

```rbs
class Parser
  def parse: ((Integer | String | Symbol) input) -> String
end
```

## T::Hash with complex value type

```ruby
class Config
  sig { returns(T::Hash[String, T::Array[Integer]]) }
  def settings = {}
end
```

### result

```rbs
class Config
  def settings: -> Hash[String, Array[Integer]]
end
```

## Block with T.proc.params

```ruby
class Iterator
  sig { params(items: T::Array[Integer], blk: T.proc.params(x: Integer).returns(String)).returns(T::Array[String]) }
  def map_items(items, &blk) = items.map(&blk)
end
```

### result

```rbs
class Iterator
  def map_items: (Array[Integer] items, ?Proc &blk) -> Array[String]
end
```

## Block with T.proc.void

```ruby
class Logger
  sig { params(msg: String, blk: T.proc.void).void }
  def log(msg, &blk)
    puts msg
    blk.call
  end
end
```

### result

```rbs
class Logger
  def log: (String msg, ?Proc &blk) -> void
end
```

## Unknown sig modifier does not crash

```ruby
class Safe
  sig { some_future_unknown_modifier.returns(Integer) }
  def safe_method = 42
end
```

### result

```rbs
class Safe
  def safe_method: -> Integer
end
```

## Mix methods with and without sig

```ruby
class Mixed
  sig { returns(Integer) }
  def typed_method = 42

  def inferred_method = "hello"

  sig { params(x: String).returns(Integer) }
  def another_typed(x) = x.length
end
```

### result

```rbs
class Mixed
  def typed_method: -> Integer
  def inferred_method: -> "hello"
  def another_typed: (String x) -> Integer
end
```

## NilClass to nil

```ruby
class Nullable
  sig { returns(NilClass) }
  def nothing = nil
end
```

### result

```rbs
class Nullable
  def nothing: -> nil
end
```

## T::Boolean parameter and return

```ruby
class Validator
  sig { params(value: T.any(String, Integer), strict: T::Boolean).returns(T::Boolean) }
  def valid?(value, strict) = true
end
```

### result

```rbs
class Validator
  def valid?: ((Integer | String) value, bool strict) -> bool
end
```

## override + sig do...end

```ruby
class Subclass
  sig do
    override
    .params(x: Integer)
    .returns(String)
  end
  def format(x) = x.to_s
end
```

### result

```rbs
class Subclass
  def format: (Integer x) -> String
end
```

## Top-level sig do end

```ruby
sig do
  params(x: Integer, y: Integer)
  .returns(Integer)
end
def add(x, y) = x + y
```

### result

```rbs
class Object
  def add: (Integer x, Integer y) -> Integer
end
```

## Register type parameter with type_member

### update

```ruby
class GenericBox
  Elem = type_member

  sig { returns(Elem) }
  def get
  end
end
```

### result

```rbs
class GenericBox[Elem]
  def get: -> Elem
end
```
