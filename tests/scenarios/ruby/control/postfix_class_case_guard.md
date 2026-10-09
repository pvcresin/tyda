## class equality narrows postfix conditional assignment

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

## Keep dynamic dispatch for a singleton equality override

### update

```ruby
class String
  def self.===(other) = other.is_a?(String)
end

def overridden_class_case_equality(flag)
  value = if flag
    1
  else
    "value"
  end
  if String === value
    value
  else
    :other
  end
end
```

### result

```rbs
class Object < BasicObject
  def overridden_class_case_equality: (untyped flag) -> (1 | "value" | :other)
end

class String
  def self.===: ((Integer | String) other) -> bool
end
```
