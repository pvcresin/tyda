# Ruby / Variable / Instance

## Unresolved ivar returns untyped

### update

```ruby
class A
  def missing = @missing
end
```

### result

```rbs
class A
  def missing: -> untyped
end
```

## Instance variable set in initialize

### update

```ruby
class A
  def initialize
    @x = "a"
  end
  def foo = @x
end
```

### result

```rbs
class A
  def initialize: -> void
  def foo: -> "a"
end
```

## Multiple instance variables

### update

```ruby
class A
  def initialize
    @x = "a"
    @y = 30
  end
  def foo = @x
  def bar = @y
end
```

### result

```rbs
class A
  def initialize: -> void
  def foo: -> "a"
  def bar: -> 30
end
```

## Assign instance variable in method

### update

```ruby
class A
  def initialize
    @x = 0
  end
  def foo
    @x = 1
    @x
  end
end
```

### result

```rbs
class A
  def initialize: -> void
  def foo: -> 0 | 1
end
```

## Instance variables from initialize with multiple args

### update

```ruby
class A
  def initialize(x, y, z)
    @x = x
    @y = y
    @z = z
  end
end

A.new(1, 1.0, "a")
```

### result

```rbs
class A
  def initialize: (Integer x, Float y, String z) -> void
end
```

## Set in initialize and return in method

### update

```ruby
class A
  def initialize(n)
    @n = n
  end

  def value = @n
end

A.new(42)
```

### result

```rbs
class A
  def initialize: (Integer n) -> void
  def value: -> 42
end
```

## Assign literal in initialize and return in method

### update

```ruby
class A
  def initialize(x)
    @x = 42
  end

  def foo = @x
end
```

### result

```rbs
class A
  def initialize: (untyped x) -> void
  def foo: -> 42
end
```

## Reuse ivar inside same `def self.x`

### update

```ruby
module M
  def self.count
    @count ||= 4
    @count + 1
  end

  def self.cached
    @cached ||= "hello"
    @cached.length
  end
end
```

### result

```rbs
module M
  def self.count: -> Integer
  def self.cached: -> 5
end
```

## `instance_variable_get` with static ivar name

### update

```ruby
class A
  def initialize(name, count)
    @name = name
    @count = count
  end

  def name_value = instance_variable_get(:@name)
  def count_value = instance_variable_get("@count")
  def missing_value = instance_variable_get(:@missing)
end

A.new("item", 1)
```

### result

```rbs
class A
  def initialize: (String name, Integer count) -> void
  def name_value: -> "item"
  def count_value: -> 1
  def missing_value: -> untyped
end
```

## Instance variable reflection with interpolated name

### update

```ruby
class A
  def read_name
    @name = "item"
    key = "name"
    instance_variable_get("@#{key}")
  end

  def set_count
    key = "count"
    instance_variable_set(:"@#{key}", 1)
    @count
  end

  def read_union(flag)
    @name = "item"
    @count = 1
    key = flag ? "name" : "count"
    instance_variable_get("@#{key}")
  end

  def present
    key = "name"
    instance_variable_defined?("@#{key}")
  end
end
```

### result

```rbs
class A
  def read_name: -> "item"
  def set_count: -> 1
  def read_union: (untyped flag) -> (1 | "item")
  def present: -> bool
end
```

## `instance_variable_set` affects return and later ivar read

### update

```ruby
class A
  def set_name
    instance_variable_set(:@name, "item")
  end

  def set_count
    instance_variable_set("@count", 1)
    @count
  end

  def set_via_send
    public_send(:instance_variable_set, :@flag, true)
    send(:instance_variable_get, :@flag)
  end
end
```

### result

```rbs
class A
  def set_name: -> "item"
  def set_count: -> 1
  def set_via_send: -> true
end
```

## `instance_variable_get` on explicit receiver reads known ivar

### update

```ruby
class Item
  def initialize(name)
    @name = name
  end
end

class Box
  def initialize(item)
    @item = item
  end

  def item_name
    @item.instance_variable_get(:@name)
  end

  def class_value
    Item.instance_variable_set(:@kind, :entry)
    Item.instance_variable_get(:@kind)
  end
end

Box.new(Item.new("item"))
```

### result

```rbs
class Box
  def initialize: (Item item) -> void
  def item_name: -> "item"
  def class_value: -> :entry
end

class Item
  def initialize: (String name) -> void
end
```

## `instance_variable_get` accepts static to_sym names

### update

```ruby
class Item
  def initialize
    @name = "item"
    @count = 1
  end

  def read_name
    field = "name"
    instance_variable_get("@#{field}".to_sym)
  end

  def read_union(flag)
    field = flag ? "name" : "count"
    instance_variable_get("@#{field}".intern)
  end
end
```

### result

```rbs
class Item
  def initialize: -> void
  def read_name: -> "item"
  def read_union: (untyped flag) -> (1 | "item")
end
```

## Subclass method reads instance variable from superclass initialize

### update

```ruby
class Base
  def initialize(x)
    @x = "42"
  end

  def from_base(_)
    @x
  end
end

class Sub < Base
  def from_sub(_)
    @x
  end
end
```

### result

```rbs
class Base
  def initialize: (untyped x) -> void
  def from_base: (untyped _) -> "42"
end

class Sub < Base
  def from_sub: (untyped _) -> "42"
end
```

## Read-before-write ivar is treated as a falsey guard

### update

```ruby
class Sentinel
  def update
    return "skip" if @warning_issued
    @warning_issued = true
    "first call"
  end
end
```

### result

```rbs
class Sentinel
  def update: -> "first call" | "skip"
end
```

## Self-referential lvar nil check is true

### update

```ruby
def foo
  x = x.nil?
  x
end
```

### result

```rbs
class Object
  def foo: -> true
end
```
