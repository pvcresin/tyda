# Sorbet / Sig / Struct Prop Scoped Types

## T::Array type argument resolves in lexical scope

```ruby
module NS
  class Inner < T::Struct
    prop :x, Integer
  end

  class S < T::Struct
    prop :xs, T::Array[Inner]
  end
end
```

### result

```rbs
class NS::Inner < T::Struct
  def x: -> Integer
  def x=: (Integer x) -> Integer
end

class NS::S < T::Struct
  def xs: -> Array[NS::Inner]
  def xs=: (Array[NS::Inner] xs) -> Array[NS::Inner]
end
```

## T.nilable member resolves in lexical scope

```ruby
module NS
  class Inner < T::Struct
    prop :x, Integer
  end

  class S < T::Struct
    prop :y, T.nilable(Inner)
  end
end
```

### result

```rbs
class NS::Inner < T::Struct
  def x: -> Integer
  def x=: (Integer x) -> Integer
end

class NS::S < T::Struct
  def y: -> NS::Inner?
  def y=: (NS::Inner? y) -> NS::Inner?
end
```

## Composite type arguments resolve forward references

```ruby
module NS
  class S < T::Struct
    prop :xs, T::Array[Inner]
    prop :h, T::Hash[String, Inner]
    prop :u, T.any(Inner, Integer)
  end

  class Inner < T::Struct
    prop :x, Integer
  end
end
```

### result

```rbs
class NS::Inner < T::Struct
  def x: -> Integer
  def x=: (Integer x) -> Integer
end

class NS::S < T::Struct
  def xs: -> Array[NS::Inner]
  def xs=: (Array[NS::Inner] xs) -> Array[NS::Inner]
  def h: -> Hash[String, NS::Inner]
  def h=: (Hash[String, NS::Inner] h) -> Hash[String, NS::Inner]
  def u: -> Integer | NS::Inner
  def u=: ((Integer | NS::Inner) u) -> (Integer | NS::Inner)
end
```

## Type argument prefers lexical scope over top-level

```ruby
class Inner < T::Struct
  prop :t, String
end

module NS
  class S < T::Struct
    prop :xs, T::Array[Inner]
  end

  class Inner < T::Struct
    prop :x, Integer
  end
end
```

### result

```rbs
class Inner < T::Struct
  def t: -> String
  def t=: (String t) -> String
end

class NS::Inner < T::Struct
  def x: -> Integer
  def x=: (Integer x) -> Integer
end

class NS::S < T::Struct
  def xs: -> Array[NS::Inner]
  def xs=: (Array[NS::Inner] xs) -> Array[NS::Inner]
end
```

## Unresolved type argument keeps the bare name

```ruby
module NS
  class S < T::Struct
    prop :ms, T::Array[Missing]
    prop :mix, T::Array[T.nilable(Inner)]
  end

  class Inner < T::Struct
    prop :x, Integer
  end
end
```

### result

```rbs
class NS::Inner < T::Struct
  def x: -> Integer
  def x=: (Integer x) -> Integer
end

class NS::S < T::Struct
  def ms: -> Array[Missing]
  def ms=: (Array[Missing] ms) -> Array[Missing]
  def mix: -> Array[NS::Inner?]
  def mix=: (Array[NS::Inner?] mix) -> Array[NS::Inner?]
end
```
