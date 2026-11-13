# Ruby / Method / Optional Args

## Optional arg inferred from default

### update

```ruby
def foo(x, y = 1) = x
foo("hello")
```

### result

```rbs
class Object < BasicObject
  def foo: [T] (T x, ?Integer y) -> T
end
```

## Optional arg with no calls

### update

```ruby
def foo(x, y = "default") = y
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x, ?String y) -> String
end
```

## Splat arg

### update

```ruby
def foo(*args) = args
foo(1, 2, 3)
```

### result

```rbs
class Object < BasicObject
  def foo: (*Integer args) -> Array[Integer]
end
```

## Combine required optional and splat args

### update

```ruby
def foo(x, y = "default", *rest) = rest
foo(1, "hello", :a, :b)
```

### result

```rbs
class Object < BasicObject
  def foo: (Integer x, ?String y, *Symbol rest) -> Array[Symbol]
end
```

## Infer return type from optional arg

### update

```ruby
def foo(x = 42) = x
```

### result

```rbs
class Object < BasicObject
  def foo: (?Integer x) -> Integer
end
```

## Block arg

### update

```ruby
def foo(&block) = 42
```

### result

```rbs
class Object < BasicObject
  def foo: () ?{ (?) -> untyped } -> 42
end
```

## Default argument constant uses the enclosing lexical scope

### update

```ruby
module Foo
  CONST = 3

  def self.f(x = CONST) = x
end

def g = Foo.f
```

### result

```rbs
module Foo
  CONST: 3

  def self.f: (?Integer x) -> Integer
end

class Object < BasicObject
  def g: -> Integer
end
```

## Splat arg element read keeps the element type

### update

```ruby
class SplatReader
  def head(*values) = values.first
end

SplatReader.new.head("a")
```

### result

```rbs
class SplatReader
  def head: (*String values) -> String?
end
```

## Splat argument after required args fills the optional parameter

### update

```ruby
class Runner
  def run(a, b, c = :default) = c
end

items = ["x"].to_a
Runner.new.run(:a, :b, *items)
```

### result

```rbs
class Runner
  def run: (Symbol a, Symbol b, ?(String | Symbol) c) -> (String | :default)
end
```

## Splat argument that covers a required arg can also fill the optional parameter

### update

```ruby
class Runner
  def run(a, b, c = 1) = c
end

items = ["x"].to_a
Runner.new.run(:a, *items)
```

### result

```rbs
class Runner
  def run: (Symbol a, String b, ?(Integer | String) c) -> (String | 1)
end
```
