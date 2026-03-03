# Ruby / Literal / Number

## Float plus Integer returns Float

### update

```ruby
class A
  def self.a = 1.0 + 1
  def self.b = 1 + 1.0
  def self.c = 1.0 * 2
end
```

### result

```rbs
class A
  def self.a: -> Float
  def self.b: -> Float
  def self.c: -> Float
end
```

## Rational and Complex literals

### update

```ruby
def rational_literal = 1r

def imaginary_literal = 1i
```

### result

```rbs
class Object
  def rational_literal: -> Rational
  def imaginary_literal: -> Complex
end
```
