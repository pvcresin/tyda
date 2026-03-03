# Ruby / Variable / Constant Resolution

## Top-level constant is visible from deep nesting

### update

```ruby
TOP = 1

module Outer
  module Inner
    class A
      def f = TOP
    end
  end
end
```

### result

```rbs
TOP: 1

class Outer::Inner::A
  def f: -> 1
end
```

## Inner constant shadows same-name outer constant

### update

```ruby
CONST = "top"

module M
  CONST = "inner"
  def self.f = CONST
end

class N
  def g = CONST
end
```

### result

```rbs
CONST: "top"

module M
  CONST: "inner"

  def self.f: -> "inner"
end

class N
  def g: -> "top"
end
```

## Absolute `::CONST` resolves from top level

### update

```ruby
CONST = "top"

module M
  CONST = "inner"
  def self.f = ::CONST
end
```

### result

```rbs
CONST: "top"

module M
  CONST: "inner"

  def self.f: -> "top"
end
```

## Nested scope cannot see sibling bare constant

### update

```ruby
module A
  ACONST = 1
end

module B
  def self.f = ACONST
end
```

### result

```rbs
module A
  ACONST: 1
end

module B
  def self.f: -> untyped
end
```

## Included module constant is visible through ancestors

### update

```ruby
module Helper
  HCONST = "h"
end

class C
  include Helper

  def f = HCONST
end
```

### result

```rbs
class C
  include Helper

  def f: -> "h"
end

module Helper
  HCONST: "h"
end
```

## Superclass constant is visible through ancestors

### update

```ruby
class P
  PCONST = 7
end

class Q < P
  def f = PCONST
end
```

### result

```rbs
class P
  PCONST: 7
end

class Q < P
  def f: -> 7
end
```

## Qualified `Outer::CONST` follows enclosing scopes

### update

```ruby
module Outer
  module Inner
    CONST = "inner"
  end

  class C
    def f = Inner::CONST
  end
end
```

### result

```rbs
class Outer::C
  def f: -> "inner"
end

module Outer::Inner
  CONST: "inner"
end
```

## Same-name sibling class resolves by call-site nesting

### update

```ruby
class Foo
  def kind = :top
end

module Inner
  class Foo
    def kind = :inner
  end

  def self.make = Foo.new
end

def make_top = Foo.new
```

### result

```rbs
class Foo
  def kind: -> :top
end

module Inner
  def self.make: -> Inner::Foo
end

class Inner::Foo
  def kind: -> :inner
end

class Object
  def make_top: -> Foo
end
```

## Nested same-name constant prefers inner scope

### update

```ruby
NAME = "top"

module Outer
  NAME = "outer"

  module Inner
    NAME = "inner"
    def self.f = NAME
  end

  def self.g = NAME
end

def h = NAME
```

### result

```rbs
NAME: "top"

class Object
  def h: -> "top"
end

module Outer
  NAME: "outer"

  def self.g: -> "outer"
end

module Outer::Inner
  NAME: "inner"

  def self.f: -> "inner"
end
```

## Reopened parent module resolves by nesting

### update

```ruby
module Outer
  CONST = 1
end

module Outer
  def self.f = CONST
end
```

### result

```rbs
module Outer
  CONST: 1

  def self.f: -> 1
end
```

## Deep reopened module resolves inner constant

### update

```ruby
module Outer
  module Inner
    CONST = "x"
  end
end

module Outer
  module Inner
    def self.f = CONST
  end
end
```

### result

```rbs
module Outer::Inner
  CONST: "x"

  def self.f: -> "x"
end
```

## Resolve included module constant as `Mod::CONST`

### update

```ruby
module M
  MCONST = 5
end

class C
  include M

  def f = M::MCONST
end
```

### result

```rbs
class C
  include M

  def f: -> 5
end

module M
  MCONST: 5
end
```

## Resolve superclass constant by qualified reference

### update

```ruby
class P
  PCONST = "p"
end

class Q < P
  def f = P::PCONST
end
```

### result

```rbs
class P
  PCONST: "p"
end

class Q < P
  def f: -> "p"
end
```

## Class body resolves constants by nesting

### update

```ruby
module Outer
  CONST = 99

  class C
    INSIDE = CONST
  end
end
```

### result

```rbs
module Outer
  CONST: 99
end

class Outer::C
  INSIDE: 99
end
```

## Lexical constant wins over same-name superclass constant

### update

```ruby
class P
  CONST = "p"
end

module Ns
  CONST = "ns"

  class C < P
    def f = CONST
  end
end
```

### result

```rbs
module Ns
  CONST: "ns"
end

class Ns::C < P
  def f: -> "ns"
end

class P
  CONST: "p"
end
```

## Explicit `Parent::CONST` reads superclass constant

### update

```ruby
class P
  CONST = "p"
end

module Ns
  CONST = "ns"

  class C < P
    def f = P::CONST
  end
end
```

### result

```rbs
module Ns
  CONST: "ns"
end

class Ns::C < P
  def f: -> "p"
end

class P
  CONST: "p"
end
```

## `class A::B::C` reads own lexical and ancestor constants

### update

```ruby
module A
  module B
    BCONST = "b"
  end
end

class A::B::C
  def f = BCONST
end
```

### result

```rbs
module A::B
  BCONST: "b"
end

class A::B::C
  def f: -> "b"
end
```

## Nested form sees outer module constant

### update

```ruby
module A
  CONST = "a"

  module B
    def self.f = CONST
  end
end
```

### result

```rbs
module A
  CONST: "a"
end

module A::B
  def self.f: -> "a"
end
```

## `class << self` body sees outer class constant

### update

```ruby
class A
  CONST = "v"

  class << self
    def f = CONST
  end
end
```

### result

```rbs
class A
  CONST: "v"

  def self.f: -> "v"
end
```

## Lexical constant wins over included module constant

### update

```ruby
module Inc
  V = "inc"
end

class P
  V = "p"
end

module Ns
  V = "ns"

  class C < P
    include Inc

    def f = V
  end
end
```

### result

```rbs
module Inc
  V: "inc"
end

module Ns
  V: "ns"
end

class Ns::C < P
  include Inc

  def f: -> "ns"
end

class P
  V: "p"
end
```

## Superclass constant is visible when lexical and include miss

### update

```ruby
class P
  V = "p"
end

module Ns
  class C < P
    def f = V
  end
end
```

### result

```rbs
class Ns::C < P
  def f: -> "p"
end

class P
  V: "p"
end
```

## Included module constant wins over superclass constant

### update

```ruby
module Inc
  V = "inc"
end

class P
  V = "p"
end

module Ns
  class C < P
    include Inc

    def f = V
  end
end
```

### result

```rbs
module Inc
  V: "inc"
end

class Ns::C < P
  include Inc

  def f: -> "inc"
end

class P
  V: "p"
end
```

## Same short name in many namespaces stays in each nesting

### update

```ruby
module A
  class Item
    def label = "a"
  end

  def self.make = Item.new
end

module B
  class Item
    def label = "b"
  end

  def self.make = Item.new
end
```

### result

```rbs
module A
  def self.make: -> A::Item
end

class A::Item
  def label: -> "a"
end

module B
  def self.make: -> B::Item
end

class B::Item
  def label: -> "b"
end
```

## Resolve later class body constant from self::CONST and default arg

### update

```ruby
class Holder
  attr_reader :fallback

  def self.empty
    self::EMPTY
  end

  def initialize(value, fallback = EMPTY)
    @value = value
    @fallback = fallback
  end

  class EmptyHolder < Holder
    def initialize
    end
  end

  EMPTY = EmptyHolder.new
end
```

### result

```rbs
class Holder
  EMPTY: Holder::EmptyHolder

  def fallback: -> Holder::EmptyHolder
  def self.empty: -> Holder::EmptyHolder
  def initialize: (untyped value, ?Holder::EmptyHolder fallback) -> void
end

class Holder::EmptyHolder < Holder
  def initialize: -> void
end
```

## Resolve later constant from method body

### update

```ruby
class Catalog
  def title
    TITLE
  end

  def title_with(value)
    TITLE
  end

  TITLE = "catalog"
end
```

### result

```rbs
class Catalog
  TITLE: "catalog"

  def title: -> "catalog"
  def title_with: (untyped value) -> "catalog"
end
```

## Apply later constant through initialize ivar to attr_reader

### update

```ruby
class Holder
  attr_reader :item

  def initialize
    @item = ITEM
  end

  ITEM = "item"
end
```

### result

```rbs
class Holder
  ITEM: "item"

  def item: -> "item"
  def initialize: -> void
end
```

## Resolve later nested class from short name in method body

### update

```ruby
class Container
  def build
    Item.new
  end

  class Item
  end
end
```

### result

```rbs
class Container
  def build: -> Container::Item
end
```

## Resolve later outer namespace constant from nested method body

### update

```ruby
class Namespace
  class Entry
    def label
      LABEL
    end
  end

  LABEL = "entry"
end
```

### result

```rbs
class Namespace
  LABEL: "entry"
end

class Namespace::Entry
  def label: -> "entry"
end
```

## Resolve later class body constant from class << self method body

### update

```ruby
class Registry
  class << self
    def empty
      self::EMPTY
    end
  end

  class EmptyRegistry
  end

  EMPTY = EmptyRegistry.new
end
```

### result

```rbs
class Registry
  EMPTY: Registry::EmptyRegistry

  def self.empty: -> Registry::EmptyRegistry
end
```

## Resolve later included constant from method body

### update

```ruby
module Source
  ITEM = "source"
end

class Receiver
  def item
    ITEM
  end

  include Source
end
```

### result

```rbs
class Receiver
  include Source

  def item: -> "source"
end

module Source
  ITEM: "source"
end
```

## Resolve later case branch constants from method body and default arg

### update

```ruby
class Choice
  def label(value = LABEL)
    LABEL
  end

  case :primary
  when :primary
    LABEL = "choice"
  end
end
```

### result

```rbs
class Choice
  LABEL: "choice"

  def label: (?String value) -> "choice"
end
```

## Resolve later pattern branch constant from method body

### update

```ruby
class MatchBox
  def label
    LABEL
  end

  case [1]
  in [1]
    LABEL = "match"
  end
end
```

### result

```rbs
class MatchBox
  LABEL: "match"

  def label: -> "match"
end
```

## Resolve later begin ensure included constant from method body and default arg

### update

```ruby
module Provider
  TOKEN = "provider"
end

class Client
  def token(value = TOKEN)
    TOKEN
  end

  begin
  rescue StandardError
  ensure
    include Provider
  end
end
```

### result

```rbs
class Client
  include Provider

  def token: (?String value) -> "provider"
end

module Provider
  TOKEN: "provider"
end
```

## Resolve later destructured constant from method body

### update

```ruby
class Box
  def value
    VALUE
  end

  VALUE, OTHER = ["direct", "other"]
end
```

### result

```rbs
class Box
  OTHER: "other"
  VALUE: "direct"

  def value: -> "direct"
end
```

## Resolve later nested destructured constant from method body

### update

```ruby
class Box
  def value
    VALUE
  end

  GROUP, (VALUE, OTHER) = ["group", ["inner", "other"]]
end
```

### result

```rbs
class Box
  GROUP: "group"
  OTHER: "other"
  VALUE: "inner"

  def value: -> "inner"
end
```

## Resolve later const_set constant from method body

### update

```ruby
class Box
  def value
    self.class::VALUE
  end

  const_set(:VALUE, "static")
end
```

### result

```rbs
class Box
  VALUE: "static"

  def value: -> "static"
end
```

## Resolve later const_set path from method body

### update

```ruby
class Box
  def value
    VALUE
  end
end

Box.const_set(:VALUE, "path")
```

### result

```rbs
class Box
  VALUE: "path"

  def value: -> "path"
end
```

## Resolve const_set in branch using class body local

### update

```ruby
class Box
  def value
    self.class::VALUE
  end

  value = "branch"
  if true
    const_set(:VALUE, value)
  end
end
```

### result

```rbs
class Box
  VALUE: "branch"

  def value: -> "branch"
end
```

## const_get resolves nested class from static symbol name

### update

```ruby
module Container
  class Item
    def label = "item"
  end
end

class Reader
  def self.label
    Container.const_get(:Item).new.label
  end
end
```

### result

```rbs
class Container::Item
  def label: -> "item"
end

class Reader
  def self.label: -> "item"
end
```

## const_get resolves static to_sym name

### update

```ruby
module Container
  class Item
    def label = "item"
  end

  def self.label
    name = "Item"
    const_get("#{name}".to_sym, false).new.label
  end
end
```

### result

```rbs
module Container
  def self.label: -> "item"
end

class Container::Item
  def label: -> "item"
end
```

## const_get resolves static name values

### update

```ruby
module Container
  NAME = "Item"

  class Item
    def label = "item"
  end

  class Other
    def label = "other"
  end

  def self.local_label
    name = "Item"
    const_get(name, false).new.label
  end

  def self.symbol_label
    name = :Other
    const_get(name, false).new.label
  end

  def self.constant_label
    const_get(NAME, false).new.label
  end

  def self.send_label
    name = "Item"
    send(:const_get, name, false).new.label
  end
end
```

### result

```rbs
module Container
  NAME: "Item"

  def self.local_label: -> "item"
  def self.symbol_label: -> "other"
  def self.constant_label: -> "item"
  def self.send_label: -> "item"
end

class Container::Item
  def label: -> "item"
end

class Container::Other
  def label: -> "other"
end
```

## constants feeds static const_get

### update

```ruby
module Container
  VALUE = "value"

  class Item
    def self.label = "item"
  end

  class Entry
    def self.label = "entry"
  end

  def self.local_labels
    constants(false).map { |name| const_get(name, false) }.grep(Class).map(&:label)
  end
end

class Reader
  def self.names = Container.constants(false)

  def self.labels
    Container.constants(false).map { |name| Container.const_get(name, false) }.grep(Class).map(&:label)
  end
end
```

### result

```rbs
module Container
  VALUE: "value"

  def self.local_labels: -> Array["entry" | "item"]
end

class Container::Entry
  def self.label: -> "entry"
end

class Container::Item
  def self.label: -> "item"
end

class Reader
  def self.names: -> Array[:Entry | :Item | :VALUE]
  def self.labels: -> Array["entry" | "item"]
end
```

## const_get resolves static constant paths

### update

```ruby
module Container
  class Item
    class Detail
      def label = "detail"
    end
  end

  def self.relative_label
    const_get("Item::Detail", false).new.label
  end

  def self.interpolated_label
    name = "Detail"
    const_get("Item::#{name}", false).new.label
  end
end

def absolute_label
  Object.const_get("::Container::Item::Detail").new.label
end
```

### result

```rbs
module Container
  def self.relative_label: -> "detail"
  def self.interpolated_label: -> "detail"
end

class Container::Item::Detail
  def label: -> "detail"
end

class Object < BasicObject
  def absolute_label: -> "detail"
end
```

## const_defined? resolves static name values

### update

```ruby
module Container
  NAME = "Item"

  class Item
  end

  def self.has_local
    name = NAME
    const_defined?(name, false)
  end

  def self.has_absolute
    Object.const_defined?("::Container::Item")
  end

  def self.has_send
    name = NAME
    send(:const_defined?, name, false)
  end
end
```

### result

```rbs
module Container
  NAME: "Item"

  def self.has_local: -> true
  def self.has_absolute: -> true
  def self.has_send: -> true
end
```

## const_get resolves constant value from static string name

### update

```ruby
module Container
  VALUE = "value"
end

class Reader
  def self.value
    Container.const_get("VALUE", false)
  end
end
```

### result

```rbs
module Container
  VALUE: "value"
end

class Reader
  def self.value: -> "value"
end
```

## Bare const_get resolves from class object self

### update

```ruby
module Container
  class Item
    def label = "item"
  end

  def self.build
    const_get(:Item, false).new.label
  end
end
```

### result

```rbs
module Container
  def self.build: -> "item"
end

class Container::Item
  def label: -> "item"
end
```

## const_get false does not fall back to ancestor constants

### update

```ruby
class Parent
  VALUE = 1
end

class Child < Parent
  def self.inherited_value = const_get(:VALUE)
  def self.direct_value = const_get(:VALUE, false)
end
```

### result

```rbs
class Child < Parent
  def self.inherited_value: -> 1
  def self.direct_value: -> untyped
end

class Parent
  VALUE: 1
end
```

## const_defined? returns true for static known constant

### update

```ruby
module Container
  class Item
  end

  def self.has_item
    const_defined?(:Item, false)
  end
end
```

### result

```rbs
module Container
  def self.has_item: -> true
end
```

## const_get through send resolves static constant name

### update

```ruby
module Container
  class Item
    def label = "item"
  end
end

class Reader
  def self.label
    Container.send(:const_get, :Item, false).new.label
  end
end
```

### result

```rbs
class Container::Item
  def label: -> "item"
end

class Reader
  def self.label: -> "item"
end
```
