# Sorbet / RBS Comment / Modifiers

## Keep # @override modifier

### update

`sorbet/config`

```ruby
.
```

```ruby
class Child < Base
  # @override
  #: (Integer) -> String
  def convert(x) = x.to_s
end
```

### result

```rbs
class Child < Base
  # @override
  def convert: (Integer x) -> String
end
```

## Keep # @abstract modifier

### update

`sorbet/config`

```ruby
.
```

```ruby
class Base
  # @abstract
  #: (String) -> void
  def process(data)
    raise "not implemented"
  end
end
```

### result

```rbs
class Base
  # @abstract
  def process: (String data) -> void
end
```

## Keep # @final modifier

### update

`sorbet/config`

```ruby
.
```

```ruby
class Service
  # @final
  #: () -> Integer
  def version = 1
end
```

### result

```rbs
class Service
  # @final
  def version: -> Integer
end
```

## Type parameter comment and modifier before class

### update

`sorbet/config`

```ruby
.
```

```ruby
# @requires_ancestor: ::Kernel
#: [out Elem]
class Box
  #: -> Elem
  def first
  end
end
```

### result

```rbs
# @requires_ancestor: ::Kernel
class Box[Elem]
  def first: -> Elem
end
```

## Keep class and module modifier comments

### update

`sorbet/config`

```ruby
.
```

```ruby
# @abstract
class Base
end

# @interface
module M
  #: -> void
  def call
  end
end
```

### result

```rbs
# @abstract
class Base
end

# @interface
module M
  def call: -> void
end
```

## Keep method modifier comment with args

### update

`sorbet/config`

```ruby
.
```

```ruby
class Child
  # @override(allow_incompatible: true)
  #: (Integer) -> String
  def value(x) = x.to_s
end
```

### result

```rbs
class Child
  # @override(allow_incompatible: true)
  def value: (Integer x) -> String
end
```

## Type parameter comment on class << self

### update

`sorbet/config`

```ruby
.
```

```ruby
module Factory
  #: [InstanceType]
  class << self
    #: -> InstanceType
    def make = nil
  end
end
```

### result

```rbs
module Factory[InstanceType]
  def self.make: -> InstanceType
end
```

## Type parameter comment on module

### update

`sorbet/config`

```ruby
.
```

```ruby
#: [Elem]
module BoxLike
  #: (Elem) -> Elem
  def id(x) = x
end
```

### result

```rbs
module BoxLike[Elem]
  def id: (Elem x) -> Elem
end
```
