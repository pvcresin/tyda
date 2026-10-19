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
