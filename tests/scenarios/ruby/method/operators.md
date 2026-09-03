# Ruby / Method / Operators

## Operator method definition

### update

```ruby
class A
  def +(other)
    42
  end
  def -(other)
    42
  end
  def *(other)
    42
  end
end
```

### result

```rbs
class A
  def +: (untyped other) -> 42
  def -: (untyped other) -> 42
  def *: (untyped other) -> 42
end
```

## Arithmetic with integer addition

### update

```ruby
def foo
  a = 1 + 2
  a
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> Integer
end
```

## Multiple arithmetic operations

### update

```ruby
def arith
  a = 1 - 2
  b = 3 * 4
  c = 10 / 2
  d = 10 % 3
  e = 2 ** 8
  [a, b, c, d, e]
end
```

### result

```rbs
class Object < BasicObject
  def arith: -> [Integer, Integer, Integer, Integer, Numeric]
end
```

## Comparison operator ==

### update

```ruby
def compare(a, b) = a == b
compare(1, 2)
```

### result

```rbs
class Object < BasicObject
  def compare: (Integer a, Integer b) -> bool
end
```

## Comparison operators

### update

```ruby
def comparisons
  a = 1 < 2
  b = 1 > 2
  c = 1 <= 2
  d = 1 >= 2
  e = 1 <=> 2
  a
end
```

### result

```rbs
class Object < BasicObject
  def comparisons: -> bool
end
```

## String concatenation

### update

```ruby
def string_concat = "hello" + " world"
```

### result

```rbs
class Object < BasicObject
  def string_concat: -> String
end
```

## Not equal operator

### update

```ruby
def not_equal = 1 != 2
```

### result

```rbs
class Object < BasicObject
  def not_equal: -> bool
end
```

## Case equality operator

### update

```ruby
def case_eq = /hello/ === "hello world"
```

### result

```rbs
class Object < BasicObject
  def case_eq: -> bool
end
```

## Bitwise operators

### update

```ruby
def bitwise
  a = 0xFF & 0x0F
  b = 0xF0 | 0x0F
  c = 0xFF ^ 0x0F
  [a, b, c]
end
```

### result

```rbs
class Object < BasicObject
  def bitwise: -> [Integer, Integer, Integer]
end
```

## Shift operators

### update

```ruby
def shift_ops
  a = 1 << 8
  b = 256 >> 4
  [a, b]
end
```

### result

```rbs
class Object < BasicObject
  def shift_ops: -> [Integer, Integer]
end
```

## Unary not operator

### update

```ruby
def unary_ops
  a = !true
  b = -1
  a
end
```

### result

```rbs
class Object < BasicObject
  def unary_ops: -> false
end
```

## Unary minus

### update

```ruby
def negate
  a = 1
  -a
end
```

### result

```rbs
class Object < BasicObject
  def negate: -> Integer
end
```

## Bitwise not operator

### update

```ruby
def bit_not = ~0xFF
```

### result

```rbs
class Object < BasicObject
  def bit_not: -> Integer
end
```

## Index access

### update

```ruby
def index_test
  arr = [1, 2, 3]
  arr[0]
end
```

### result

```rbs
class Object < BasicObject
  def index_test: -> 1
end
```

## Array << operator

### update

```ruby
def shift_test
  arr = [1, 2, 3]
  arr << 4
end
```

### result

```rbs
class Object < BasicObject
  def shift_test: -> [1, 2, 3, 4]
end
```

## Define arithmetic operator methods

### update

```ruby
class Vector
  def initialize(x, y)
    @x = x
    @y = y
  end

  def +(other)
    Vector.new(@x, @y)
  end

  def -(other)
    Vector.new(@x, @y)
  end

  def %(other)
    0
  end
end
Vector.new(1, 2)
```

### result

```rbs
class Vector
  def initialize: ((Integer | untyped) x, (Integer | untyped) y) -> void
  def +: (untyped other) -> Vector
  def -: (untyped other) -> Vector
  def %: (untyped other) -> 0
end
```

## Define equality operator methods

### update

```ruby
class Value
  def ==(other)
    true
  end

  def !=(other)
    false
  end

  def ===(other)
    true
  end
end
```

### result

```rbs
class Value
  def ==: (untyped other) -> true
  def !=: (untyped other) -> false
  def ===: (untyped other) -> true
end
```

## Define ordering operator methods

### update

```ruby
class Value
  def <=>(other)
    0
  end

  def <(other)
    true
  end

  def >(other)
    false
  end

  def <=(other)
    true
  end

  def >=(other)
    false
  end
end
```

### result

```rbs
class Value
  def <=>: (untyped other) -> 0
  def <: (untyped other) -> true
  def >: (untyped other) -> false
  def <=: (untyped other) -> true
  def >=: (untyped other) -> false
end
```

## Define index and shift operator methods

### update

```ruby
class Vector
  def initialize(x, y)
    @x = x
    @y = y
  end

  def [](index)
    @x
  end

  def []=(index, value)
    @x = value
  end

  def <<(item)
    self
  end

  def >>(amount)
    self
  end
end
Vector.new(1, 2)
```

### result

```rbs
class Vector
  def initialize: (Integer x, Integer y) -> void
  def []: (untyped index) -> (1 | 2)
  def []=: (untyped index, untyped value) -> untyped
  def <<: (untyped item) -> Vector
  def >>: (untyped amount) -> Vector
end
```

## Define bitwise and unary operator methods

### update

```ruby
class Vector
  def &(other)
    self
  end

  def |(other)
    self
  end

  def ^(other)
    self
  end

  def ~
    self
  end

  def -@
    self
  end

  def +@
    self
  end
end
```

### result

```rbs
class Vector
  def &: (untyped other) -> Vector
  def |: (untyped other) -> Vector
  def ^: (untyped other) -> Vector
  def ~: -> Vector
  def -@: -> Vector
  def +@: -> Vector
end
```
