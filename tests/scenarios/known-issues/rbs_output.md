# Known Issues / RBS Output

## define_method with a computed name in a multi-line call

### update

```ruby
module Worker
  define_method(
    ::Other::Worker.method_defined?(:process_work) ? :process_work : :do_work,
  ) do |msg|
    msg
  end
end
```

### result

```rbs
module Worker
  def do_work: (untyped msg) -> untyped
  def process_work: (untyped msg) -> untyped
end
```

## Core class name shadowed by a nested module

### update

```ruby
module Util
  module Hash
    def self.deep(h) = h
  end
  def self.blank = {}
end
```

### result

```rbs
module Util
  def self.blank: -> ::Hash[untyped, untyped]
end

module Util::Hash
  def self.deep: [H] (H h) -> H
end
```

## Unknown constant read through self is not a type

### update

```ruby
class Proc3
  def self.call(source)
    source if source.is_a?(String) || source.is_a?(self::ACCUMULATOR)
  end
  def self.setup(k) = const_set(:ACCUMULATOR, k)
end
```

### result

```rbs
class Proc3
  def self.call: (untyped source) -> untyped
  def self.setup: (untyped k) -> untyped
end
```

## Unknown constant with a method call is not a class

### update

```ruby
module Es
  def self.meta
    match = Ext::VERSION.match(/x/)
    return "#{match[1]}p" if match[2]
    Ext::VERSION
  end
end
```

### result

```rbs
module Es
  def self.meta: -> untyped
end
```

## Constant made by a Struct subclass new holds a class

### update

```ruby
module Fx
  class Options < Struct
    def c = 1
  end
  ConnOpts = Options.new(:a, :b)
  def self.opts = ConnOpts.new
end
```

### result

```rbs
module Fx
  ConnOpts: singleton(Fx::Options)

  def self.opts: -> Fx::Options
end

class Fx::Options < Struct[untyped]
  def c: -> 1
end
```

## Data define on a constant path and a reopened class are one declaration

### update

```ruby
module Lcs
end
Lcs::Block = Data.define(:changes, :insert)
class Lcs::Block
  def size = 1
end
```

### result

```rbs
module Lcs
end

class Lcs::Block < Data
  def changes: -> untyped
  def insert: -> untyped
  def initialize: (changes: untyped, insert: untyped) -> void
  def self.members: -> Array[:changes | :insert]
  def with: (?changes: untyped, ?insert: untyped) -> Lcs::Block
  def size: -> 1
end
```

## Module in one branch and constant alias in another are one declaration

### update

```ruby
module Gx
  if defined?(::Oj)
    module Js
      def self.dump(o) = o
    end
  else
    Js = ::JSON
  end
end
```

### result

```rbs
module Gx
end

module Gx::Js
  def self.dump: [O] (O o) -> O
end
```

## const_set over an existing class does not add a constant

### update

```ruby
module Hx
  class Options
    def a = 1
  end
  unless ENV["X"]
    k = Options.dup
    const_set(:Options, k)
  end
end
```

### result

```rbs
module Hx
end

class Hx::Options
  def a: -> 1
end
```

## Classes defined in top-level control flow

### update

```ruby
if RUBY_VERSION > "3"
  class Kz
    def a = 1
  end
end
begin
  class Kb
    def b = 1
  end
end
[1].each do
  class Kc
    def c = 1
  end
end
```

### result

```rbs
class Kb
  def b: -> 1
end

class Kc
  def c: -> 1
end

class Kz
  def a: -> 1
end
```

## Redefined core method is not declared again

### update

```ruby
class String
  def to_s = self
  def shout = upcase
end
```

### result

```rbs
class String
  def shout: -> String
end
```

## Module included into Object has a BasicObject self type

### update

```ruby
module Trap
  def trap(sig) = sig
end
class Object
  include Trap
end
module Kernel
  prepend Trap
end
```

### result

```rbs
module Kernel
  prepend Trap
end

class Object < BasicObject
  include Trap
end

module Trap : BasicObject
  def trap: [S] (S sig) -> S
end
```

## Including a module instance does not include its class

### update

```ruby
module Store
  class Translatable < Module
    def initialize(name) = super()
  end
  Translation = Translatable.new(:key)
  class Text
    include Translation
  end
end
```

### result

```rbs
module Store
  Translation: Store::Translatable
end

class Store::Text
end

class Store::Translatable < Module
  def initialize: (Symbol name) -> void
end
```

## rbs-inline block with blank comment lines

### update

```ruby
module Px
  # @rbs!
  #   module Tr
  #     class Rip
  #       EXPR: Integer
  #
  #       def lex: () -> Integer
  #     end
  #   end

  class Node
    def x = 1
  end
end
```

### result

```rbs
module Px
  module Tr
    class Rip
      EXPR: Integer

      def lex: () -> Integer
    end
  end
end

class Px::Node
  def x: -> 1
end
```

## Constant written through self in a block is not a top-level constant

### update

```ruby
module OrmAdapter
  class ActiveRecord
    def x = 1
  end
end
Hooks.on_load(:ar) do
  self::OrmAdapter = ::OrmAdapter::ActiveRecord
end
```

### result

```rbs
module OrmAdapter
end

class OrmAdapter::ActiveRecord
  def x: -> 1
end
```

## Constant written through a module alias in another file

### update

### file: lib/a.rb

```ruby
module EventMachine
  module Protocols
    def self.x = 1
  end
end
EM = EventMachine
```

```ruby
EM::P = EventMachine::Protocols
```

### result

```rbs
module EM = EventMachine

module EventMachine
  module P = EventMachine::Protocols
end

module EventMachine::Protocols
  def self.x: -> 1
end
```

## Generic class narrowed by is_a? or when gets type arguments

### update

```ruby
class G
  def a(v) = v.is_a?(Set) ? v : nil
  def c(v)
    case v
    when Range then v
    end
  end
end
```

### result

```rbs
class G
  def a: (untyped v) -> Set[untyped]?
  def c: (untyped v) -> Range[untyped]?
end
```
