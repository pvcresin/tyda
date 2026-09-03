# Sorbet / Sig / Basic

## Parameter type without sig

```ruby
class Foo
  sig { params(x: Integer).returns(String) }
  def convert(x) = x.to_s
end
```

### result

```rbs
class Foo
  def convert: (Integer x) -> String
end
```

## sig void method

```ruby
class Foo
  sig { void }
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

## sig with multiple params

```ruby
class Foo
  sig { params(name: String, age: Integer).returns(String) }
  def greet(name, age) = "#{name}: #{age}"
end
```

### result

```rbs
class Foo
  def greet: (String name, Integer age) -> String
end
```

## sig returns with no args

```ruby
class Foo
  sig { returns(Integer) }
  def count = 42
end
```

### result

```rbs
class Foo
  def count: -> Integer
end
```

## Class method with sig

```ruby
class Foo
  sig { params(n: Integer).returns(Integer) }
  def self.double(n) = n * 2
end
```

### result

```rbs
class Foo
  def self.double: (Integer n) -> Integer
end
```

## Multiple methods with sig

```ruby
class Calc
  sig { params(x: Integer, y: Integer).returns(Integer) }
  def add(x, y) = x + y

  sig { params(x: Integer, y: Integer).returns(Integer) }
  def sub(x, y) = x - y
end
```

### result

```rbs
class Calc
  def add: (Integer x, Integer y) -> Integer
  def sub: (Integer x, Integer y) -> Integer
end
```

## Method without sig uses code inference

```ruby
class Foo
  sig { returns(Integer) }
  def typed_method = 42

  def untyped_method = "hello"
end
```

### result

```rbs
class Foo
  def typed_method: -> Integer
  def untyped_method: -> "hello"
end
```

## Top-level sig

```ruby
sig { params(x: Integer).returns(String) }
def convert(x) = x.to_s
```

### result

```rbs
class Object < BasicObject
  def convert: (Integer x) -> String
end
```
