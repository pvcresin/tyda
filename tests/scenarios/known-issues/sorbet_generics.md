# Known Issues / Sorbet Generics

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
