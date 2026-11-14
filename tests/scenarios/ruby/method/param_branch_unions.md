# Ruby / Method / Parameter Branch Unions

## A branch that returns an untyped parameter keeps it in the union

### update

```ruby
class Guards
  def default_when_absent(x)
    return 0 unless x
    x
  end

  def ternary_default(x)
    x.nil? ? 0 : x
  end

  def or_default(flag)
    if flag
      "value"
    else
      0
    end
  end
end
```

### result

```rbs
class Guards
  def default_when_absent: (untyped x) -> untyped
  def ternary_default: (untyped x) -> untyped
  def or_default: (untyped flag) -> (0 | "value")
end
```

## An argument that may be an untyped value or nil stays untyped

### update

```ruby
class OptionalUntypedArgument
  def positional(x)
    take(x ? x : nil)
  end

  def keyword(x)
    take_keyword(project: x || nil)
  end

  def take(project) = project
  def take_keyword(project:) = project
end
```

### result

```rbs
class OptionalUntypedArgument
  def positional: (untyped x) -> untyped
  def keyword: (untyped x) -> untyped
  def take: [P] (P project) -> P
  def take_keyword: (project: untyped) -> untyped
end
```

## A call in an unreachable branch adds nothing to the parameter

### update

```ruby
class UnreachableArgument
  def run
    value = nil
    take(value) if value
  end

  def mixed
    value = nil
    take_mixed(value) if value
    take_mixed("text")
  end

  def take(value) = value
  def take_mixed(value) = value
end
```

### result

```rbs
class UnreachableArgument
  def run: -> nil
  def mixed: -> String
  def take: [V] (V value) -> V
  def take_mixed: [V] (V value) -> V
end
```
