# Ruby / Control / Array Nonempty Alias

## Preserve nonempty knowledge through a local alias

```yaml
known_issue: true
```

### update

```ruby
def first_after_nonempty_guard
  values = [1].map { |value| value }
  return :empty if values.empty?
  values[0]
end

def first_after_nonempty_guard_alias
  values = [1].map { |value| value }
  return :empty if values.empty?
  alias_values = values
  alias_values[0]
end
```

### result

```rbs
class Object < BasicObject
  def first_after_nonempty_guard: -> 1
  def first_after_nonempty_guard_alias: -> 1
end
```
