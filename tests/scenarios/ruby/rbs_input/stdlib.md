# Ruby / RBS Input / Stdlib

## RBS definitions for multiple classes

### update

```rbs
class String
  def length: -> Integer
  def to_i: -> Integer
end

class Integer
  def to_f: -> Float
end
```

```ruby
def measure(s) = s.length
measure("hello")
```

### result

```rbs
class Object < BasicObject
  def measure: (String s) -> Integer
end
```

## RBS definition for module

### update

```rbs
module Kernel
  def puts: (*untyped args) -> nil
end
```

```ruby
def greet = "hello"
```

### result

```rbs
class Object < BasicObject
  def greet: -> "hello"
end
```
