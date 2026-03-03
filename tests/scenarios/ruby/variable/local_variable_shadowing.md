# Ruby / Variable / Local Variable Shadowing

## Same-name local assignment shadows method

### update

```ruby
class A
  def x = :method

  def call
    x = :local
    x
  end
end
```

### result

```rbs
class A
  def x: -> :method
  def call: -> :local
end
```

## `name()` forces method call

### update

```ruby
class A
  def x = :method

  def call
    x = :local
    x()
  end
end
```

### result

```rbs
class A
  def x: -> :method
  def call: -> :method
end
```

## Assignment inside unreachable if false introduces local

### update

```ruby
class A
  def x = :method

  def call
    if false
      x = :local
    end
    x
  end
end
```

### result

```rbs
class A
  def x: -> :method
  def call: -> :local
end
```

## Local assignment shadows attr_reader method

### update

```ruby
class A
  attr_reader :name

  def initialize
    @name = "alice"
  end

  def value
    name = "bob"
    name
  end
end
```

### result

```rbs
class A
  def name: -> "alice"
  def initialize: -> void
  def value: -> "bob"
end
```

## `self.name` calls attr_reader despite same-name local

### update

```ruby
class A
  attr_reader :name

  def initialize
    @name = "alice"
  end

  def value
    name = "bob"
    self.name
  end
end
```

### result

```rbs
class A
  def name: -> "alice"
  def initialize: -> void
  def value: -> "alice"
end
```

## Setter without `self.` is local assignment

### update

```ruby
class A
  attr_accessor :name

  def initialize
    @name = "alice"
  end

  def rename
    name = "bob"
    @name
  end
end
```

### result

```rbs
class A
  def name: -> "alice"
  def name=: (String name) -> "alice"
  def initialize: -> void
  def rename: -> "alice"
end
```

## Reference before assignment is method then local

### update

```ruby
class A
  def x = :method

  def f
    a = x
    x = :local
    [a, x]
  end
end
```

### result

```rbs
class A
  def x: -> :method
  def f: -> [:method, :local]
end
```

## Right side of `x = x` is local not method

### update

```ruby
class A
  def x = :method

  def f
    x = x
    x
  end
end
```

### result

```rbs
class A
  def x: -> :method
  def f: -> nil
end
```

## `||=` is treated as local assignment

### update

```ruby
class A
  def x = :method

  def f
    x ||= :local
    x
  end
end
```

### result

```rbs
class A
  def x: -> :method
  def f: -> :local
end
```

## `self.name = ...` calls setter method

### update

```ruby
class A
  attr_accessor :name

  def initialize
    @name = "alice"
  end

  def rename
    self.name = "bob"
  end
end
```

### result

```rbs
class A
  def name: -> String
  def name=: (String name) -> "alice"
  def initialize: -> void
  def rename: -> "bob"
end
```
