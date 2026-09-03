# Ruby / RBS Comment / Continuation

## Blank line stops attachment to next def

### update

```ruby
class Gap
  #: () -> Integer

  def value = :ok
end
```

### result

```rbs
class Gap
  def value: -> :ok
end
```

## Multiple #: lines as overloads

### update

```ruby
class Formatter
  #: (Integer) -> String
  #: () -> String
  def format(n = nil) = n ? n.to_s : "default"
end
```

### result

```rbs
class Formatter
  def format: (?Integer n) -> String
            | -> String
end
```

## RBS comment for keyword args

### update

```ruby
#: (name: String, age: Integer) -> String
def greet(name:, age:) = "#{name}: #{age}"
```

### result

```rbs
class Object < BasicObject
  def greet: (name: String, age: Integer) -> String
end
```

## Optional keyword arg

### update

```ruby
#: (?timeout: Integer) -> void
def connect(timeout: 30)
end
```

### result

```rbs
class Object < BasicObject
  def connect: (?timeout: Integer) -> void
end
```

## Rest arg

### update

```ruby
#: (*Integer) -> Integer
def sum(*nums) = 0
```

### result

```rbs
class Object < BasicObject
  def sum: (*Integer nums) -> Integer
end
```

## Double splat arg

### update

```ruby
#: (**String) -> void
def log(**opts)
end
```

### result

```rbs
class Object < BasicObject
  def log: (**String opts) -> void
end
```

## Block arg

### update

```ruby
#: () { (Integer) -> String } -> Array[String]
def map_items(&blk) = []
```

### result

```rbs
class Object < BasicObject
  def map_items: { (Integer) -> String } -> Array[String]
end
```

## Mixed required optional and keyword args

### update

```ruby
#: (String, ?Integer, name: String) -> void
def mixed(a, b = 0, name:)
end
```

### result

```rbs
class Object < BasicObject
  def mixed: (String a, ?Integer b, name: String) -> void
end
```

## Continuation line with modifier

### update

`sorbet/config`

```ruby
.
```

```ruby
class Api
  # @override
  #: (
  #|   String,
  #|   Integer
  #| ) -> Hash[Symbol, String]
  def fetch(url, timeout) = {}
end
```

### result

```rbs
class Api
  # @override
  def fetch: (String url, Integer timeout) -> Hash[Symbol, String]
end
```
