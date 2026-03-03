# Ruby / Runtime / Math

## Math singleton methods return Float

### update

```ruby
class Probe
  def sqrt    = Math.sqrt(2)
  def sin     = Math.sin(1.0)
  def cos     = Math.cos(0.0)
  def tan     = Math.tan(0.5)
  def log     = Math.log(2.71828)
  def log2    = Math.log2(8)
  def log10   = Math.log10(100)
  def exp     = Math.exp(1.0)
  def hypot   = Math.hypot(3, 4)
  def atan2   = Math.atan2(1.0, 1.0)
  def cbrt    = Math.cbrt(27)
end
```

### result

```rbs
class Probe
  def sqrt: -> Float
  def sin: -> Float
  def cos: -> Float
  def tan: -> Float
  def log: -> Float
  def log2: -> Float
  def log10: -> Float
  def exp: -> Float
  def hypot: -> Float
  def atan2: -> Float
  def cbrt: -> Float
end
```

## Math constants

### update

```ruby
class Probe
  def pi    = Math::PI
  def e     = Math::E
end
```

### result

```rbs
class Probe
  def pi: -> Float
  def e: -> Float
end
```
