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

class NeverMatchesClass
  def self.===(_other) = false
end

class KindOfCaseParent
  def self.===(object) = object.is_a?(self)
end

class KindOfCaseChild < KindOfCaseParent; end

class FakeKindOf
  def is_a?(_type) = true
end

class AlwaysMatchesCase
  def self.===(_other) = true
end

module KindOfCaseNamespace
  class Parent
    def self.===(object) = object.is_a?(self)
  end

  class Child < Parent; end
end

module AlwaysMatchesClassObjects
  refine Class do
    def ===(_other) = true
  end
end

def inherited_kind_of_case_equality_narrows(flag)
  value = if flag
    KindOfCaseChild.new
  else
    "value"
  end
  case value
  when KindOfCaseParent
    value
  else
    :other
  end
end

def kind_of_case_equality_narrows_untyped(value)
  case value
  when KindOfCaseParent
    value
  else
    :other
  end
end

def overridden_is_a_keeps_case_union(flag)
  value = if flag
    FakeKindOf.new
  else
    "value"
  end
  case value
  when KindOfCaseParent
    value
  else
    :other
  end
end

def namespaced_kind_of_case_equality_narrows(flag)
  value = if flag
    KindOfCaseNamespace::Child.new
  else
    "value"
  end
  case value
  when KindOfCaseNamespace::Parent
    value
  else
    :other
  end
end

def arbitrary_true_case_equality_keeps_union(flag)
  value = if flag
    1
  else
    "value"
  end
  case value
  when AlwaysMatchesCase
    value
  else
    :other
  end
end

using AlwaysMatchesClassObjects

def refined_class_equality_keeps_union(flag)
  value = if flag
    1
  else
    "value"
  end
  if Integer === value
    value
  else
    :unreachable
  end
end

def refined_class_case_keeps_when_branch(flag)
  value = if flag
    1
  else
    "value"
  end
  case value
  when Integer
    value
  else
    :unreachable
  end
end

def refined_class_equality_respects_singleton_override(flag)
  value = if flag
    1
  else
    "value"
  end
  if NeverMatchesClass === value
    :matched
  else
    :other
  end
end

def refined_class_case_respects_singleton_override(flag)
  value = if flag
    1
  else
    "value"
  end
  case value
  when NeverMatchesClass
    :matched
  else
    :other
  end
end

def unknown_class_case_keeps_union(flag)
  value = if flag
    1
  else
    "value"
  end
  case value
  when MissingCaseClass
    value
  else
    :other
  end
end
```

### result

```rbs
class AlwaysMatchesCase
  def self.===: (untyped _other) -> true
end

class FakeKindOf
  def is_a?: (untyped _type) -> true
end

class KindOfCaseNamespace::Parent
  def self.===: (untyped object) -> bool
end

class KindOfCaseParent
  def self.===: (untyped object) -> bool
end

class NeverMatchesClass
  def self.===: ((Integer | String) _other) -> false
end

class Object < BasicObject
  def overridden_class_case_equality: (untyped flag) -> (1 | "value" | :other)
  def inherited_kind_of_case_equality_narrows: (untyped flag) -> (:other | KindOfCaseParent)
  def kind_of_case_equality_narrows_untyped: (untyped value) -> (untyped | :other)
  def overridden_is_a_keeps_case_union: (untyped flag) -> ("value" | :other | FakeKindOf)
  def namespaced_kind_of_case_equality_narrows: (untyped flag) -> (:other | KindOfCaseNamespace::Parent)
  def arbitrary_true_case_equality_keeps_union: (untyped flag) -> (1 | "value")
  def refined_class_equality_keeps_union: (untyped flag) -> (1 | "value")
  def refined_class_case_keeps_when_branch: (untyped flag) -> (1 | "value")
  def refined_class_equality_respects_singleton_override: (untyped flag) -> :other
  def refined_class_case_respects_singleton_override: (untyped flag) -> :other
  def unknown_class_case_keeps_union: (untyped flag) -> (1 | "value" | :other)
end

class String
  def self.===: ((Integer | String) other) -> bool
end
```
