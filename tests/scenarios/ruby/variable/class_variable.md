# Ruby / Variable / Class Variable

## Read and write class variable

### update

```ruby
class Counter
  @@count = 0

  def increment
    @@count += 1
  end

  def current = @@count
end
```

### result

```rbs
class Counter
  def increment: -> 1
  def current: -> 0 | 1
end
```

## Return `@@x` after `@@x ||= literal`

### update

```ruby
class A
  @@x = nil

  def self.ensure
    @@x ||= "loaded"
    @@x
  end
end
```

### result

```rbs
class A
  def self.ensure: -> "loaded"?
end
```

## Multiple class variable writes return union

### update

```ruby
class A
  @@state = :initial

  def advance
    @@state = :running
  end

  def finish
    @@state = :done
  end

  def current = @@state
end
```

### result

```rbs
class A
  def advance: -> :running
  def finish: -> :done
  def current: -> :done | :initial | :running
end
```

## Same-name class variables stay scoped

### update

```ruby
class First
  @@value = 1

  def self.value = @@value
end

class Second
  @@value = "two"

  def self.value = @@value
end
```

### result

```rbs
class First
  def self.value: -> 1
end

class Second
  def self.value: -> "two"
end
```

## Inherited class variable read

### update

```ruby
class Parent
  @@value = :base
end

class Child < Parent
  def self.value = @@value

  def value = self.class.class_variable_get(:@@value)
end
```

### result

```rbs
class Child < Parent
  def self.value: -> :base
  def value: -> :base
end
```

## Static class variable reflection

### update

```ruby
class Target
end

class Writer
  def value
    Target.class_variable_set(:@@value, 1)
    Target.class_variable_get(:@@value)
  end

  def flag
    Target.public_send(:class_variable_set, :@@flag, true)
    Target.send(:class_variable_get, :@@flag)
  end
end
```

### result

```rbs
class Writer
  def value: -> 1
  def flag: -> true
end
```

## Class variable reflection with interpolated name

### update

```ruby
class Store
  @@name = "item"
  @@count = 1

  def self.read_name
    key = "name"
    class_variable_get("@@#{key}")
  end

  def self.read_union(flag)
    key = flag ? "name" : "count"
    class_variable_get(:"@@#{key}")
  end

  def self.write_flag
    key = "flag"
    class_variable_set("@@#{key}", true)
    @@flag
  end
end
```

### result

```rbs
class Store
  def self.read_name: -> "item"
  def self.read_union: (untyped flag) -> (1 | "item")
  def self.write_flag: -> true
end
```

## Class variable reflection with static to_sym name

### update

```ruby
class Store
  @@name = "item"
  @@count = 1

  def self.read_name
    key = "name"
    class_variable_get("@@#{key}".to_sym)
  end

  def self.read_union(flag)
    key = flag ? "name" : "count"
    class_variable_get("@@#{key}".intern)
  end
end
```

### result

```rbs
class Store
  def self.read_name: -> "item"
  def self.read_union: (untyped flag) -> (1 | "item")
end
```

## Cross-file class variable reflection

### update

`lib/source.rb`

```ruby
class Source
  @@value = "stored"
end
```

```ruby
class Reader
  def value = Source.class_variable_get(:@@value)

  def present = Source.class_variable_defined?(:@@value)
end
```

### result

```rbs
class Reader
  def value: -> "stored"
  def present: -> bool
end
```
