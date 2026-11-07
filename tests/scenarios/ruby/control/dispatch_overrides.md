# Ruby / Control / Dispatch Overrides

## Array index and slice facts require standard dispatch

### update

```ruby
def overridden_index_after_nonempty_guard
  values = [1]
  def values.[](_index) = nil
  return :empty if values.empty?
  values[0]
end

def overridden_constructor_index_after_nonempty_guard
  values = Array.new(1, 1)
  def values.[](_index) = nil
  return :empty if values.empty?
  values[0]
end

def overridden_slice_after_nonempty_guard
  values = [1]
  def values.slice(_index) = nil
  return :empty if values.empty?
  values.slice(0)
end
```

### result

```rbs
class Object < BasicObject
  def overridden_index_after_nonempty_guard: -> nil
  def overridden_constructor_index_after_nonempty_guard: -> nil
  def overridden_slice_after_nonempty_guard: -> nil
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

def module_case_when_override(flag)
  value = flag ? 1 : "value"
  case value
  when String then value
  else :other
  end
end

def module_case_in_override(flag)
  value = flag ? 1 : "value"
  case value
  in String then value
  else :other
  end
end
```

### result

```rbs
class Module
  def ===: ((Integer | String) other) -> bool
end

class Object < BasicObject
  def module_case_equality_override: (untyped flag) -> (1 | :other)
  def module_case_when_override: (untyped flag) -> (1 | "value" | :other)
  def module_case_in_override: (untyped flag) -> (1 | "value" | :other)
end
```

## Source case equality bodies select the branch per argument

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
  def ===: ((Integer | String) other) -> bool
end

class Object < BasicObject
  def module_case_equality_override: (untyped flag) -> (1 | :other)
end
```

## Source case equality bodies that do not narrow keep both branches

### update

```ruby
class Module
  def ===(other)
    other.is_a?(Integer)
    other.frozen?
  end
end

class WeakGuard
  def self.===(other) = other.is_a?(Integer) || other.frozen?
end

class TwoParameterGuard
  def self.===(other, strict = false) = other.is_a?(Integer)
end

def several_statements(flag)
  value = flag ? 1 : "value"
  if String === value then value else :other end
end

def one_sided_or(flag)
  value = flag ? 1 : "value"
  if WeakGuard === value then value else :other end
end

def optional_parameter(flag)
  value = flag ? 1 : "value"
  if TwoParameterGuard === value then value else :other end
end
```

### result

```rbs
class Module
  def ===: ((Integer | String) other) -> bool
end

class Object < BasicObject
  def several_statements: (untyped flag) -> (1 | "value" | :other)
  def one_sided_or: (untyped flag) -> (1 | "value" | :other)
  def optional_parameter: (untyped flag) -> (1 | "value" | :other)
end

class TwoParameterGuard
  def self.===: ((Integer | String) other, ?bool strict) -> bool
end

class WeakGuard
  def self.===: ((Integer | String) other) -> bool
end
```

## Source case equality narrows through negation and inheritance

### update

```ruby
class EvenGuard
  def self.===(other) = !other.is_a?(String)
end

class SubGuard < EvenGuard
end

class OverridingGuard < EvenGuard
  def self.===(_other) = false
end

def negated(flag)
  value = flag ? 1 : "value"
  if EvenGuard === value then value else :other end
end

def inherited(flag)
  value = flag ? 1 : "value"
  if SubGuard === value then value else :other end
end

def overridden(flag)
  value = flag ? 1 : "value"
  if OverridingGuard === value then value else :other end
end

def unless_form(flag)
  value = flag ? 1 : "value"
  return :string unless EvenGuard === value
  value
end
```

### result

```rbs
class EvenGuard
  def self.===: ((Integer | String) other) -> bool
end

class Object < BasicObject
  def negated: (untyped flag) -> (1 | :other)
  def inherited: (untyped flag) -> (1 | :other)
  def overridden: (untyped flag) -> :other
  def unless_form: (untyped flag) -> (1 | :string)
end

class OverridingGuard < EvenGuard
  def self.===: ((Integer | String) _other) -> false
end
```

## Class-specific overrides do not use built-in case equality narrowing

### update

```ruby
class CaseTarget
  def self.===(_other) = true
end

def class_case_equality_override(flag)
  value = if flag
    1
  else
    "value"
  end
  if CaseTarget === value
    value
  else
    :other
  end
end
```

### result

```rbs
class CaseTarget
  def self.===: ((Integer | String) _other) -> true
end

class Object < BasicObject
  def class_case_equality_override: (untyped flag) -> (1 | "value")
end
```

## Class patterns use source-defined case equality conservatively

### update

```ruby
class PatternTarget
  def self.===(_other) = [true, false].sample
end

def class_pattern_equality_override(flag)
  value = if flag
    1
  else
    "value"
  end

  case value
  in PatternTarget
    value
  else
    :other
  end
end
```

### result

```rbs
class Object < BasicObject
  def class_pattern_equality_override: (untyped flag) -> (1 | "value" | :other)
end

class PatternTarget
  def self.===: (untyped _other) -> bool
end
```

## Operator writes prefer source-defined operators

### update

```ruby
class ProductOverride
  def *(other) = :product
end

def source_defined_multiply_write
  product = ProductOverride.new
  product *= 2
  product
end
```

### result

```rbs
class Object < BasicObject
  def source_defined_multiply_write: -> :product
end

class ProductOverride
  def *: (untyped other) -> :product
end
```
