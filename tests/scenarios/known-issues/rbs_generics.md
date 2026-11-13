# Known Issues / RBS Generics

## A generic class infers its type argument from new

```rbs
class Box[T]
  def initialize: (T value) -> void
  def get: -> T
end
```

```ruby
class BoxUser
  def box = Box.new(1)
  def value = Box.new(1).get
end
```

### result

```rbs
class BoxUser
  def box: -> Box[Integer]
  def value: -> Integer
end
```

## A generic RBS method keeps working with a source definition in the same file

```rbs
class Ident
  def self.id: [T] (T x) -> T
end
```

```ruby
class Ident
  def self.id(x) = x
end

class IdentUser
  def symbol = Ident.id(:s)
end
```

### result

```rbs
class Ident
  def self.id: [T] (T x) -> T
end

class IdentUser
  def symbol: -> :s
end
```
