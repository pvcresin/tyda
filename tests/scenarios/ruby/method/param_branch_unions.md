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
  def default_when_absent: (untyped x) -> (untyped | 0)
  def ternary_default: (untyped x) -> (untyped | 0)
  def or_default: (untyped flag) -> (0 | "value")
end
```
