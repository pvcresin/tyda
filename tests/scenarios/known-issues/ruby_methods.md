# Known Issues / Ruby Methods

## Blockless Enumerable helpers keep the element type from a custom each

```ruby
class Pair
  include Enumerable

  def each
    yield 1
    yield 2
  end
end

class PairUser
  def all = Pair.new.to_a
  def head = Pair.new.first
  def ordered = Pair.new.sort
  def lowest = Pair.new.min
end
```

### result

```rbs
class Pair
  include Enumerable

  def each: { (Integer) -> untyped } -> untyped
end

class PairUser
  def all: -> Array[1 | 2]
  def head: -> (1 | 2)?
  def ordered: -> Array[1 | 2]
  def lowest: -> (1 | 2)?
end
```

## Enumerable select with a symbol proc returns an Array

```ruby
class Items
  include Enumerable

  def initialize = @items = [1, 2, 3]
  def each(&block) = @items.each(&block)
end

class ItemsUser
  def odd = Items.new.select(&:odd?)
end
```

### result

```rbs
class Items
  include Enumerable

  def initialize: -> void
  def each: () ?{ (?) -> untyped } -> untyped
end

class ItemsUser
  def odd: -> Array[untyped]
end
```

## A default built from an earlier parameter keeps its type

```ruby
class DefaultDep
  def double(base, scaled = base * 2) = scaled
end

DefaultDep.new.double(3)
```

### result

```rbs
class DefaultDep
  def double: (Integer base, ?Integer scaled) -> Integer
end
```

## An ivar set from a parameter with an or fallback keeps the joined type

```ruby
class OrFallback
  def initialize(label)
    @label = label || "none"
  end

  def label = @label
end

OrFallback.new(nil)
OrFallback.new("a")
```

### result

```rbs
class OrFallback
  def initialize: (String? label) -> void
  def label: -> String
end
```

## A yield result stored in a local keeps the block signature

```ruby
class YieldResult
  def run
    result = yield
    result
  end
end

class YieldResultUser
  def call = YieldResult.new.run { "done" }
end
```

### result

```rbs
class YieldResult
  def run: { () -> "done" } -> "done"
end

class YieldResultUser
  def call: -> "done"
end
```

## A yield inside an each block passes the element type

```ruby
class NameList
  def each_name(names)
    names.each { |name| yield name }
  end
end

NameList.new.each_name(["a"]) { |name| name.size }
```

### result

```rbs
class NameList
  def each_name: (Array[String] names) { (String) -> Integer } -> Array[String]
end
```

## A yield of a singleton method defined later in the class keeps its type

```ruby
class LaterSetup
  def self.setup
    yield instance
  end

  def self.instance = new
end
```

### result

```rbs
class LaterSetup
  def self.setup: { (LaterSetup) -> untyped } -> untyped
  def self.instance: -> LaterSetup
end
```

## A singleton attr_accessor reader set outside the class body is nilable

```ruby
class Level
  class << self
    attr_accessor :current
  end
end

Level.current = 3
```

### result

```rbs
class Level
  def self.current: -> Integer?
  def self.current=: (Integer current) -> Integer
end
```

## A short type name in an inline RBS annotation resolves in the enclosing class

```ruby
class Lookup
  # @rbs!
  #   interface _Named
  #     def name: () -> String
  #   end

  #: -> _Named
  def named = raise

  def label = named.name
end
```

### result

```rbs
class Lookup
  interface _Named
    def name: () -> String
  end

  def named: -> _Named
  def label: -> String
end
```
