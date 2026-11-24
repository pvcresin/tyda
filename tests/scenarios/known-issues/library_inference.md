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

## Implicit self call with a block in a reopened Array uses the block overload

### update

```ruby
class Array
  def strings = map { |x| x.to_s }
  def strings_by_symbol = map(&:to_s)
end

class Hash
  def string_keys = transform_keys(&:to_s)
end
```

### result

```rbs
class Array[unchecked out E]
  def strings: -> Array[String]
  def strings_by_symbol: -> Array[String]
end

class Hash[unchecked out K, unchecked out V]
  def string_keys: -> Hash[String, V]
end
```

## Own method named like an Enumerable method keeps its untyped return

### update

```ruby
class Mapper
  def map(key) = [1][key]
  def select(key) = [1][key]
  def from_map(key) = map(key)
  def from_select(key) = select(key)
end
```

### result

```rbs
class Mapper
  def map: (untyped key) -> untyped
  def select: (untyped key) -> untyped
  def from_map: (untyped key) -> untyped
  def from_select: (untyped key) -> untyped
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

## Plus-assign of an array literal keeps the array type

### update

```ruby
class Args
  def ints
    r = [1]
    r += [2]
    r
  end

  def strings
    r = ["a"]
    r += ["b"]
  end
end
```

### result

```rbs
class Args
  def ints: -> Array[1 | 2]
  def strings: -> Array["a" | "b"]
end
```

## Kernel Rational without the exception keyword is not nilable

### update

```ruby
class Limits
  SMALL = Rational(1, 10**12)
  MAX = 60 - Rational(1, 10**12)
end
```

### result

```rbs
class Limits
  SMALL: Rational
  MAX: Rational
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
## Own method overrides the method of a core superclass

### update

```ruby
class Server < Socket
  def accept = accept_nonblock
  def accept_nonblock = 1
end
```

### result

```rbs
class Server < Socket
  def accept: -> 1
  def accept_nonblock: -> 1
end
```
## Comparable clamp with a range returns the receiver type

### update

```ruby
class Jitter
  LIMIT = 0.0..1.0
  def literal = 0.5.clamp(0.0..1.0)
  def constant = 0.5.clamp(LIMIT)
end
```

### result

```rbs
class Jitter
  LIMIT: Range[Float]

  def literal: -> Float
  def constant: -> Float
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
## Empty array concat with an untyped value is not an empty tuple

### update

```ruby
class Collector
  def collect(items)
    result = []
    result.concat(items)
  end

  def collect_later(items)
    result = []
    result.concat(items)
    result
  end
end
```

### result

```rbs
class Collector
  def collect: (untyped items) -> Array[untyped]
  def collect_later: (untyped items) -> Array[untyped]
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
## attr_reader named like an Object method is used by implicit self calls

### update

```ruby
class Wrapper
  attr_reader :hash

  def initialize(hash)
    @hash = hash
  end

  def to_hash = hash
end
```

### result

```rbs
class Wrapper
  def hash: -> untyped
  def initialize: (untyped hash) -> void
  def to_hash: -> untyped
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
## Own method named like a Kernel method is used by implicit self calls

### update

```ruby
class Page
  def limit(num) = spawn(num)
  def spawn(num) = "x"
end
```

### result

```rbs
class Page
  def limit: (untyped num) -> "x"
  def spawn: (untyped num) -> "x"
end
```

## Module singleton new defined in Ruby is used by calls to new

### update

```ruby
module Mailer
  class Message
    def initialize(source)
      @source = source
    end
  end

  def self.new(*args) = Message.new(*args)
  def self.read(source) = new(source)
end
```

### result

```rbs
module Mailer
  def self.new: (*untyped args) -> Mailer::Message
  def self.read: (untyped source) -> Mailer::Message
end

class Mailer::Message
  def initialize: (untyped source) -> void
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
