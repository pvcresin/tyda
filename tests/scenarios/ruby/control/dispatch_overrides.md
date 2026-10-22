# Ruby / Control / Dispatch Overrides

## Array index and slice facts require standard dispatch

```yaml
known_issue: true
```

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
  def overridden_index_after_nonempty_guard: -> nil
  def overridden_slice_after_nonempty_guard: -> nil
end
```

## Module case equality overrides do not narrow as Class#===

```yaml
known_issue: true
```

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

def module_case_when_equality_override(flag)
  value = if flag
    1
  else
    "value"
  end
  case value
  when String
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
  def module_case_equality_override: (untyped flag) -> (1 | :other)
  def module_case_when_equality_override: (untyped flag) -> (1 | "value" | :other)
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
  def class_case_equality_override: (untyped flag) -> (1 | "value" | :other)
end
```

## `case` with a class-specific `===` override keeps both branches

### update

```ruby
class CaseWhenTarget
  def self.===(_other) = true
end

def class_case_when_equality_override
  value = String.new
  case value
  when CaseWhenTarget
    value
  else
    :other
  end
end
```

### result

```rbs
class CaseWhenTarget
  def self.===: (untyped _other) -> true
end

class Object < BasicObject
  def class_case_when_equality_override: -> :other | String
end
```

## Custom equality preserves observed input alternatives

### update

```rbs
class Object < BasicObject
  def open_case_unknown: -> untyped
end
```

```ruby
class OpenCaseCustomTarget
  def self.===(_other) = _other.nil?
end

class OpenCaseStandardTarget
end

def dynamic_case_nil_branch(value)
  value
end

def dynamic_case_nil_else(value)
  value
end

def dynamic_case_nil(value)
  case value
  when OpenCaseCustomTarget
    dynamic_case_nil_branch(value)
  else
    dynamic_case_nil_else(value)
  end
end

def dynamic_case_mixed_branch(value)
  value
end

def dynamic_case_mixed(value)
  case value
  when OpenCaseCustomTarget
    dynamic_case_mixed_branch(value)
  else
    :other
  end
end

def standard_case_branch(value)
  value
end

def standard_case_open(value)
  case value
  when OpenCaseStandardTarget
    standard_case_branch(value)
  else
    :other
  end
end

def dynamic_case_subsequent_branch(value)
  value
end

def dynamic_case_subsequent_else(value)
  value
end

def dynamic_case_with_subsequent_when(value)
  case value
  when OpenCaseCustomTarget
    :custom
  when OpenCaseStandardTarget
    dynamic_case_subsequent_branch(value)
  else
    dynamic_case_subsequent_else(value)
  end
end

dynamic_case_nil(nil)
dynamic_case_nil(open_case_unknown)
dynamic_case_mixed(nil)
dynamic_case_mixed("known")
dynamic_case_mixed(open_case_unknown)
standard_case_open(OpenCaseStandardTarget.new)
dynamic_case_with_subsequent_when(nil)
dynamic_case_with_subsequent_when("known")
dynamic_case_with_subsequent_when(open_case_unknown)
```

### result

```rbs
class Object < BasicObject
  def dynamic_case_nil_branch: (untyped? value) -> (nil | untyped)
  def dynamic_case_nil_else: (untyped? value) -> (nil | untyped)
  def dynamic_case_nil: (untyped? value) -> untyped?
  def dynamic_case_mixed_branch: ((String | untyped)? value) -> (String | nil | untyped)
  def dynamic_case_mixed: ((String | untyped)? value) -> (String | :other | untyped)?
  def standard_case_branch: (OpenCaseStandardTarget value) -> OpenCaseStandardTarget
  def standard_case_open: (OpenCaseStandardTarget value) -> OpenCaseStandardTarget
  def dynamic_case_subsequent_branch: (OpenCaseStandardTarget value) -> OpenCaseStandardTarget
  def dynamic_case_subsequent_else: ((String | untyped)? value) -> (String | nil | untyped)
  def dynamic_case_with_subsequent_when: ((String | untyped)? value) -> (String | :custom | OpenCaseStandardTarget | untyped)?
end

class OpenCaseCustomTarget
  def self.===: (untyped _other) -> bool
end
```

## Custom class equality keeps an object's custom `is_a?` result dynamic

### update

```ruby
class IsAKindCaseTarget
  def self.===(_other) = _other.is_a?(self)
end

class IsAKindCaseValue
  def is_a?(_other) = true
end

def custom_is_a_case_branch(value)
  value
end

def custom_is_a_case(value)
  case value
  when IsAKindCaseTarget
    custom_is_a_case_branch(value)
  else
    :other
  end
end

custom_is_a_case(IsAKindCaseValue.new)
```

### result

```rbs
class IsAKindCaseTarget
  def self.===: (untyped _other) -> bool
end

class IsAKindCaseValue
  def is_a?: (untyped _other) -> true
end

class Object < BasicObject
  def custom_is_a_case_branch: (IsAKindCaseValue value) -> IsAKindCaseValue
  def custom_is_a_case: (IsAKindCaseValue value) -> (:other | IsAKindCaseValue)
end
```

## `case` uses standard class equality after an unrelated extension

### update

```ruby
class CaseWhenEnumerableExtended
  extend Enumerable
end

def case_when_extended_match(value)
  value
end

def case_when_with_extended_class(condition)
  value = condition ? CaseWhenEnumerableExtended.new : "other"
  case value
  when CaseWhenEnumerableExtended
    case_when_extended_match(value)
  else
    :other
  end
end

case_when_with_extended_class(true)
case_when_with_extended_class(false)
```

### result

```rbs
class CaseWhenEnumerableExtended
  extend Enumerable
end

class Object < BasicObject
  def case_when_extended_match: (CaseWhenEnumerableExtended value) -> CaseWhenEnumerableExtended
  def case_when_with_extended_class: (bool condition) -> (:other | CaseWhenEnumerableExtended)
end
```

## `case` resolves class constants in the lexical namespace

### update

```ruby
class CaseWhenLexicalTarget; end

module CaseWhenLexical
  class CaseWhenLexicalTarget
    def self.===(_other) = true
  end

  def self.check
    value = String.new
    case value
    when CaseWhenLexicalTarget
      value
    else
      :other
    end
  end
end
```

### result

```rbs
module CaseWhenLexical
  def self.check: -> :other | String
end

class CaseWhenLexical::CaseWhenLexicalTarget
  def self.===: (untyped _other) -> true
end
```
