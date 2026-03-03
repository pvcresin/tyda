# Ruby / Class / Instance Variable Collection Operations

## Mutating write on an instance variable returns the pushed array

### update

```ruby
class Builder
  def initialize
    @items = []
  end

  def add
    @items << 1
  end
end
```

### result

```rbs
class Builder
  def initialize: -> void
  def add: -> [1]
end
```

## Instance variable mutating write returns the same type as a read

### update

```ruby
class Seeded
  def initialize
    @items = [0]
  end

  def add
    @items << 1
  end

  def items
    @items
  end
end
```

### result

```rbs
class Seeded
  def initialize: -> void
  def add: -> [0, 1]
  def items: -> [0, 1]
end
```

## Hash `merge!` on an instance variable returns the merged hash

### update

```ruby
class Config
  def initialize
    @options = { verbose: false }
  end

  def enable
    @options.merge!(verbose: true)
  end

  def options
    @options
  end
end
```

### result

```rbs
class Config
  def initialize: -> void
  def enable: -> { verbose: true }
  def options: -> { verbose: true }
end
```

## Block iteration on an instance variable infers the element type

### update

```ruby
class Numbers
  def initialize
    @values = [1, 2, 3]
  end

  def doubled
    @values.map { |n| n * 2 }
  end

  def evens
    @values.select { |n| n.even? }
  end

  def total
    @values.reduce(0) { |sum, n| sum + n }
  end
end
```

### result

```rbs
class Numbers
  def initialize: -> void
  def doubled: -> Array[Integer]
  def evens: -> Array[1 | 2 | 3]
  def total: -> Integer
end
```
