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

## A generic method with a block maps the type argument

```rbs
class Box[T]
  def map: [R] () { (T) -> R } -> Box[R]
  def get: -> T
end

class BoxSource
  def box: -> Box[Integer]
end
```

```ruby
class BoxMapper
  def mapped = BoxSource.new.box.map { |x| x.to_s }.get
end
```

### result

```rbs
class BoxMapper
  def mapped: -> String
end
```

## A user-defined interface bound keeps the argument class

```rbs
interface _Named
  def name: () -> String
end

class Dog
  def name: () -> String
end

class Picker
  def self.pick: [T < _Named] (T x) -> T
end
```

```ruby
class PickerUser
  def dog = Picker.pick(Dog.new)
  def dog_name = Picker.pick(Dog.new).name
end
```

### result

```rbs
class PickerUser
  def dog: -> Dog
  def dog_name: -> String
end
```

## A generic interface argument infers the type argument

```rbs
interface _Getter[T]
  def get: () -> T
end

class IntGetter
  def get: () -> Integer
end

class Opener
  def self.open: [T] (_Getter[T] getter) -> T
end
```

```ruby
class OpenerUser
  def value = Opener.open(IntGetter.new)
end
```

### result

```rbs
class OpenerUser
  def value: -> Integer
end
```

## A generic module included with a type argument

```rbs
module Holder[T]
  def held: -> T
end

class IntHolder
  include Holder[Integer]
end
```

```ruby
class HolderUser
  def value = IntHolder.new.held
end
```

### result

```rbs
class HolderUser
  def value: -> Integer
end
```

## A subclass of a generic superclass fixes the type argument

```rbs
class Box[T]
  def get: -> T
end

class IntBox < Box[Integer]
end
```

```ruby
class IntBoxUser
  def value = IntBox.new.get
end
```

### result

```rbs
class IntBoxUser
  def value: -> Integer
end
```

## A lower bound type parameter is read

```rbs
class Lower
  def self.keep: [T > Integer] (T x) -> T
end
```

```ruby
class LowerUser
  def value = Lower.keep(1)
end
```

### result

```rbs
class LowerUser
  def value: -> Integer
end
```
