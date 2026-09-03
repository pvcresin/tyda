# Ruby / Class / Singleton Class Of Const

## `class << SomeConst` opens singleton class

### update

```ruby
class A
end

class << A
  def hello = :singleton
end

def f = A.hello
```

### result

```rbs
class A
  def self.hello: -> :singleton
end

class Object < BasicObject
  def f: -> :singleton
end
```

## Nested singleton class methods are not on the class

### update

```ruby
class A
end

class << A
  class << self
    def deep = :deep
  end
end

def f = A.deep
```

### result

```rbs
class Object < BasicObject
  def f: -> untyped
end
```

## Constants inside `class << X` are not class constants

### update

```ruby
class A
end

class << A
  CONST = 7
end

def f = A::CONST
```

### result

```rbs
class Object < BasicObject
  def f: -> untyped
end
```

## `singleton_class.define_method` registers class method

### update

```ruby
class Builder
  singleton_class.define_method(:build) do |value|
    value.to_s
  end
end

def build_value = Builder.build(1)
```

### result

```rbs
class Builder
  def self.build: (Integer value) -> String
end

class Object < BasicObject
  def build_value: -> String
end
```

## Constant `singleton_class.define_method` registers class method

### update

```ruby
class Runner
end

Runner.singleton_class.define_method(:run) { :done }

def run_value = Runner.run
```

### result

```rbs
class Object < BasicObject
  def run_value: -> :done
end

class Runner
  def self.run: -> :done
end
```

## `singleton_class.attr_reader` returns class ivar

### update

```ruby
class Store
  @name = "store"

  singleton_class.attr_reader :name
end

def store_name = Store.name
```

### result

```rbs
class Object < BasicObject
  def store_name: -> "store"
end

class Store
  def self.name: -> "store"
end
```

## `singleton_class.alias_method` reuses class method

### update

```ruby
class Label
  def self.name = "label"

  singleton_class.alias_method :title, :name
end

def label_title = Label.title
```

### result

```rbs
class Label
  def self.name: -> "label"
  alias self.title self.name
end

class Object < BasicObject
  def label_title: -> "label"
end
```

## `singleton_class.class_eval` registers class method

### update

```ruby
class Factory
  singleton_class.class_eval do
    def make = :item
  end
end

def make_item = Factory.make
```

### result

```rbs
class Factory
  def self.make: -> :item
end

class Object < BasicObject
  def make_item: -> :item
end
```

## Nested singleton methods are not callable on the class

### update

```ruby
class Foo
  class << self
    def on_foo = 1

    class << self
      def on_eigen = 2
    end
  end
end

def f = Foo.on_foo
def g = Foo.on_eigen
```

### result

```rbs
class Foo
  def self.on_foo: -> 1
end

class Object < BasicObject
  def f: -> 1
  def g: -> untyped
end
```

## Singleton-class constant is not visible as a class constant

### update

```ruby
class Foo
  class << self
    A = 1

    def bar = A
  end

  def self.baz = A
end
```

### result

```rbs
class Foo
  def self.bar: -> 1
  def self.baz: -> untyped
end
```
