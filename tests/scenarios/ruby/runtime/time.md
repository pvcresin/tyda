# Ruby / Runtime / Time

## Time constructors and accessors

### update

```ruby
class Probe
  def now    = Time.now
  def at     = Time.at(0)
  def year   = Time.now.year
  def month  = Time.now.month
  def to_i   = Time.now.to_i
  def to_f   = Time.now.to_f
  def utc    = Time.now.utc
end
```

### result

```rbs
class Probe
  def now: -> Time
  def at: -> Time
  def year: -> Integer
  def month: -> Integer
  def to_i: -> Integer
  def to_f: -> Float
  def utc: -> Time
end
```

## Time arithmetic

### update

```ruby
class Probe
  def plus_seconds  = Time.now + 60
  def minus_seconds = Time.now - 60
  def time_diff     = Time.now - Time.at(0)
  def strftime      = Time.now.strftime("%Y")
end
```

### result

```rbs
class Probe
  def plus_seconds: -> Time
  def minus_seconds: -> Time
  def time_diff: -> Float
  def strftime: -> String
end
```
