# Ruby / Method / dup and clone

## `dup` and `clone` preserve user class receiver type

### update

```ruby
class Container
  def initialize(value) = @value = value
  attr_reader :value
end

class Probe
  def via_dup   = Container.new(1).dup
  def via_clone = Container.new("x").clone
end
```

### result

```rbs
class Container
  def initialize: ((Integer | String) value) -> void
  def value: -> 1 | "x"
end

class Probe
  def via_dup: -> Container
  def via_clone: -> Container
end
```

## `dup` keeps collection literal shapes

### update

```ruby
class Probe
  def array_dup = [1, 2, 3].dup
  def hash_dup  = { a: 1 }.dup
  def str_dup   = "hello".dup
  def sym_dup   = :sym.dup
end
```

### result

```rbs
class Probe
  def array_dup: -> [1, 2, 3]
  def hash_dup: -> { a: 1 }
  def str_dup: -> "hello"
  def sym_dup: -> :sym
end
```
