# Ruby / Variable / Instance Variable Assignment Flow

## A direct write replaces an older instance variable value

### update

```ruby
class A
  def value
    @value = nil
    @value = "ready"
    @value
  end
end
```

### result

```rbs
class A
  def value: -> "ready"
end
```

## Branch writes merge the reachable instance variable values

### update

```ruby
class A
  def value(condition)
    @value = nil
    if condition
      @value = "ready"
    else
      @value = 1
    end
    @value
  end
end
```

### result

```rbs
class A
  def value: (untyped condition) -> (1 | "ready")
end
```

## Compound writes update the current instance variable value

### update

```ruby
class CompoundFlow
  def or_assign
    @value = nil
    @value ||= "ready"
    @value
  end

  def and_assign
    @value = "ready"
    @value &&= 1
    @value
  end

  def and_keeps_false
    @value = false
    @value &&= 1
    @value
  end

  def operator_assign
    @value = "ready"
    @value += " now"
    @value
  end
end
```

### result

```rbs
class CompoundFlow
  def or_assign: -> "ready"
  def and_assign: -> 1
  def and_keeps_false: -> false
  def operator_assign: -> String
end
```

## An ivar assigned from a no-argument call keeps that value in the same method

### update

```ruby
class IvarCallValue
  def reset
    @items = nil
  end

  def build
    @items = [1, nil].compact
    consume(@items)
  end

  def consume(items) = items
end
```

### result

```rbs
class IvarCallValue
  def reset: -> nil
  def build: -> Array[Integer]
  def consume: [T] (T items) -> T
end
```

## A self call after a no-argument call write ends the local value

### update

```ruby
class IvarCallValueAfterSelfCall
  def reset
    @items = nil
  end

  def build
    @items = [1, nil].compact
    helper
    @items
  end

  def helper = 1
end
```

### result

```rbs
class IvarCallValueAfterSelfCall
  def reset: -> nil
  def build: -> [1]?
  def helper: -> 1
end
```
