# Sorbet / Sig / RBI Sorbet Assertions

## T.let in RBI method definition

### update

```rbi
# typed: strict
class TypedClass
  sig { params(x: Integer).returns(String) }
  def convert(x); end
end
```

```ruby
class TypedClass
  def use_let
    x = T.let(42, Integer)
    x
  end
end
```

### result

```rbs
class TypedClass
  def use_let: -> Integer
end
```

## Multiple sig overloads in RBI

### update

```rbi
# typed: strict
class Overloaded
  sig { params(x: Integer).returns(String) }
  sig { params(x: String).returns(Integer) }
  def convert(x); end
end
```

```ruby
class Overloaded
  def test = convert(42)
end
```

### result

```rbs
class Overloaded
  def test: -> String
end
```

## T::Struct in RBI

### update

```rbi
# typed: strict
class Coordinate < T::Struct
  const :lat, Float
  const :lng, Float
end
```

```ruby
class Coordinate
  def to_s = "coord"
end
```

### result

```rbs
class Coordinate < T::Struct
  def to_s: -> "coord"
end
```
