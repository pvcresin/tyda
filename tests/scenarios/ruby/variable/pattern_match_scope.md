# Ruby / Variable / Pattern Match Scope

## Hash pattern binding introduces local in enclosing scope

```yaml
known_issue: true
```

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
  def f: -> :matched
end
```

## Pattern variable shadows method and only `x()` calls method

```yaml
known_issue: true
```

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
  def f: -> [:pattern_value, :method]
end
```

## Array pattern introduces multiple local bindings

```yaml
known_issue: true
```

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
  def f: -> [1, "two"]
end
```
