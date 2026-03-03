# Ruby / Literal / Range methods

## Range iteration and conversion

### update

```ruby
class Probe
  def to_array      = (1..5).to_a
  def step_chain    = (1..10).step(2).to_a
  def size          = (1..10).size
  def first_one     = (1..10).first
  def map_to_string = (1..3).map { |n| n.to_s }
  def select_evens  = (1..10).select { |n| n.even? }
end
```

### result

```rbs
class Probe
  def to_array: -> Array[Integer]
  def step_chain: -> Array[Integer]
  def size: -> (Integer | Float)?
  def first_one: -> Integer
  def map_to_string: -> Array[String]
  def select_evens: -> Array[Integer]
end
```

## Range filter with a symbol-to-proc block returns an Array

### update

```ruby
class Probe
  def select_sym = (1..10).select(&:even?)
  def reject_sym = (1..10).reject(&:even?)
  def map_sym    = (1..5).map(&:to_s)
end
```

### result

```rbs
class Probe
  def select_sym: -> Array[Integer]
  def reject_sym: -> Array[Integer]
  def map_sym: -> Array[String]
end
```

## Range predicates and aggregates

### update

```ruby
class Probe
  def include_check = (1..10).include?(5)
  def min_int       = (1..10).min
  def max_int       = (1..10).max
  def sum_int       = (1..10).sum
end
```

### result

```rbs
class Probe
  def include_check: -> bool
  def min_int: -> Integer?
  def max_int: -> Integer?
  def sum_int: -> Integer
end
```
