# Ruby / Class / Enumerable include

## Enumerable include exposes iteration helpers when `each` is defined

### update

```ruby
class Bag
  include Enumerable

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
  include Enumerable

  def initialize: -> void
  def each: (?untyped &block) -> untyped
end

class Probe
  def to_array: -> Array[untyped]
  def first_item: -> untyped
  def count_items: -> Integer
  def any_item: -> bool
  def include_two: -> bool
  def map_to_string: -> Array[String]
  def select_evens: -> Array[untyped]
end
```

## Enumerable helpers infer the element type from an explicit yield

### update

```ruby
class NumberBag
  include Enumerable

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
  include Enumerable

  def each: { (Integer) -> untyped } -> untyped
end
```
