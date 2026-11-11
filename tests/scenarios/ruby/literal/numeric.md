# Ruby / Literal / Numeric

## Integer literal

### update

```ruby
def numeric_literals
  a = 1
  b = 1_000
  c = 0xff
  d = 0b1010
  e = 0o777
  a
end
```

### result

```rbs
class Object < BasicObject
  def numeric_literals: -> 1
end
```

## Float literal

### update

```ruby
def float_literals
  a = 1.0
  b = 1.0e10
  a
end
```

### result

```rbs
class Object < BasicObject
  def float_literals: -> Float
end
```

## Rational literal

### update

```ruby
def rational_test
  r = 1/3r
  r
end
```

### result

```rbs
class Object < BasicObject
  def rational_test: -> Rational
end
```

## Complex literal

### update

```ruby
def complex_test
  c = 1i
  c
end
```

### result

```rbs
class Object < BasicObject
  def complex_test: -> Complex
end
```

## clamp and between? on Integer literals

### update

```ruby
def low = -5.clamp(0, 10)
def mid = 5.clamp(0, 10)
def high = 100.clamp(0, 10)
def between = 5.between?(0, 10)
```

### result

```rbs
class Object < BasicObject
  def low: -> (-5 | 0 | 10)
  def mid: -> (0 | 5 | 10)
  def high: -> (0 | 10 | 100)
  def between: -> bool
end
```

## divmod of Integer literals is a two-element Integer tuple

### update

```ruby
def parts = 10.divmod(3)
```

### result

```rbs
class Object < BasicObject
  def parts: -> [Integer, Integer]
end
```

## Integer arithmetic with Rational and Float literals

### update

```ruby
class Ratio
  def half = 1 / 2r
  def mixed = 1 + 1.5
end
```

### result

```rbs
class Ratio
  def half: -> Rational
  def mixed: -> Float
end
```

## divmod destructuring binds Integer parts

### update

```ruby
class Splitter
  def remainder
    _quotient, rest = 7.divmod(2)
    rest
  end
end
```

### result

```rbs
class Splitter
  def remainder: -> Integer
end
```
