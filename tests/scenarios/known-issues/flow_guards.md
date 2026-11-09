# Known Issues / Flow Guards

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
