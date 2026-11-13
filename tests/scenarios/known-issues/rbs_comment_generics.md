# Known Issues / RBS Comment Generics

## A class type parameter from @rbs generic

```ruby
# @rbs generic T
class Box
  #: (T) -> void
  def initialize(value) = @value = value

  #: -> T
  def get = @value
end
```

### result

```rbs
class Box[T]
  def initialize: (T value) -> void
  def get: -> T
end
```

## A class type parameter bound from @rbs generic

```ruby
# @rbs generic T < Numeric
class NumBox
  #: -> T
  def get = nil
end
```

### result

```rbs
class NumBox[T < Numeric]
  def get: -> T
end
```

## An interface embedded with @rbs!

```ruby
class Greeter
  # @rbs!
  #   interface _Named
  #     def name: () -> String
  #   end

  #: (_Named x) -> String
  def greet(x) = x.name
end
```

### result

```rbs
class Greeter
  interface _Named
    def name: -> String
  end

  def greet: (_Named x) -> String
end
```
