# Ruby / Control / Conditional Assign

## Local variable ||= assignment

### update

```ruby
def foo
  x = nil
  x ||= "hello"
  x
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> "hello"
end
```

## Local variable &&= assignment

### update

```ruby
def bar
  x = "hello"
  x &&= 42
  x
end
```

### result

```rbs
class Object < BasicObject
  def bar: -> 42
end
```

## ||= on false uses right-hand side

### update

```ruby
def from_false
  x = false
  x ||= "hello"
end
```

### result

```rbs
class Object < BasicObject
  def from_false: -> "hello"
end
```

## Instance variable ||= assignment

### update

```ruby
class Cache
  def get
    @value ||= "default"
  end
end
```

### result

```rbs
class Cache
  def get: -> "default"
end
```

## Instance variable &&= assignment

### update

```ruby
class Config
  def update
    @setting &&= "new_value"
  end
end
```

### result

```rbs
class Config
  def update: -> nil
end
```

## &&= on false keeps false

### update

```ruby
def keep_false
  x = false
  x &&= 42
end
```

### result

```rbs
class Object < BasicObject
  def keep_false: -> false
end
```

## Return type from ||= assignment

### update

```ruby
def compute(x)
  result = x
  result ||= 0
  result
end
compute("hello")
```

### result

```rbs
class Object < BasicObject
  def compute: (String x) -> String
end
```

## Assignment in a condition narrows the bound variable

### update

```ruby
class Parser
  def first_capture
    if m = "hello world".match(/(\w+)/)
      m[1]
    end
  end

  def all_pre
    out = []
    while m = "abc".match(/(\w)/)
      out << m.pre_match
      break
    end
    out
  end
end
```

### result

```rbs
class Parser
  def first_capture: -> String?
  def all_pre: -> Array[String]
end
```
