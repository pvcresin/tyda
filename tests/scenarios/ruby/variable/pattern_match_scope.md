# Ruby / Variable / Pattern Match Scope

## Hash pattern binding introduces local in enclosing scope

### update

```ruby
class A
  def f
    case { v: :matched }
    in { v: x }
      x
    end
  end
end
```

### result

```rbs
class A
  def f: -> nil | untyped
end
```

## Pattern variable shadows method and only `x()` calls method

### update

```ruby
class A
  def x = :method

  def f
    case { v: :pattern_value }
    in { v: x }
      [x, x()]
    end
  end
end
```

### result

```rbs
class A
  def x: -> :method
  def f: -> [untyped, :method]?
end
```

## Array pattern introduces multiple local bindings

### update

```ruby
class A
  def f
    case [1, "two"]
    in [a, b]
      [a, b]
    end
  end
end
```

### result

```rbs
class A
  def f: -> [untyped, untyped]?
end
```
