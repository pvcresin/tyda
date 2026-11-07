# Known Issues / Flow Guards

## An ivar assigned from a no-argument call keeps that value in the same method

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
  def consume: (Array[Integer] items) -> Array[Integer]
end
```

## A nil guard on a self call reaches the argument of a later call

```ruby
class GuardedSelfCallValue
  def value = 1
end

class GuardedSelfCallArgument
  def run
    consume(source) if source
  end

  def source
    GuardedSelfCallValue.new if rand > 0.5
  end

  def consume(value)
    value.value
  end
end
```

### result

```rbs
class GuardedSelfCallArgument
  def run: -> 1?
  def source: -> GuardedSelfCallValue?
  def consume: (GuardedSelfCallValue value) -> 1
end

class GuardedSelfCallValue
  def value: -> 1
end
```

## A repeated ENV read keeps its truthy guard

```ruby
class RepeatedEnvRead
  def run
    consume(ENV["TOKEN"]) if ENV["TOKEN"]
  end

  def consume(value) = value
end
```

### result

```rbs
class RepeatedEnvRead
  def run: -> String?
  def consume: (String value) -> String
end
```

## A when clause with a constant lookup narrows the subject to that class

```ruby
class DynamicWhenFirst < StandardError
  def detail = 1
end

class DynamicWhenSecond < StandardError
end

class DynamicWhenSubject
  ERRORS = { first: DynamicWhenFirst, second: DynamicWhenSecond }.freeze

  def handle(error)
    case error
    when ERRORS[:first]
      consume(error)
    end
  end

  def consume(error) = error

  def run
    handle(DynamicWhenFirst.new)
    handle(DynamicWhenSecond.new)
  end
end
```

### result

```rbs
class DynamicWhenFirst < StandardError
  def detail: -> 1
end

class DynamicWhenSubject
  ERRORS: { first: singleton(DynamicWhenFirst), second: singleton(DynamicWhenSecond) }
  def handle: ((DynamicWhenFirst | DynamicWhenSecond) error) -> DynamicWhenFirst?
  def consume: (DynamicWhenFirst error) -> DynamicWhenFirst
  def run: -> DynamicWhenFirst?
end
```

## A rest parameter keeps Array through a filtering block

```ruby
class RestParameterFilter
  def rejected(*keys)
    keys.reject { |key| key }
  end
end
```

### result

```rbs
class RestParameterFilter
  def rejected: (*untyped keys) -> Array[untyped]
end
```

## A tuple argument with an unknown element keeps the declared return

```ruby
module TupleJoinHelper
  def self.joined(parts)
    parts.join(" ")
  end
end

class TupleJoinCaller
  def run(value)
    parts = ["a"]
    parts << value.name
    TupleJoinHelper.joined(parts)
  end
end
```

### result

```rbs
class TupleJoinCaller
  def run: (untyped value) -> String
end

module TupleJoinHelper
  def self.joined: ([String, untyped] parts) -> String
end
```
