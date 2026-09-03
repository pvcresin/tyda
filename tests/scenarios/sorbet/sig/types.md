# Sorbet / Sig / Types

## T::Boolean to bool

```ruby
class Foo
  sig { params(flag: T::Boolean).returns(T::Boolean) }
  def toggle(flag) = !flag
end
```

### result

```rbs
class Foo
  def toggle: (bool flag) -> bool
end
```

## T.nilable to optional

```ruby
class Foo
  sig { params(name: T.nilable(String)).returns(T.nilable(Integer)) }
  def parse(name) = name&.length
end
```

### result

```rbs
class Foo
  def parse: (String? name) -> Integer?
end
```

## T.any to union

```ruby
class Foo
  sig { params(value: T.any(String, Integer)).returns(String) }
  def stringify(value) = value.to_s
end
```

### result

```rbs
class Foo
  def stringify: ((Integer | String) value) -> String
end
```

## T.untyped

```ruby
class Foo
  sig { params(x: T.untyped).returns(T.untyped) }
  def passthrough(x) = x
end
```

### result

```rbs
class Foo
  def passthrough: (untyped x) -> untyped
end
```

## T::Array[T]

```ruby
class Foo
  sig { params(items: T::Array[String]).returns(T::Array[Integer]) }
  def lengths(items) = items.map(&:length)
end
```

### result

```rbs
class Foo
  def lengths: (Array[String] items) -> Array[Integer]
end
```

## T::Hash[K, V]

```ruby
class Foo
  sig { params(data: T::Hash[Symbol, Integer]).returns(T::Hash[String, String]) }
  def transform(data) = {}
end
```

### result

```rbs
class Foo
  def transform: (Hash[Symbol, Integer] data) -> Hash[String, String]
end
```

## T.class_of to singleton

```ruby
class Foo
  sig { returns(T.class_of(String)) }
  def string_class = String
end
```

### result

```rbs
class Foo
  def string_class: -> singleton(String)
end
```

## T::Class and T::Module to class object types

```ruby
class Foo
  sig { returns(T::Class[T.anything]) }
  def any_class = Foo

  sig { returns(T::Class[String]) }
  def string_class = String

  sig { returns(T::Module[T.anything]) }
  def some_module = Comparable
end
```

### result

```rbs
class Foo
  def any_class: -> Class
  def string_class: -> singleton(String)
  def some_module: -> Module
end
```

## T.self_type to self and T.attached_class to instance

```ruby
class Fluent
  sig { returns(T.self_type) }
  def chain = self

  sig { returns(T.attached_class) }
  def self.factory = new
end
```

### result

```rbs
class Fluent
  def chain: -> self
  def self.factory: -> Fluent
end
```

## T.noreturn to bot

```ruby
class Foo
  sig { returns(T.noreturn) }
  def fail_hard
    raise "error"
  end
end
```

### result

```rbs
class Foo
  def fail_hard: -> bot
end
```

## Nested T.nilable with T.any

```ruby
class Foo
  sig { params(x: T.nilable(T.any(String, Integer))).returns(String) }
  def to_str(x) = x.to_s
end
```

### result

```rbs
class Foo
  def to_str: ((Integer | String)? x) -> String
end
```

## T.anything to top

```ruby
class Foo
  sig { params(x: T.anything).returns(T.anything) }
  def accept_anything(x) = x
end
```

### result

```rbs
class Foo
  def accept_anything: (top x) -> top
end
```

## T.all to intersection type

```ruby
class Foo
  sig { params(x: T.all(Comparable, Enumerable)).returns(T.all(Comparable, Enumerable)) }
  def process(x) = x
end
```

### result

```rbs
class Foo
  def process: (Comparable & Enumerable x) -> Comparable & Enumerable
end
```

## T.all intersection with three or more types

```ruby
class Foo
  sig { params(x: T.all(Readable, Writable, Closeable)).returns(String) }
  def use_io(x) = "done"
end
```

### result

```rbs
class Foo
  def use_io: (Readable & Writable & Closeable x) -> String
end
```

## Convert Sorbet collection aliases to RBS generics

```ruby
class CollectionTypes
  sig { returns(T::Enumerator::Lazy[Integer]) }
  def lazy_items = [].lazy

  sig { returns(T::Set[String]) }
  def names = Set.new

  sig { returns(T::Range[Integer]) }
  def range = 1..3

  sig { returns(T::Enumerable[String]) }
  def each_name = []
end
```

### result

```rbs
class CollectionTypes
  def lazy_items: -> Enumerator::Lazy[Integer]
  def names: -> Set[String]
  def range: -> Range[Integer]
  def each_name: -> Enumerable[String]
end
```

## T.noreturn parameter

```ruby
class Foo
  sig { params(handler: T.proc.returns(T.noreturn)).void }
  def on_error(handler)
  end
end
```

### result

```rbs
class Foo
  def on_error: (Proc handler) -> void
end
```

## T.anything parameter and return

```ruby
class Foo
  sig { params(x: T.anything, y: T.anything).returns(T.anything) }
  def accept_all(x, y) = x
end
```

### result

```rbs
class Foo
  def accept_all: (top x, top y) -> top
end
```

## RBS comment wins over sig

```ruby
class Foo
  sig { returns(String) }
  #: -> Integer
  def priority = 42
end
```

### result

```rbs
class Foo
  def priority: -> Integer
end
```

## Attached class factory returns the subclass

### update

`sorbet/config`

```ruby
.
```

```ruby
class Module
  include T::Sig
end

class Parent
  sig { returns(T.attached_class) }
  def self.make = new
end

class Child < Parent
end

def foo = Child.make
```

### result

```rbs
class Module
  include T::Sig
end

class Object < BasicObject
  def foo: -> Child
end

class Parent
  def self.make: -> Parent
end

module T::Sig
  def sig: -> untyped
end
```
