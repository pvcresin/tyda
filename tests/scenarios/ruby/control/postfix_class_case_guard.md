## class equality narrows postfix conditional assignment

```yaml
known_issue: true
```

### update

```ruby
def quote_string_pattern(flag)
  pattern = if flag
    "value"
  else
    /value/
  end
  pattern = Regexp.new(Regexp.quote(pattern) + "$") if String === pattern
  pattern
end
```

### result

```rbs
class Object < BasicObject
  def quote_string_pattern: (untyped flag) -> Regexp
end
```
