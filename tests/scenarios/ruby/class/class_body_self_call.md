# Ruby / Class / Class Body Self Call

## Call class << self singleton setter in same module body

### update

```ruby
module Utils
  class << self
    attr_accessor :default_parser
  end
  self.default_parser = 42
end
```

### result

```rbs
module Utils
  def self.default_parser: -> untyped
  def self.default_parser=: (Integer default_parser) -> untyped
end
```

## Setter interaction with `self.y = v`

### update

```ruby
module Utils
  class << self
    attr_accessor :default_parser
  end
  self.default_parser = 42

  def self.param_depth_limit=(v)
    self.default_parser = v
  end
end
```

### result

```rbs
module Utils
  def self.default_parser: -> untyped
  def self.default_parser=: (Integer default_parser) -> untyped
  def self.param_depth_limit=: (untyped v) -> untyped
end
```

## Call `SomeClass.x = y` in top-level class body

### update

```ruby
class Store
  class << self
    attr_accessor :registry
  end
end

Store.registry = {}

def read = Store.registry
```

### result

```rbs
class Object < BasicObject
  def read: -> untyped
end

class Store
  def self.registry: -> untyped
  def self.registry=: (Hash[untyped, untyped] registry) -> untyped
end
```
