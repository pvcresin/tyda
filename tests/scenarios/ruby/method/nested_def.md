# Ruby / Method / Nested Def

## def inside an instance method defines a method on the class

```yaml
known_issue: true
```

### update

```ruby
class Foo
  def setup
    def inner = 1
    :ok
  end
end
```

### result

```rbs
class Foo
  def setup: -> :ok
  def inner: -> 1
end
```
