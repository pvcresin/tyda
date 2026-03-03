# Sorbet / Sig / Sorbet Assertions

## T.let explicit variable type

### update

```ruby
class Asserter
  def with_let
    x = T.let(42, Integer)
    x
  end
end
```

### result

```rbs
class Asserter
  def with_let: -> Integer
end
```

## T.cast type cast

### update

```ruby
class Caster
  def with_cast(x) = T.cast(x, String)
end
```

### result

```rbs
class Caster
  def with_cast: (untyped x) -> String
end
```

## T.must removes nil from nilable type

### update

```ruby
class MustUser
  #: (Integer | nil) -> Integer
  def unwrap(x) = T.must(x)
end
```

### result

```rbs
class MustUser
  def unwrap: (Integer? x) -> Integer
end
```

## T.unsafe treats value as untyped

### update

```ruby
class UnsafeUser
  def dangerous = T.unsafe("hello")
end
```

### result

```rbs
class UnsafeUser
  def dangerous: -> untyped
end
```

## T.absurd bot type

### update

```ruby
class AbsurdUser
  def never_returns(x) = T.absurd(x)
end
```

### result

```rbs
class AbsurdUser
  def never_returns: (untyped x) -> bot
end
```

## T.bind block self type

### update

```ruby
class Binder
  def with_bind
    result = T.bind(self, Binder)
    result
  end
end
```

### result

```rbs
class Binder
  def with_bind: -> Binder
end
```

## T.must_because removes nil with a reason block

### update

```ruby
class MustReason
  #: (String | nil) -> String
  def unwrap(value) = T.must_because(value) { "missing" }
end
```

### result

```rbs
class MustReason
  def unwrap: (String? value) -> String
end
```

## T.let with complex type

### update

```ruby
class ComplexLet
  def with_array
    arr = T.let([], Array)
    arr
  end
end
```

### result

```rbs
class ComplexLet
  def with_array: -> Array
end
```

## Type alias with T.type_alias

### update

```ruby
class Aliases
  MyString = T.type_alias { String }

  def name
    x = T.let("hello", MyString)
    x
  end
end
```

### result

```rbs
class Aliases
  def name: -> String
end
```

## T.nilable T.let narrows through guard

### update

```ruby
class NilableLetGuard
  def normalize
    name = T.let(nil, T.nilable(String))
    if name.nil?
      "missing"
    else
      name.upcase
    end
  end
end
```

### result

```rbs
class NilableLetGuard
  def normalize: -> String
end
```

## T.any T.let narrows through is_a?

### update

```ruby
class AnyLetNarrowing
  def convert(flag)
    value = T.let(flag ? "name" : 1, T.any(String, Integer))
    if value.is_a?(String)
      value.to_sym
    else
      value + 1
    end
  end
end
```

### result

```rbs
class AnyLetNarrowing
  def convert: (untyped flag) -> (Integer | Symbol)
end
```

## T::Array T.let propagates to block arg

### update

```ruby
class ArrayLetGeneric
  def shout
    items = T.let(["a"], T::Array[String])
    items.map { |item| item.upcase }
  end
end
```

### result

```rbs
class ArrayLetGeneric
  def shout: -> Array[String]
end
```

## T::Hash T.let keeps Hash type

### update

```ruby
class HashLetGeneric
  def table
    T.let({ name: "a" }, T::Hash[Symbol, String])
  end
end
```

### result

```rbs
class HashLetGeneric
  def table: -> Hash[Symbol, String]
end
```

## T.all T.let keeps intersection

### update

```ruby
module Named
  def name = "n"
end

class NamedItem
  include Named
end

class AllLetIntersection
  def item
    T.let(NamedItem.new, T.all(NamedItem, Named))
  end
end
```

### result

```rbs
class AllLetIntersection
  def item: -> NamedItem & Named
end

module Named
  def name: -> "n"
end

class NamedItem
  include Named
end
```

## Tuple T.let narrows index access

### update

```ruby
class TupleLetAssertion
  def second
    pair = T.let([1, "a"], [Integer, String])
    pair[1].upcase
  end
end
```

### result

```rbs
class TupleLetAssertion
  def second: -> String
end
```

## Keep custom generic syntax in T.let

### update

```ruby
class Box
end

class GenericLetAssertion
  def box
    T.let(Box.new, Box[Integer])
  end
end
```

### result

```rbs
class GenericLetAssertion
  def box: -> Box[Integer]
end
```

## T.class_of T.let becomes singleton type

### update

```ruby
class ClassOfLetAssertion
  def string_class
    T.let(String, T.class_of(String))
  end
end
```

### result

```rbs
class ClassOfLetAssertion
  def string_class: -> singleton(String)
end
```
