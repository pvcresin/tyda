# Ruby / Class / Data Define

## Constant assignment pattern

### update

```ruby
Coordinate = Data.define(:lat, :lng)
```

### result

```rbs
class Coordinate
  def lat: -> untyped
  def lng: -> untyped
  def initialize: (lat: untyped, lng: untyped) -> void
  def self.members: -> Array[:lat | :lng]
  def with: (?lat: untyped, ?lng: untyped) -> Coordinate
end
```

## Splat member list

### update

```ruby
FIELDS = { name: :name, count: :count }

Entry = Data.define(*FIELDS.values)

def entry_name = Entry.new(name: "a", count: 1).name
def entry_count = Entry.new(name: "a", count: 1).count
```

### result

```rbs
FIELDS: { name: :name, count: :count }

class Entry
  def name: -> "a"
  def count: -> 1
  def initialize: (name: String, count: Integer) -> void
  def self.members: -> Array[:count | :name]
  def with: (?name: String, ?count: Integer) -> Entry
end

class Object < BasicObject
  def entry_name: -> "a"
  def entry_count: -> 1
end
```

## Register instance and singleton methods in block

### update

```ruby
Entry = Data.define(:value) do
  def label = "entry"

  class << self
    def build = new(value: "created")
  end
end

def read_label = Entry.new(value: "a").label
def build_entry = Entry.build
```

### result

```rbs
class Entry
  def value: -> "a" | "created"
  def initialize: (value: String) -> void
  def self.members: -> Array[:value]
  def with: (?value: String) -> Entry
  def label: -> "entry"
  def self.build: -> Entry
end

class Object < BasicObject
  def read_label: -> "entry"
  def build_entry: -> Entry
end
```

## Inheritance pattern with added methods

### update

```ruby
class Token < Data.define(:type, :value)
  def to_s = "token"
end
```

### result

```rbs
class Token
  def type: -> untyped
  def value: -> untyped
  def initialize: (type: untyped, value: untyped) -> void
  def self.members: -> Array[:type | :value]
  def with: (?type: untyped, ?value: untyped) -> Token
  def to_s: -> "token"
end
```

## Single member

### update

```ruby
Name = Data.define(:value)
```

### result

```rbs
class Name
  def value: -> untyped
  def initialize: (value: untyped) -> void
  def self.members: -> Array[:value]
  def with: (?value: untyped) -> Name
end
```

## Writer method is not generated

### update

```ruby
Immutable = Data.define(:x, :y)
```

### result

```rbs
class Immutable
  def x: -> untyped
  def y: -> untyped
  def initialize: (x: untyped, y: untyped) -> void
  def self.members: -> Array[:x | :y]
  def with: (?x: untyped, ?y: untyped) -> Immutable
end
```

## const_set pattern

### update

```ruby
module Store
  const_set("Entry", Data.define(:name) do
    def label = name.to_s
  end)
end

def entry_name = Store::Entry.new(name: :item).name
def entry_label = Store::Entry.new(name: :item).label
```

### result

```rbs
class Object < BasicObject
  def entry_name: -> :item
  def entry_label: -> String
end

class Store::Entry
  def name: -> :item
  def initialize: (name: Symbol) -> void
  def self.members: -> Array[:name]
  def with: (?name: Symbol) -> Store::Entry
  def label: -> String
end
```

## with returns self class type

### update

```ruby
Point = Data.define(:x, :y)

def foo = Point.new(x: 1, y: 2).with(x: 3)
```

### result

```rbs
class Object < BasicObject
  def foo: -> Point
end

class Point
  def x: -> 1
  def y: -> 2
  def initialize: (x: Integer, y: Integer) -> void
  def self.members: -> Array[:x | :y]
  def with: (?x: Integer, ?y: Integer) -> Point
end
```
