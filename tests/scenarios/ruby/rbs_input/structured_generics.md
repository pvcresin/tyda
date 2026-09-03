# Ruby / RBS Input / Structured Generics

## RBS generic return type round-trips through inference

```rbs
class Foo
end

class Bar
  def make: -> Foo[Bar]
end
```

```ruby
def use_generic(bar)
  bar.make
end

use_generic(Bar.new)
```

### result

```rbs
class Object < BasicObject
  def use_generic: (Bar bar) -> Foo[Bar]
end
```

## RBS nested generic return type round-trips

```rbs
class Wrap
  def wrapped: -> Foo[Array[Foo[Integer]]]
end
```

```ruby
def use_nested(wrap)
  wrap.wrapped
end

use_nested(Wrap.new)
```

### result

```rbs
class Object < BasicObject
  def use_nested: (Wrap wrap) -> Foo[Array[Foo[Integer]]]
end
```

## RBS self type argument round-trips

```rbs
class Item
  def relatives: -> Enumerable[self]
end
```

```ruby
def use_self(item)
  item.relatives
end

use_self(Item.new)
```

### result

```rbs
class Object < BasicObject
  def use_self: (Item item) -> Enumerable[Item]
end
```
