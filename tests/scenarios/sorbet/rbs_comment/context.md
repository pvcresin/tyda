# Sorbet / RBS Comment / Context

## Bind block self with self as

### update

`sorbet/config`

```ruby
.
```

```ruby
class Config
  #: -> Integer
  def timeout = 1
end

class Use
  def value
    1.times do
      #: self as Config
      return timeout
    end
  end
end
```

### result

```rbs
class Config
  def timeout: -> Integer
end

class Use
  def value: -> Integer
end
```

## Block self annotation avoids manual self as

### update

`sorbet/config`

```ruby
.
```

```ruby
class Config
  #: -> Integer
  def timeout = 1
end

class Builder
  #: { () [self: Config] -> void } -> void
  def self.configure
  end
end

class Use
  def value
    x = 1
    Builder.configure do
      x = timeout
    end
    x
  end
end
```

### result

```rbs
class Builder
  def self.configure: { () -> void } -> void
end

class Config
  def timeout: -> Integer
end

class Use
  def value: -> Integer
end
```

## Resolve bare call with @requires_ancestor

### update

`sorbet/config`

```ruby
.
```

```ruby
class Core
  #: -> String
  def helper = ""
end

# @requires_ancestor: ::Core
module NeedsCore
  def value
    helper
  end
end
```

### result

```rbs
class Core
  def helper: -> String
end

# @requires_ancestor: ::Core
module NeedsCore
  def value: -> String
end
```
