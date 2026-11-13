# Known Issues / Sorbet Generics

## A sig type parameter renders as a method type parameter

```ruby
class Container
  sig { type_parameters(:U).params(x: T.type_parameter(:U)).returns(T.type_parameter(:U)) }
  def identity(x) = x
end
```

### result

```rbs
class Container
  def identity: [U] (U x) -> U
end
```

## A sig type parameter binds at the call site

```ruby
class Container
  sig { type_parameters(:U).params(x: T.type_parameter(:U)).returns(T.type_parameter(:U)) }
  def identity(x) = x
end

class ContainerUser
  def value = Container.new.identity(:s)
end
```

### result

```rbs
class Container
  def identity: [U] (U x) -> U
end

class ContainerUser
  def value: -> :s
end
```

## A covariant type_member keeps its variance

```ruby
class Producer
  Elem = type_member(:out)

  sig { returns(Elem) }
  def get
  end
end
```

### result

```rbs
class Producer[out Elem]
  def get: -> Elem
end
```

## A type_member upper bound becomes a type parameter bound

```ruby
class NumBox
  Elem = type_member { { upper: Numeric } }

  sig { returns(Elem) }
  def get
  end
end
```

### result

```rbs
class NumBox[Elem < Numeric]
  def get: -> Elem
end
```

## A generic class from RBI binds its type argument

```rbi
class Box
  Elem = type_member

  sig { returns(Elem) }
  def get; end
end

class BoxSource
  sig { returns(Box[Integer]) }
  def box; end
end
```

```ruby
class BoxUser
  def value = BoxSource.new.box.get
end
```

### result

```rbs
class BoxUser
  def value: -> Integer
end
```

## An RBI type parameter binds at the call site

```rbi
class Ident
  sig { type_parameters(:U).params(x: T.type_parameter(:U)).returns(T.type_parameter(:U)) }
  def self.id(x); end
end
```

```ruby
class IdentUser
  def value = Ident.id(:s)
end
```

### result

```rbs
class IdentUser
  def value: -> :s
end
```

## A Sorbet-style class type parameter comment keeps its variance

`sorbet/config`

```ruby
.
```

```ruby
#: [out Elem]
class Producer
  #: -> Elem
  def get
  end
end
```

### result

```rbs
class Producer[out Elem]
  def get: -> Elem
end
```

## Extending T::Generic in Ruby keeps it out of the class declaration

```ruby
class Producer
  extend T::Generic
  Elem = type_member

  sig { returns(Elem) }
  def get
  end
end
```

### result

```rbs
class Producer[Elem]
  def get: -> Elem
end
```

## Calling through an RBI interface module keeps T::Helpers out of the output

```rbi
module Named
  extend T::Helpers
  interface!

  sig { abstract.returns(String) }
  def name; end
end

class Finder
  sig { returns(Named) }
  def self.find; end
end
```

```ruby
class FinderUser
  def value = Finder.find.name
end
```

### result

```rbs
class FinderUser
  def value: -> String
end
```
