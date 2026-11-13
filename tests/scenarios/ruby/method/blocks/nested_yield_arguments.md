## yield nested in a call argument requires a block

### update

```ruby
class NestedYieldProbe
  def wrapper(value)
    value
  end

  def call
    wrapper(yield)
  end
end
```

### result

```rbs
class NestedYieldProbe
  def wrapper: [T] (T value) -> T
  def call: { () -> untyped } -> untyped
end
```

## Block parameters in nested yields do not become nil

### update

```ruby
class CoreService
  def update_file = nil
end

class CoreServer
  def initialize
    @cores = { "lib" => CoreService.new }
  end

  def each_core
    @cores.each do |_path, core|
      yield core
    end
  end

  def update_all
    each_core { |core| core.update_file }
  end
end
```

### result

```rbs
class CoreServer
  def initialize: -> void
  def each_core: { (untyped) -> untyped } -> { "lib" => CoreService }
  def update_all: -> { "lib" => CoreService }
end

class CoreService
  def update_file: -> nil
end
```

## A yield inside a block on a parameter receiver passes the element type

### update

```ruby
class Mapper
  def map_all(xs) = xs.map { |e| yield e }
end

class MapperUser
  def labels = Mapper.new.map_all([1]) { |e| e.to_s }
end
```

### result

```rbs
class Mapper
  def map_all: [T] (Array[Integer] xs) { (Integer) -> T } -> Array[T]
end

class MapperUser
  def labels: -> Array[String]
end
```

## A yield inside a block on a literal receiver passes the element type

### update

```ruby
class Doubler
  def each_one = [1].map { |e| yield e }
end

class DoublerUser
  def labels = Doubler.new.each_one { |e| e.to_s }
end
```

### result

```rbs
class Doubler
  def each_one: [T] { (Integer) -> T } -> Array[T]
end

class DoublerUser
  def labels: -> Array[String]
end
```
