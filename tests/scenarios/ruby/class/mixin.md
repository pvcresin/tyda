# Ruby / Class / Mixin

## include

### update

```ruby
module Greetable
  def greet = "hello"
end

class Person
  include Greetable

  def name = "Alice"
end
```

### result

```rbs
module Greetable
  def greet: -> "hello"
end

class Person
  include Greetable

  def name: -> "Alice"
end
```

## Resolve included module method across files

### update

`lib/formatter.rb`

```ruby
module Formatter
  def label = "label"
end
```

```ruby
class Item
  include Formatter

  def title = label
end
```

### result

```rbs
class Item
  include Formatter

  def title: -> "label"
end
```

## extend

### update

```ruby
module ClassMethods
  def class_method = "from module"
end

class MyClass
  extend ClassMethods
end
```

### result

```rbs
module ClassMethods
  def class_method: -> "from module"
end

class MyClass
  extend ClassMethods
end
```

## Include static module splat

### update

```ruby
module Named
  def name = "name"
end

module Labeled
  def label = "label"
end

class Entry
  PARTS = [Named, Labeled]

  include(*PARTS)

  def values = [name, label]
end
```

### result

```rbs
class Entry
  include Named
  include Labeled

  PARTS: [singleton(Named), singleton(Labeled)]

  def values: -> ["name", "label"]
end

module Labeled
  def label: -> "label"
end

module Named
  def name: -> "name"
end
```

## Extend local module splat

### update

```ruby
module Buildable
  def build = :build
end

module Countable
  def count = 1
end

class Registry
  helpers = [Buildable, Countable]

  extend(*helpers)
end

def read_build = Registry.build
def read_count = Registry.count
```

### result

```rbs
module Buildable
  def build: -> :build
end

module Countable
  def count: -> 1
end

class Object < BasicObject
  def read_build: -> :build
  def read_count: -> 1
end

class Registry
  extend Buildable
  extend Countable
end
```

## Include modules selected by a local collection

### update

```ruby
module Readable
  def read = :read
end

module Writable
  def write = :write
end

class Resource
  mixins = [Readable, Writable]
  include(*mixins)

  def values = [read, write]
end
```

### result

```rbs
module Readable
  def read: -> :read
end

class Resource
  include Readable
  include Writable

  def values: -> [:read, :write]
end

module Writable
  def write: -> :write
end
```

## Send prepend static module splat

### update

```ruby
module Wrapper
  def call = :wrapped
end

class Base
  def call = :base
end

class Service < Base
  WRAPPERS = [Wrapper]

  send(:prepend, *WRAPPERS)
end

def read_call = Service.new.call
```

### result

```rbs
class Base
  def call: -> :base
end

class Object < BasicObject
  def read_call: -> :wrapped
end

class Service < Base
  prepend Wrapper

  WRAPPERS: [singleton(Wrapper)]
end

module Wrapper
  def call: -> :wrapped
end
```

## prepend

### update

```ruby
module Prepended
  def method_name = "prepended"
end

class Base
  prepend Prepended

  def method_name = "original"
end
```

### result

```rbs
class Base
  prepend Prepended

  def method_name: -> "original"
end

module Prepended
  def method_name: -> "prepended"
end
```

## Resolve singleton mixin instance as class method

### update

```ruby
require "singleton"

module A
  class B
    include Singleton
  end
end

class C
  def foo = A::B.instance
end
```

### result

```rbs
class A::B
  include Singleton

  def self.instance: -> A::B
end

class C
  def foo: -> A::B
end
```

## Link module attr_reader to including class ivar

### update

```ruby
module Nameable
  attr_reader :name
end

class Person
  include Nameable

  def initialize
    @name = "Alice"
  end
end

class Check
  def run
    Person.new.name
  end
end
```

### result

```rbs
class Check
  def run: -> "Alice"
end

module Nameable
  def name: -> untyped
end

class Person
  include Nameable

  def initialize: -> void
end
```

## Read included module constant by bare name

### update

```ruby
module Rack
  module Utils
    STATUS_WITH_NO_ENTITY_BODY = { 200 => true }
  end

  class ContentType
    include Rack::Utils

    def no_entity? = STATUS_WITH_NO_ENTITY_BODY.key?(200)
    def table = STATUS_WITH_NO_ENTITY_BODY
  end
end
```

### result

```rbs
class Rack::ContentType
  include Rack::Utils

  def no_entity?: -> bool
  def table: -> Hash[200, true]
end

module Rack::Utils
  STATUS_WITH_NO_ENTITY_BODY: Hash[200, true]
end
```

## Prepend through a constant alias

### update

```ruby
module M
  def hello = :m
end

AliasM = M

class Foo
  prepend AliasM

  def hello = :foo
end

def f = Foo.new.hello
```

### result

```rbs
AliasM: singleton(M)

class Foo
  prepend M

  def hello: -> :foo
end

module M
  def hello: -> :m
end

class Object < BasicObject
  def f: -> :m
end
```

## Extend through a constant alias

### update

```ruby
module M
  def hello = :m
end

AliasM = M

class Foo
  extend AliasM
end

def f = Foo.hello
```

### result

```rbs
AliasM: singleton(M)

class Foo
  extend M
end

module M
  def hello: -> :m
end

class Object < BasicObject
  def f: -> :m
end
```

## include of a short name uses a just-included namespace

### update

```ruby
module Foo
  module Bar
    def hello = :hi
  end
end

class Baz
  include Foo
  include Bar
end

def f = Baz.new.hello
```

### result

```rbs
class Baz
  include Foo
  include Foo::Bar
end

module Foo::Bar
  def hello: -> :hi
end

class Object < BasicObject
  def f: -> :hi
end
```

## Included module constant wins over Kernel fallback

### update

```ruby
module MyConstants
  CONST = "mine"
end

module Kernel
  CONST = "kernel"
end

class Object
  include Kernel
end

module Foo
  include MyConstants

  def self.read = CONST
end
```

### result

```rbs
module Foo
  include MyConstants

  def self.read: -> "mine"
end

module Kernel
  CONST: "kernel"
end

module MyConstants
  CONST: "mine"
end

class Object < BasicObject
  include Kernel
end
```

## Bare constant in a module sees Object ancestor constants

### update

```ruby
module Kernel
  FOUND_ME = true
end

class Object
  include Kernel
end

module Foo
  def self.inside = FOUND_ME
end

def outside = Foo::FOUND_ME
```

### result

```rbs
module Foo
  def self.inside: -> true
end

module Kernel
  FOUND_ME: true
end

class Object < BasicObject
  include Kernel

  def outside: -> untyped
end
```

## Mixin lookup and ancestors follow application order

### update

```ruby
module IncludeFirst
  def value = 1
end

module IncludeSecond
  def value = 2
end

class IncludeHost
  include IncludeFirst
  include IncludeSecond
end

module PrependFirst
  def value = 1
end

module PrependSecond
  def value = 2
end

class PrependHost
  prepend PrependFirst
  prepend PrependSecond
end

module ExtendFirst
  def value = 1
end

module ExtendSecond
  def value = 2
end

class ExtendHost
  extend ExtendFirst
  extend ExtendSecond
end

module ArityFirst
  def value(arg) = 1
end

module AritySecond
  def value(x, y) = 2
end

class ArityHost
  include ArityFirst
  include AritySecond
end

module ConstantFirst
  VALUE = 1
end

module ConstantSecond
  VALUE = 2
end

class ConstantHost
  include ConstantFirst
  include ConstantSecond
end

class MixinProbe
  def include_value = IncludeHost.new.value
  def prepend_value = PrependHost.new.value
  def extend_value = ExtendHost.value
  def arity_value = ArityHost.new.value(1, 2)
  def constant_value = ConstantHost::VALUE
  def ancestors = IncludeHost.ancestors
  def first_ancestor = IncludeHost.ancestors.first
  def second_ancestor = IncludeHost.ancestors[1]
end
```

### result

```rbs
module ArityFirst
  def value: (untyped arg) -> 1
end

class ArityHost
  include ArityFirst
  include AritySecond
end

module AritySecond
  def value: (untyped x, untyped y) -> 2
end

module ConstantFirst
  VALUE: 1
end

class ConstantHost
  include ConstantFirst
  include ConstantSecond
end

module ConstantSecond
  VALUE: 2
end

module ExtendFirst
  def value: -> 1
end

class ExtendHost
  extend ExtendFirst
  extend ExtendSecond
end

module ExtendSecond
  def value: -> 2
end

module IncludeFirst
  def value: -> 1
end

class IncludeHost
  include IncludeFirst
  include IncludeSecond
end

module IncludeSecond
  def value: -> 2
end

class MixinProbe
  def include_value: -> 2
  def prepend_value: -> 2
  def extend_value: -> 2
  def arity_value: -> 2
  def constant_value: -> 2
  def ancestors: -> [singleton(IncludeHost), singleton(IncludeSecond), singleton(IncludeFirst), singleton(Object), singleton(Kernel), singleton(BasicObject)]
  def first_ancestor: -> singleton(IncludeHost)
  def second_ancestor: -> singleton(IncludeSecond)
end

module PrependFirst
  def value: -> 1
end

class PrependHost
  prepend PrependFirst
  prepend PrependSecond
end

module PrependSecond
  def value: -> 2
end
```
