# Ruby / Class / Dynamic Namespace

## Register block def in `A = Class.new` as class method

### update

```ruby
A = Class.new do
  def hello = :hi
end

def f = A.new
def g = A.new.hello
```

### result

```rbs
class A
  def hello: -> :hi
end

class Object < BasicObject
  def f: -> A
  def g: -> :hi
end
```

## Register inheritance and block def in `A = Class.new(P)`

### update

```ruby
class P
  def hello = :parent
end

A = Class.new(P) do
  def world = :child
end

def f = A.new.hello
def g = A.new.world
```

### result

```rbs
class A < P
  def world: -> :child
end

class Object < BasicObject
  def f: -> :parent
  def g: -> :child
end

class P
  def hello: -> :parent
end
```

## Register block def in Module.new as module method

### update

```ruby
M = Module.new do
  def hello = :hi
end

class C
  include M
end

def f = C.new.hello
```

### result

```rbs
class C
  include M
end

module M
  def hello: -> :hi
end

class Object < BasicObject
  def f: -> :hi
end
```

## Register block def in nested Class.new

### update

```ruby
class Box
  class Parent
    def parent = :parent
  end

  Item = Class.new(Parent) do
    def child = :child
  end
end

def f = Box::Item.new.parent
def g = Box::Item.new.child
```

### result

```rbs
class Box::Item < Box::Parent
  def child: -> :child
end

class Box::Parent
  def parent: -> :parent
end

class Object < BasicObject
  def f: -> :parent
  def g: -> :child
end
```

## Resolve nested Module.new block def from include target

### update

```ruby
class Box
  Shared = Module.new do
    def label = "label"
  end
end

class Item
  include Box::Shared

  def value = label
end
```

### result

```rbs
module Box::Shared
  def label: -> "label"
end

class Item
  include Box::Shared

  def value: -> "label"
end
```

## Register const_set Class.new block

### update

```ruby
class Base
  def base = :base
end

module Container
  const_set(:Item, Class.new(Base) do
    def label = "item"
  end)
end

def read_base = Container::Item.new.base
def read_label = Container::Item.new.label
```

### result

```rbs
class Base
  def base: -> :base
end

class Container::Item < Base
  def label: -> "item"
end

class Object < BasicObject
  def read_base: -> :base
  def read_label: -> "item"
end
```

## Register const_set Module.new block

### update

```ruby
class Box
  const_set("Shared", Module.new do
    def label = "shared"
  end)
end

class Entry
  include Box::Shared

  def value = label
end
```

### result

```rbs
module Box::Shared
  def label: -> "shared"
end

class Entry
  include Box::Shared

  def value: -> "shared"
end
```

## Register send const_set Class.new block

### update

```ruby
class Box
  send(:const_set, :Entry, Class.new do
    attr_reader :name

    def initialize(name)
      @name = name
    end
  end)
end

def read_name = Box::Entry.new("entry").name
```

### result

```rbs
class Box::Entry
  def name: -> "entry"
  def initialize: (String name) -> void
end

class Object < BasicObject
  def read_name: -> "entry"
end
```

## Class.new constants attach to the outer lexical scope

### update

```ruby
Foo = Class.new do
  CONST = 2
end

def f = CONST
def g = Foo::CONST
```

### result

```rbs
CONST: 2

class Object < BasicObject
  def f: -> 2
  def g: -> untyped
end
```

## Nested class inside Class.new uses the outer lexical scope

### update

```ruby
class Foo
  Bar = Class.new do
    class Baz
      def self.v = 1
    end
  end
end

def f = Foo::Baz.v
```

### result

```rbs
class Foo::Baz
  def self.v: -> 1
end

class Object < BasicObject
  def f: -> 1
end
```

## Class.new self-qualified constant belongs to the assigned class

### update

```ruby
Foo = Class.new do
  self::CONST = 2
end

def f = Foo::CONST
```

### result

```rbs
class Foo
  CONST: 2
end

class Object < BasicObject
  def f: -> 2
end
```

## Class.new self-qualified nested class belongs to the assigned class

### update

```ruby
class Foo
  Bar = Class.new do
    class self::Baz
      def self.v = 1
    end
  end
end

def f = Foo::Bar::Baz.v
```

### result

```rbs
class Foo::Bar::Baz
  def self.v: -> 1
end

class Object < BasicObject
  def f: -> 1
end
```

## Nested Class.new constants still attach to the outer lexical scope

### update

```ruby
Foo = Class.new do
  Class.new do
    CONST = 1
  end
end

def f = CONST
def g = Foo::CONST
```

### result

```rbs
CONST: 1

class Object < BasicObject
  def f: -> 1
  def g: -> untyped
end
```

## Singleton class inside Class.new keeps constants off the assigned class

### update

```ruby
Foo = Class.new do
  class << self
    CONST = 1

    def bar = CONST
  end
end

def f = Foo.bar
def g = Foo::CONST
```

### result

```rbs
class Foo
  def self.bar: -> 1
end

class Object < BasicObject
  def f: -> 1
  def g: -> untyped
end
```

## Class.new block parameter is the new singleton class

### update

```ruby
def parented = Class.new(StandardError) { |c| c }
```

### result

```rbs
class Object < BasicObject
  def parented: -> singleton(StandardError)
end
```

## Self-qualified nested class path

### update

```ruby
class A
  class self::B::C
    def foo = 1
  end
end
```

### result

```rbs
class A::B::C
  def foo: -> 1
end
```
