# Ruby / Class / T::Struct

## Basic T::Struct

### update

```ruby
class User < T::Struct
  const :name, String
  const :age, Integer
end
```

### result

```rbs
class User < T::Struct
  def name: -> String
  def age: -> Integer
end
```

## T::Struct with readable and writable prop

### update

```ruby
class Config < T::Struct
  prop :host, String
  prop :port, Integer
  const :version, String
end
```

### result

```rbs
class Config < T::Struct
  def host: -> String
  def host=: (String host) -> String
  def port: -> Integer
  def port=: (Integer port) -> Integer
  def version: -> String
end
```

## T::Struct with method

### update

```ruby
class Point < T::Struct
  const :x, Integer
  const :y, Integer

  def to_s = "point"
end
```

### result

```rbs
class Point < T::Struct
  def x: -> Integer
  def y: -> Integer
  def to_s: -> "point"
end
```

## prop forward-reference resolves to nested class over top-level

### update

```ruby
module A
  class B < T::Struct
    prop :foo, Foo

    class Foo < T::Struct
      prop :x, Integer
    end
  end

  module Foo
  end
end
```

### result

```rbs
class A::B < T::Struct
  def foo: -> A::B::Foo
  def foo=: (A::B::Foo foo) -> A::B::Foo
end

class A::B::Foo < T::Struct
  def x: -> Integer
  def x=: (Integer x) -> Integer
end
```

## prop falls back to top-level when no nested match

### update

```ruby
class Foo < T::Struct
  const :x, Integer
end

module A
  class B < T::Struct
    const :foo, Foo
  end
end
```

### result

```rbs
class A::B < T::Struct
  def foo: -> Foo
end

class Foo < T::Struct
  def x: -> Integer
end
```

## prop with absolute reference skips lexical scope

### update

```ruby
class Foo < T::Struct
  const :x, Integer
end

module A
  class Foo < T::Struct
    const :y, String
  end

  class B < T::Struct
    const :foo, ::Foo
  end
end
```

### result

```rbs
class A::B < T::Struct
  def foo: -> Foo
end

class A::Foo < T::Struct
  def y: -> String
end

class Foo < T::Struct
  def x: -> Integer
end
```

## prop resolves to outer namespace constant

### update

```ruby
module A
  class Foo < T::Struct
    const :x, Integer
  end

  class B < T::Struct
    const :foo, Foo
  end
end
```

### result

```rbs
class A::B < T::Struct
  def foo: -> A::Foo
end

class A::Foo < T::Struct
  def x: -> Integer
end
```
