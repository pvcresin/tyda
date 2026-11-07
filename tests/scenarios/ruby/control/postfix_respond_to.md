# Ruby / control / postfix respond_to?

## Narrow a postfix modifier receiver by respond_to?

### update

```ruby
def stringify_if_supported(value)
  value = value.to_str if value.respond_to?(:to_str)
  value
end

stringify_if_supported(nil)
stringify_if_supported("name")
```

### result

```rbs
class Object < BasicObject
  def stringify_if_supported: (String? value) -> String?
end
```
