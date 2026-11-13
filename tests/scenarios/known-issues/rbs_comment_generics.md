# Known Issues / RBS Comment Generics

## A method type parameter in a #: comment

```ruby
class Ident
  #: [T] (T x) -> T
  def id(x) = x
end

class IdentUser
  def symbol = Ident.new.id(:s)
end
```

### result

```rbs
class Ident
  def id: [T] (T x) -> T
end

class IdentUser
  def symbol: -> :s
end
```

## A union bound in a #: comment

```ruby
class Bounded
  #: [T < Integer | String] (T x) -> T
  def keep(x) = x
end
```

### result

```rbs
class Bounded
  def keep: [T < Integer | String] (T x) -> T
end
```

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
