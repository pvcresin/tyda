# Ruby / Class / Absolute path declaration

## Top-level absolute class declaration

### update

```ruby
class ::Foo::Bar
  def greet = "hi"
end
```

### result

```rbs
class Foo::Bar
  def greet: -> "hi"
end
```

## Absolute class declaration inside a module

### update

```ruby
module M
  class ::Top
    def t = 1
  end
end
```

### result

```rbs
class Top
  def t: -> 1
end
```

## Absolute module declaration inside a module

### update

```ruby
module Outer
  module ::Global
    def g = :ok
  end
end
```

### result

```rbs
module Global
  def g: -> :ok
end
```

## Absolute class inside a module nests inner class under the top-level class

### update

```ruby
module Foo
  class ::Bar
    class Baz
      def self.ok = 1
    end
  end
end

def f = Bar::Baz.ok
```

### result

```rbs
class Bar::Baz
  def self.ok: -> 1
end

class Object
  def f: -> 1
end
```

## Absolute class inside a module still sees the module constant

### update

```ruby
module Foo
  CONST = 42

  class ::Bar
    def self.from_lexical = CONST
  end
end

def f = Bar.from_lexical
```

### result

```rbs
class Bar
  def self.from_lexical: -> 42
end

module Foo
  CONST: 42
end

class Object
  def f: -> 42
end
```

## Absolute class inside compact nesting stays top-level and lexical

### update

```ruby
module Foo
  CONST = 7

  class Bar
    class ::Quuux
      def self.v = CONST
    end
  end
end

def f = Quuux.v
```

### result

```rbs
module Foo
  CONST: 7
end

class Object
  def f: -> 7
end

class Quuux
  def self.v: -> 7
end
```
