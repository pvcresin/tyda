# Ruby / RBS Input / Superclass RBS

## Call RBS method from subclass

### update

```rbs
class Base
  def id: -> Integer
  def name: -> String
end
```

```ruby
class Base
  def id = 1
  def name = "base"
end
class Child < Base
  def label = "child"
end
def test_inherited_id
  c = Child.new
  c.id
end
def test_inherited_name
  c = Child.new
  c.name
end
def test_own
  c = Child.new
  c.label
end
```

### result

```rbs
class Base
  def id: -> Integer
  def name: -> String
end

class Child < Base
  def label: -> "child"
end

class Object < BasicObject
  def test_inherited_id: -> Integer
  def test_inherited_name: -> String
  def test_own: -> "child"
end
```

## Chain from inherited method return type

### update

```rbs
class Base
  def name: -> String
end
```

```ruby
class Base
  def name = "base"
end
class Child < Base
  def label = "child"
end
def test_chain
  c = Child.new
  c.name.length
end
```

### result

```rbs
class Base
  def name: -> String
end

class Child < Base
  def label: -> "child"
end

class Object < BasicObject
  def test_chain: -> Integer
end
```

## Method override

### update

```rbs
class Base
  def value: -> Integer
end
```

```ruby
class Base
  def value = 42
end
class Override < Base
  def value = "hello"
end
def test_base
  b = Base.new
  b.value
end
def test_override
  o = Override.new
  o.value
end
```

### result

```rbs
class Base
  def value: -> Integer
end

class Object < BasicObject
  def test_base: -> Integer
  def test_override: -> "hello"
end

class Override < Base
  def value: -> "hello"
end
```

## Deep inheritance

### update

```rbs
class Animal
  def sound: -> String
end
```

```ruby
class Animal
  def sound = "..."
end
class Dog < Animal
  def bark = "woof"
end
class Puppy < Dog
  def play = "yay"
end
def test_grandchild
  p = Puppy.new
  p.sound
end
def test_child
  p = Puppy.new
  p.bark
end
def test_own
  p = Puppy.new
  p.play
end
```

### result

```rbs
class Animal
  def sound: -> String
end

class Dog < Animal
  def bark: -> "woof"
end

class Object < BasicObject
  def test_grandchild: -> String
  def test_child: -> "woof"
  def test_own: -> "yay"
end

class Puppy < Dog
  def play: -> "yay"
end
```

## Resolve inherited method without RBS

### update

```ruby
class Vehicle
  def wheels = 4
end
class Car < Vehicle
  def brand = "Toyota"
end
def test_inherited
  c = Car.new
  c.wheels
end
def test_own
  c = Car.new
  c.brand
end
```

### result

```rbs
class Car < Vehicle
  def brand: -> "Toyota"
end

class Object < BasicObject
  def test_inherited: -> 4
  def test_own: -> "Toyota"
end

class Vehicle
  def wheels: -> 4
end
```

## Use inherited method type in subclass chain

### update

```rbs
class NumberProvider
  def value: -> Integer
end
```

```ruby
class NumberProvider
  def value = 42
end
class SpecialProvider < NumberProvider
  def label = "special"
end
def test
  sp = SpecialProvider.new
  sp.value.to_f
end
```

### result

```rbs
class NumberProvider
  def value: -> Integer
end

class Object < BasicObject
  def test: -> Float
end

class SpecialProvider < NumberProvider
  def label: -> "special"
end
```
