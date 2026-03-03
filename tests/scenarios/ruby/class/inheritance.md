# Ruby / Class / Inheritance

## Basic inheritance

### update

```ruby
class Animal
  def speak = "..."
end

class Dog < Animal
  def bark = "woof"
end
```
### result

```rbs
class Animal
  def speak: -> "..."
end

class Dog < Animal
  def bark: -> "woof"
end
```

## Override method through inheritance

### update

```ruby
class Base
  def value = 0
end

class Child < Base
  def value = "hello"
end
```

### result

```rbs
class Base
  def value: -> 0
end

class Child < Base
  def value: -> "hello"
end
```

## Inheritance with initialize

### update

```ruby
class Shape
  def initialize(color)
    @color = color
  end
end

class Circle < Shape
  def initialize(color, radius)
    @color = color
    @radius = radius
  end
end

Circle.new("red", 5)
```

### result

```rbs
class Circle < Shape
  def initialize: (String color, Integer radius) -> void
end

class Shape
  def initialize: (untyped color) -> void
end
```

## Use absolute constant path as superclass

### update

```ruby
module A
  module B
    class C < ::B::C
      def foo = bar
    end
  end
end

module B
  class C
    def bar = 1
  end
end
```

### result

```rbs
class A::B::C < B::C
  def foo: -> 1
end

class B::C
  def bar: -> 1
end
```

## Subclass can call parent predicate method

### update

```ruby
class Base
  def initialize(detail)
    @detail = detail
  end

  def has_detail?
    !@detail.nil?
  end
end

class Child < Base
  def label
    has_detail? ? "yes" : "no"
  end
end
```

### result

```rbs
class Base
  def initialize: (untyped detail) -> void
  def has_detail?: -> bool
end

class Child < Base
  def label: -> "no" | "yes"
end
```

## Prefer top-level constant over same-name nested constant

### update

```ruby
class A
  def foo = 1
end

module B
  class A < A
    def bar = foo
  end
end
```

### result

```rbs
class A
  def foo: -> 1
end

class B::A < A
  def bar: -> 1
end
```

## Register Class.new(Parent) as subclass

### update

```ruby
class Parent
  def greet = "hi"
end

CONST = Class.new(Parent)

class User
  def run
    CONST.new.greet
  end
end
```

### result

```rbs
class Parent
  def greet: -> "hi"
end

class User
  def run: -> "hi"
end
```

## Class.new(Parent) inheritance in nested constant

### update

```ruby
class Parent
  def greet = "hi"
end

class Outer
  Nested = Class.new(Parent)
end

class User
  def run
    Outer::Nested.new.greet
  end
end
```

### result

```rbs
class Parent
  def greet: -> "hi"
end

class User
  def run: -> "hi"
end
```

## Resolve inherited methods on exception subclass

### update

```ruby
class Foo
  MyErr = Class.new(StandardError)

  def from_std
    StandardError.new("x").message
  end

  def from_runtime
    RuntimeError.new("x").message
  end

  def from_custom
    MyErr.new("y").message
  end
end
```

### result

```rbs
class Foo
  def from_std: -> String
  def from_runtime: -> String
  def from_custom: -> String
end
```

## Union class new inherits keyword init

### update

```ruby
class A
  def initialize(k:)
  end
end

class B < A
end

(rand < 0.5 ? A : B).new(k: 1)
```

### result

```rbs
class A
  def initialize: (k: Integer) -> void
end
```

## Super forwards the parent parameter type

### update

```ruby
class A
  def foo(x) = x + 1
end

class B < A
  def foo(x)
    super
  end
end

B.new.foo(1)
```

### result

```rbs
class A
  def foo: (untyped x) -> Integer
end

class B < A
  def foo: (Integer x) -> Integer
end
```
