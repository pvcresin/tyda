# Ruby / Class / Enumerable include

## Enumerable include exposes iteration helpers when `each` is defined

### update

```ruby
class Bag
  include Enumerable[untyped]

  def initialize = @items = [1, 2, 3]
  def each(&block) = @items.each(&block)
end

class Probe
  def to_array       = Bag.new.to_a
  def first_item     = Bag.new.first
  def count_items    = Bag.new.count
  def any_item       = Bag.new.any?
  def include_two    = Bag.new.include?(2)
  def map_to_string  = Bag.new.map { |n| n.to_s }
  def select_evens   = Bag.new.select { |n| n.even? }
end
```

### result

```rbs
class Bag
  include Enumerable[untyped]

  def initialize: -> void
  def each: () ?{ (?) -> untyped } -> untyped
end

class Probe
  def to_array: -> Array[untyped]
  def first_item: -> untyped
  def count_items: -> Integer
  def any_item: -> bool
  def include_two: -> bool
  def map_to_string: -> Array[untyped]
  def select_evens: -> Array[untyped]
end
```

## Enumerable helpers infer the element type from an explicit yield

### update

```ruby
class NumberBag
  include Enumerable[untyped]

  def each
    yield 1
    yield 2
    yield 3
  end
end

class Client
  def doubled  = NumberBag.new.map { |n| n * 2 }
  def stringed = NumberBag.new.map { |n| n.to_s }
  def chosen   = NumberBag.new.select { |n| n > 1 }
  def found    = NumberBag.new.find { |n| n > 1 }
end
```

### result

```rbs
class Client
  def doubled: -> Array[Integer]
  def stringed: -> Array[String]
  def chosen: -> Array[1 | 2 | 3]
  def found: -> (1 | 2 | 3)?
end

class NumberBag
  include Enumerable[untyped]

  def each: [R] { (Integer) -> R } -> R
end
```

## Enumerable select with a symbol proc returns an Array

```ruby
class Items
  include Enumerable[untyped]

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
  include Enumerable[untyped]

  def initialize: -> void
  def each: () ?{ (?) -> untyped } -> untyped
end

class ItemsUser
  def odd: -> Array[untyped]
end
```

## Blockless Enumerable helpers keep the element type from a custom each

```ruby
class Pair
  include Enumerable[untyped]

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
  include Enumerable[untyped]

  def each: [R] { (Integer) -> R } -> R
end

class PairUser
  def all: -> Array[1 | 2]
  def head: -> (1 | 2)?
  def ordered: -> Array[1 | 2]
  def lowest: -> (1 | 2)?
end
```
