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

## Clearing an instance variable preserves its shared element summary

### update

```ruby
class Resettable
  def initialize
    @items = [1]
  end

  def clear_items
    @items.clear
  end

  def return_clear_items
    return @items.clear
  end

  def assign_clear_items
    result = @items.clear
    result
  end

  def items
    @items
  end
end
```

### result

```rbs
class Resettable
  def initialize: -> void
  def clear_items: -> [ ]
  def return_clear_items: -> [ ]
  def assign_clear_items: -> [ ]
  def items: -> Array[1]
end
```

## Source-defined Array#clear keeps its return type

### update

```ruby
class Array
  def clear = :custom
end

class ClearOverride
  def clear_items = [1].clear
end
```

### result

```rbs
class Array[unchecked out E]
  def clear: -> :custom
end

class ClearOverride
  def clear_items: -> :custom
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

## Same-method read sees an instance variable string append

### update

```ruby
class Builder
  def run
    @text = "a"
    @text << "b"
    @text
  end
end
```

### result

```rbs
class Builder
  def run: -> String
end
```

## An instance variable read after an append keeps the element

### update

```ruby
class AppendInPlace
  def run
    @items = [0]
    @items << 1
    @items
  end
end
```

### result

```rbs
class AppendInPlace
  def run: -> [0, 1]
end
```

## An instance variable Hash keeps a key written in the same method

### update

```ruby
class IndexWriteInPlace
  def run
    @table = {}
    @table[:a] = 1
    @table
  end
end
```

### result

```rbs
class IndexWriteInPlace
  def run: -> Hash[:a, 1]
end
```
