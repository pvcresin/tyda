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

## A value checked by break unless is yielded as non-nil

```ruby
class YieldReadParts
  def each_part(file, len)
    while len > 0
      part = file.read(len)
      break unless part
      len -= part.length
      yield part
    end
  end
end
```

### result

```rbs
class YieldReadParts
  def each_part: (untyped file, untyped len) { (String) -> untyped } -> nil
end
```
