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
  def foo: (String x, ?Integer y) -> String
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

## Unknown positional arguments do not become their defaults

### update

```ruby
def positional_sink(value) = value
def keyword_sink(value) = value

def positional_default(value = nil, marker = 0) = positional_sink(value)
def keyword_default(value: nil, marker: 0) = keyword_sink(value)
def positional_non_nil_default(value = "fallback") = value
def keyword_non_nil_default(value: "fallback") = value

positional_default(T.unsafe("ignored"), 1)
keyword_default(value: T.unsafe("ignored"), marker: 1)
positional_non_nil_default(T.unsafe("ignored"))
keyword_non_nil_default(value: T.unsafe("ignored"))
```

### result

```rbs
class Object < BasicObject
  def positional_sink: (untyped value) -> untyped
  def keyword_sink: (untyped value) -> untyped
  def positional_default: (?untyped value, ?Integer marker) -> untyped
  def keyword_default: (?value: untyped, ?marker: Integer) -> untyped
  def positional_non_nil_default: (?untyped value) -> untyped
  def keyword_non_nil_default: (?value: untyped) -> untyped
end
```

## Unknown and known calls stay unioned

### update

```ruby
def unknown_and_known(value) = value

unknown_and_known("known")
unknown_and_known(T.unsafe("unknown"))
```

### result

```rbs
class Object < BasicObject
  def unknown_and_known: ((String | untyped) value) -> (String | untyped)
end
```

## Unknown and known keyword calls stay unioned with the default

### update

```ruby
def unknown_and_known_keyword(value: "fallback") = value

unknown_and_known_keyword(value: "known")
unknown_and_known_keyword(value: T.unsafe("unknown"))
```

### result

```rbs
class Object < BasicObject
  def unknown_and_known_keyword: (?value: (String | untyped)) -> (String | untyped)
end
```

## Unknown array elements stay in the inferred argument

### update

```ruby
def unknown_and_known_arrays(value) = value

unknown_and_known_arrays(["known"])
unknown_and_known_arrays([T.unsafe("unknown")])
```

### result

```rbs
class Object < BasicObject
  def unknown_and_known_arrays: (Array[String | untyped] value) -> Array[String | untyped]
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
  def foo: (?untyped &block) -> 42
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
