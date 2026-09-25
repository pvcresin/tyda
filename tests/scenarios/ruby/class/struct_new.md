# Ruby / Class / Struct New

## Constant assignment pattern

### update

```ruby
Point = Struct.new(:x, :y)
```

### result

```rbs
class Point
  def x: -> untyped
  def x=: (untyped x) -> untyped
  def y: -> untyped
  def y=: (untyped y) -> untyped
  def initialize: (untyped x, untyped y) -> void
  def self.members: -> Array[:x | :y]
end
```

## Inheritance pattern

### update

```ruby
class Line < Struct.new(:start_point, :end_point)
  def length = 42
end
```

### result

```rbs
class Line
  def start_point: -> untyped
  def start_point=: (untyped start_point) -> untyped
  def end_point: -> untyped
  def end_point=: (untyped end_point) -> untyped
  def initialize: (untyped start_point, untyped end_point) -> void
  def self.members: -> Array[:end_point | :start_point]
  def length: -> 42
end
```

## keyword_init: true

### update

```ruby
Config = Struct.new(:host, :port, keyword_init: true)
```

### result

```rbs
class Config
  def host: -> untyped
  def host=: (untyped host) -> untyped
  def port: -> untyped
  def port=: (untyped port) -> untyped
  def initialize: (host: untyped, port: untyped) -> void
  def self.members: -> Array[:host | :port]
end
```

## keyword_init call sites update members

### update

```ruby
Option = Struct.new(:name, :value, keyword_init: true)

def option_name = Option.new(name: "level", value: 1).name
def option_value = Option.new(name: "level", value: 1).value
```

### result

```rbs
class Object < BasicObject
  def option_name: -> String
  def option_value: -> Integer
end

class Option
  def name: -> String
  def name=: (String name) -> String
  def value: -> Integer
  def value=: (Integer value) -> Integer
  def initialize: (name: String, value: Integer) -> void
  def self.members: -> Array[:name | :value]
end
```

## Splat member list

### update

```ruby
MEMBERS = %i[name count]

Entry = Struct.new(*MEMBERS, keyword_init: true)

def entry_name = Entry.new(name: "a", count: 1).name
def entry_count = Entry.new(name: "a", count: 1).count
```

### result

```rbs
MEMBERS: [:name, :count]

class Entry
  def name: -> String
  def name=: (String name) -> String
  def count: -> Integer
  def count=: (Integer count) -> Integer
  def initialize: (name: String, count: Integer) -> void
  def self.members: -> Array[:count | :name]
end

class Object < BasicObject
  def entry_name: -> String
  def entry_count: -> Integer
end
```

## Register block def on generated class

### update

```ruby
Item = Struct.new(:name, :count) do
  def label = "item"

  def reset_value = reset_count

  private

  def reset_count = 0
end

def read_label = Item.new("a", 1).label
def read_reset = Item.new("b", 2).reset_value
```

### result

```rbs
class Item
  def name: -> String
  def name=: (String name) -> String
  def count: -> Integer
  def count=: (Integer count) -> Integer
  def initialize: (String name, Integer count) -> void
  def self.members: -> Array[:count | :name]
  def label: -> "item"
  def reset_value: -> 0
  private def reset_count: -> 0
end

class Object < BasicObject
  def read_label: -> "item"
  def read_reset: -> 0
end
```

## Single member

### update

```ruby
Wrapper = Struct.new(:value)
```

### result

```rbs
class Wrapper
  def value: -> untyped
  def value=: (untyped value) -> untyped
  def initialize: (untyped value) -> void
  def self.members: -> Array[:value]
end
```

## const_set pattern

### update

```ruby
module Store
  const_set(:Pair, Struct.new(:key, :value, keyword_init: true))
end

def pair_key = Store::Pair.new(key: :name, value: 1).key
def pair_value = Store::Pair.new(key: :name, value: 1).value
```

### result

```rbs
class Object < BasicObject
  def pair_key: -> Symbol
  def pair_value: -> Integer
end

class Store::Pair
  def key: -> Symbol
  def key=: (Symbol key) -> Symbol
  def value: -> Integer
  def value=: (Integer value) -> Integer
  def initialize: (key: Symbol, value: Integer) -> void
  def self.members: -> Array[:key | :value]
end
```

## Struct.new superclass members from the call site

### update

```ruby
class Point < Struct.new(:x, :y)
end

def foo = Point.new(1, "hello").x
```

### result

```rbs
class Object < BasicObject
  def foo: -> Integer
end

class Point
  def x: -> Integer
  def x=: (Integer x) -> Integer
  def y: -> String
  def y=: (String y) -> String
  def initialize: (Integer x, String y) -> void
  def self.members: -> Array[:x | :y]
end
```
