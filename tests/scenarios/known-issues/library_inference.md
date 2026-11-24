# Known Issues / Library Inference

## Singleton new declared in RBS overrides the constructor

```rbs
module Fx
  def self.new: () -> Fx::Conn
  class Conn
  end
end
class Kx
  def self.new: () -> String
end
```

### update

```ruby
class A
  def c = Fx.new
  def k = Kx.new
end
```

### result

```rbs
class A
  def c: -> Fx::Conn
  def k: -> String
end
```

## module_function makes a singleton and a private instance method

### update

```ruby
module Mf
  def make = 1
  module_function :make
end
```

### result

```rbs
module Mf
  def self?.make: -> 1
end
```

## Memoized array keeps elements pushed by other methods

### update

```ruby
class T
  def store = @store ||= []
  def add(v) = store << v
  def add_s = add("s")
end
class U
  def initialize = @items = []
  def items = @items
end
class V
  def fill(u) = u.items << 1
end
V.new.fill(U.new)
```

### result

```rbs
class T
  def store: -> Array[String]
  def add: (String v) -> Array[String]
  def add_s: -> Array[String]
end

class U
  def initialize: -> void
  def items: -> Array[Integer]
end

class V
  def fill: (U u) -> Array[Integer]
end
```

## super under an unknown superclass does not use Kernel methods

### update

```ruby
class Info < Ext::Mash
  def select = super
  def format = super
end
```

### result

```rbs
class Info < Ext::Mash
  def select: -> untyped
  def format: -> untyped
end
```

## Hash merged with an untyped hash keeps its keys

### update

```ruby
class Cl
  attr_reader :options
  def initialize(opts = {})
    @options = { url: "x", connection_opts: {} }.merge(opts)
    @options[:connection_opts][:ssl] = 1
  end
  def m(opts) = { url: "x" }.merge(opts)
end
```

### result

```rbs
class Cl
  def options: -> Hash[untyped, untyped]
  def initialize: (?Hash[untyped, untyped] opts) -> void
  def m: (untyped opts) -> Hash[untyped, untyped]
end
```

## Splat of a union of arrays spreads the elements

### update

```ruby
class Sp
  def d(c)
    items = c ? [1].map { _1 } : ["s"].map { _1 }
    [0, *items]
  end
end
```

### result

```rbs
class Sp
  def d: (untyped c) -> Array[0 | 1 | "s"]
end
```

## Hash constant read with an untyped key uses its value types

### update

```ruby
class Table
  DATA = { "a" => "b" }
  def lookup(key) = DATA[key]
end
```

### result

```rbs
class Table
  DATA: { "a" => "b" }

  def lookup: (untyped key) -> "b"?
end
```

## Splat of a tuple spreads into generic parameters

### update

```ruby
class Pair
  def first = take(*pair)
  def pair = [1, "a"]
  def take(a, b) = a
end
```

### result

```rbs
class Pair
  def first: -> Integer
  def pair: -> [1, "a"]
  def take: [A] (A a, String b) -> A
end
```
## Method defined after def_delegators replaces the delegator

### update

```ruby
class TaskSet
  extend Forwardable

  def_delegators :@tasks, :<<, :first

  def initialize
    @tasks = []
  end

  def <<(task)
    @tasks.push(task)
    self
  end
end
```

### result

```rbs
class TaskSet
  extend Forwardable

  def first: -> untyped
  def initialize: -> void
  def <<: (untyped task) -> TaskSet
end
```
## self in a class body passed as an argument is the class

### update

```ruby
module Registry
  def self.add(klass, name)
    klass.name
  end
end

class Plugin
  Registry.add(self, :plugin)

  def id = 1
end
```

### result

```rbs
class Plugin
  def id: -> 1
end

module Registry
  def self.add: (singleton(Plugin) klass, Symbol name) -> String?
end
```
## each with a block on a Hash.new default value is not an Enumerator

### update

```ruby
class Hooks
  def self.callbacks = @callbacks ||= Hash.new { |hash, key| hash[key] = [] }
  def self.notify(key) = callbacks[key].each { |listener| listener.call }
end
```

### result

```rbs
class Hooks
  def self.callbacks: -> Hash[untyped, [ ]]
  def self.notify: (untyped key) -> [ ]
end
```
## super with a block in a Hash subclass each is not an Enumerator

### update

```ruby
class Headers < Hash
  def each
    super do |k, v|
      yield(k, v.to_s)
    end
  end
end
```

### result

```rbs
class Headers < Hash[untyped, untyped]
  def each: { (untyped, untyped) -> untyped } -> Headers
end
```
## each with a block on a local assigned in a block is not an Enumerator

### update

```ruby
class Buffer
  def initialize
    @lock = Mutex.new
    @entries = []
  end

  def push(entry) = @entries << entry

  def flush
    entries = nil
    @lock.synchronize do
      entries = @entries
      @entries = []
    end
    entries.each do |entry|
      entry.to_s
    end
  end
end
```

### result

```rbs
class Buffer
  def initialize: -> void
  def push: (untyped entry) -> Array[untyped]
  def flush: -> Array[untyped]
end
```

## super with a Method block argument is not an Enumerator

### update

```ruby
class IndifferentHash < Hash
  def convert_value(value) = value

  def transform_values!
    super(&method(:convert_value))
  end
end
```

### result

```rbs
class IndifferentHash < Hash[untyped, untyped]
  def convert_value: [V] (V value) -> V
  def transform_values!: -> IndifferentHash
end
```
