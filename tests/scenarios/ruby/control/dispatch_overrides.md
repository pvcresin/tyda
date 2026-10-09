# Ruby / Control / Dispatch Overrides

## Array index and slice facts require standard dispatch

### update

```ruby
def overridden_index_after_nonempty_guard
  values = Array.new(1, 1)
  def values.[](_index) = nil
  return :empty if values.empty?
  values[0]
end

def overridden_slice_after_nonempty_guard
  values = Array.new(1, 1)
  def values.slice(_index) = nil
  return :empty if values.empty?
  values.slice(0)
end
```

### result

```rbs
class Object < BasicObject
  def overridden_index_after_nonempty_guard: -> :empty?
  def overridden_slice_after_nonempty_guard: -> :empty?
end
```

## Module case equality overrides do not narrow as Class#===

### update

```ruby
class Module
  def ===(other)
    other.is_a?(Integer)
  end
end

def module_case_equality_override(flag)
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
class Module
  def ===: (untyped other) -> bool
end

class Object < BasicObject
  def module_case_equality_override: (untyped flag) -> (1 | "value" | :other)
end
```
