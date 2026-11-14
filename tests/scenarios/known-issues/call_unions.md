# Known Issues / Call Unions

## An optional receiver picks an operator overload without its argument

```ruby
class Stamp
  def shifted(flag) = (flag ? Time.now : nil) - 3600

  def elapsed(flag) = (flag ? Time.now : nil) - Time.now
end
```

### result

```rbs
class Stamp
  def shifted: (untyped flag) -> Time
  def elapsed: (untyped flag) -> Float
end
```

