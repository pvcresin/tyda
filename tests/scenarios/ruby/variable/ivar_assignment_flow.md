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
  def consume: [I] (I items) -> I
end
```

## A self call that may write the instance variable ends the local value

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

  def build_kept
    @items = [1, nil].compact
    ping
    @items
  end

  def helper = reset
  def ping = 1
end
```

### result

```rbs
class IvarCallValueAfterSelfCall
  def reset: -> nil
  def build: -> [1]?
  def build_kept: -> [1]
  def helper: -> nil
  def ping: -> 1
end
```

## A replayed method pushes onto an instance variable once

### update

```ruby
class Errors
  attr_reader :errors

  def initialize
    @errors = []
  end

  def check(x)
    later(x)
  rescue ArgumentError
    @errors.push("a")
    nil
  end

  def later(x) = x
end

Errors.new.check(1)
```

### result

```rbs
class Errors
  def errors: -> ["a"]
  def initialize: -> void
  def check: (Integer x) -> Integer?
  def later: [X] (X x) -> X
end
```

## A guard on an ivar that later methods write narrows it

### update

```ruby
class MemoGuard
  def ignored?
    return @ignored unless @ignored.nil?

    @ignored = missing?
  end

  def missing? = true
end
```

### result

```rbs
class MemoGuard
  def ignored?: -> true
  def missing?: -> true
end
```
