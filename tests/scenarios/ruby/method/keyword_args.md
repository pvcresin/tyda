# Ruby / Method / Keyword Args

## Required keyword arg

### update

```ruby
def foo(x:) = x
foo(x: "hello")
```

### result

```rbs
class Object
  def foo: (x: String) -> String
end
```

## Optional keyword arg with default

### update

```ruby
def foo(x: 1) = x
```

### result

```rbs
class Object
  def foo: (?x: Integer) -> Integer
end
```

## Combine required and optional args

### update

```ruby
def foo(x:, y: 1) = x
foo(x: "hello")
```

### result

```rbs
class Object
  def foo: (x: String, ?y: Integer) -> String
end
```

## Call overrides optional keyword arg

### update

```ruby
def foo(x: 1) = x
foo(x: "str")
```

### result

```rbs
class Object
  def foo: (?x: (Integer | String)) -> (String | 1)
end
```

## Keyword arg with no calls

### update

```ruby
def foo(x:, y: "default") = x
```

### result

```rbs
class Object
  def foo: (x: untyped, ?y: String) -> untyped
end
```

## Double splat arg

### update

```ruby
def foo(**opts) = 42
```

### result

```rbs
class Object
  def foo: (**untyped opts) -> 42
end
```

## `**nil` rejects keyword args

### update

```ruby
def no_keywords(**nil) = 1
```

### result

```rbs
class Object
  def no_keywords: -> 1
end
```

## Infer return type from keyword arg

### update

```ruby
def foo(name:, age: 0) = name
foo(name: "Alice", age: 30)
```

### result

```rbs
class Object
  def foo: (name: String, ?age: Integer) -> String
end
```

## `**opts` enters scope as Hash[Symbol, untyped]

### update

```ruby
class A
  def self.opts(**opts) = opts
  def self.keys(**opts) = opts.keys
end
```

### result

```rbs
class A
  def self.opts: (**untyped opts) -> Hash[Symbol, untyped]
  def self.keys: (**untyped opts) -> Array[Symbol]
end
```

## Static keyword splat

### update

```ruby
def build(name:, count:) = [name, count]

def build_from_literal
  build(**{ name: "entry", count: 1 })
end

def build_from_local
  options = { name: "entry", count: 1 }
  build(**options)
end

def build_with_override
  options = { name: "entry" }
  build(**options, count: 1)
end

def build_from_merge
  options = { name: "entry" }
  build(**options.merge(count: 1))
end
```

### result

```rbs
class Object
  def build: (name: String, count: Integer) -> [String, Integer]
  def build_from_literal: -> [String, Integer]
  def build_from_local: -> [String, Integer]
  def build_with_override: -> [String, Integer]
  def build_from_merge: -> [String, Integer]
end
```

## Static keyword splat reaches super

### update

```ruby
class Parent
  attr_reader :name, :count

  def initialize(name:, count:)
    @name = name
    @count = count
  end
end

class Child < Parent
  def initialize(name:, count:)
    options = { name:, count: }
    super(**options)
  end
end

def child_values
  child = Child.new(name: "entry", count: 1)
  [child.name, child.count]
end
```

### result

```rbs
class Child < Parent
  def initialize: (name: String, count: Integer) -> void
end

class Object
  def child_values: -> ["entry", 1]
end

class Parent
  def name: -> "entry"
  def count: -> 1
  def initialize: (name: String, count: Integer) -> void
end
```

## Extra key in a keyword splat widens the param

### update

```ruby
def foo(check: false) = nil

opt = { foo: 1 }
foo(**opt)
```

### result

```rbs
class Object
  def foo: (?check: bool) -> nil
end
```
