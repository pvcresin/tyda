# Known Issues / Flow Values

## A self-recursive predicate keeps bool

```ruby
class RecursiveTopValue
  def x = 1
end

class RecursiveTop
  def top?(n)
    return false unless n.is_a?(RecursiveTopValue)
    n.x ? top?(n) : false
  end
end
```

### result

```rbs
class RecursiveTop
  def top?: (RecursiveTopValue n) -> bool
end

class RecursiveTopValue
  def x: -> 1
end
```
