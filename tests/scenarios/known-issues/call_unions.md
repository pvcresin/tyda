# Known Issues / Call Unions

## An argument union with an untyped arm keeps the parameter untyped

```ruby
class UnionArgTarget
  def detail = 1
end

class UnionArgCaller
  def run(flag)
    value = flag ? UnionArgTarget.new : Hash.new.fetch(:a)
    accept(value)
  end

  def accept(value)
    value.other
  end
end
```

### result

```rbs
class UnionArgCaller
  def run: (untyped flag) -> untyped
  def accept: (untyped value) -> untyped
end

class UnionArgTarget
  def detail: -> 1
end
```
