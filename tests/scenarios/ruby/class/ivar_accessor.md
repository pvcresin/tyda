# Ruby / Class / Ivar Accessor

## initialize with manual accessor

### update

```ruby
class Point
  def initialize(x, y)
    @x = x
    @y = y
  end

  def x = @x

  def y = @y
end
Point.new(1, 2)
```

### result

```rbs
class Point
  def initialize: (Integer x, Integer y) -> void
  def x: -> 1
  def y: -> 2
end
```

## Assign chained untyped call to ivar

### update

```ruby
class Config
  attr_reader :limit, :label
  def initialize(options = {})
    @limit = options[:limit].to_i
    @label = options[:label].to_s
  end
end
```

### result

```rbs
class Config
  def limit: -> Integer
  def label: -> String
  def initialize: (?Hash[untyped, untyped] options) -> void
end
```

## Hand-written getter resolves on an instance

### update

```ruby
class Counter
  def initialize(start)
    @count = start
  end

  def count = @count
end

class Client
  def current = Counter.new(10).count
  def next_value = Counter.new(10).count + 1
end
```

### result

```rbs
class Client
  def current: -> Integer
  def next_value: -> Integer
end

class Counter
  def initialize: (Integer start) -> void
  def count: -> 10
end
```
