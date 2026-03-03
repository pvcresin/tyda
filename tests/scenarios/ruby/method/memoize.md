# Ruby / Method / Memoize

## Apply strong_memoize block return to method return

### update

```ruby
class Counter
  def count
    strong_memoize(:count) do
      42
    end
  end
end
```

### result

```rbs
class Counter
  def count: -> 42
end
```

## Apply strong_memoize_with block return

### update

```ruby
class Formatter
  def initialize(prefix)
    @prefix = prefix
  end

  def format(id)
    strong_memoize_with(:format, id) do
      "#{@prefix}-#{id}"
    end
  end
end

Formatter.new("item").format(1)
```

### result

```rbs
class Formatter
  def initialize: (String prefix) -> void
  def format: (Integer id) -> String
end
```

## Read `#:` annotation on wrapped def

### update

```ruby
class UserFinder
  #: () -> String
  memoize def target_users
    "fallback"
  end
end
```

### result

```rbs
class UserFinder
  def target_users: -> String
end
```
