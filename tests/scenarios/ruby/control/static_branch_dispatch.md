## Open class operators affect static branch selection

```yaml
known_issue: true
```

### update

```ruby
class Array
  def ! = true
end

class FalseClass
  def ! = false
end

class Integer
  def ===(_other) = true
end

class Range
  def ===(_other) = false
end

class Object
  def ===(_other) = true
end

class String
  def self.===(_other) = true
end

def negated_false
  if !false
    :truthy
  else
    :falsy
  end
end

def array_negation_case
  case ![]
  when true
    :matched
  else
    :unmatched
  end
end

def literal_case
  case 1
  when 2
    :matched
  else
    :unmatched
  end
end

def range_case
  case 1
  when 1..2
    :matched
  else
    :unmatched
  end
end

def inherited_literal_case
  case 1
  when :symbol
    :matched
  else
    :unmatched
  end
end

def parenthesized_predicate_case
  case (1)
  when 1
    :matched
  else
    :unmatched
  end
end

def parenthesized_condition_case
  case 1
  when (2)
    :other
  when (1)
    :matched
  else
    :unmatched
  end
end

def parenthesized_range_condition_case
  case 1
  when (1..2)
    :matched
  else
    :unmatched
  end
end

def negated_predicate_case
  case !false
  when false
    :matched
  else
    :unmatched
  end
end

def short_circuit_predicate_case
  case true && false
  when false
    :matched
  else
    :unmatched
  end
end

def parenthesized_composite_predicate_case
  case (true && false)
  when false
    :matched
  else
    :unmatched
  end
end

def nested_negated_predicate_case
  case !!false
  when false
    :matched
  else
    :unmatched
  end
end

def ternary_predicate_case
  case true ? 1 : 2
  when 1
    :matched
  else
    :unmatched
  end
end

def ternary_condition_case
  case 1
  when true ? 2 : 1
    :matched
  else
    :unmatched
  end
end

def ternary_condition_truthiness_case
  case
  when false ? :truthy : false
    :unmatched
  else
    :matched
  end
end

def unless_condition_truthiness_case
  case
  when unless true
         :truthy
       else
         false
       end
    :unmatched
  else
    :matched
  end
end

def class_case_equality_keeps_union(flag)
  value = if flag
    1
  else
    "value"
  end
  if String === value
    value
  else
    :unreachable
  end
end

def regexp_case
  subject = String.new
  case subject
  when /needle/
    :matched
  else
    :unmatched
  end
end

```

### result

```rbs
class Array[E]
  def !: -> true
end

class FalseClass
  def !: -> false
end

class Integer < Numeric
  def ===: (untyped _other) -> true
end

class Object < BasicObject
  def ===: (untyped _other) -> true
  def negated_false: -> :falsy
  def array_negation_case: -> :matched
  def literal_case: -> :matched
  def range_case: -> :unmatched
  def inherited_literal_case: -> :unmatched
  def parenthesized_predicate_case: -> :matched
  def parenthesized_condition_case: -> :other
  def parenthesized_range_condition_case: -> :unmatched
  def negated_predicate_case: -> :matched
  def short_circuit_predicate_case: -> :matched
  def parenthesized_composite_predicate_case: -> :matched
  def nested_negated_predicate_case: -> :matched
  def ternary_predicate_case: -> :matched
  def ternary_condition_case: -> :matched
  def ternary_condition_truthiness_case: -> :matched
  def unless_condition_truthiness_case: -> :matched
  def class_case_equality_keeps_union: (untyped flag) -> (1 | "value")
  def regexp_case: -> (:matched | :unmatched)
end

class Range[E] < Object
  def ===: (untyped _other) -> false
end

class String
  def self.===: ((Integer | String) _other) -> true
end
```

## Numeric case equality preserves cross-numeric values

```yaml
known_issue: true
```

### update

```ruby
def numeric_case_preserves_cross_numeric_branch(flag)
  value = if flag
    1
  else
    1.0
  end
  case value
  when 1
    value
  else
    :unmatched
  end
end

def numeric_case_subject_keeps_both_numeric_types(flag)
  if flag
    1
  else
    1.0
  end
end

def numeric_case_keeps_nonmatching_variant(flag)
  value = if flag
    1
  else
    2.0
  end
  case value
  when 1
    value
  else
    :unmatched
  end
end

def numeric_case_keeps_large_cross_numeric_unknown(flag)
  value = if flag
    9007199254740992.0
  else
    9007199254740993
  end
  case value
  when 9007199254740993
    value
  else
    :unmatched
  end
end
```

### result

```rbs
class Object < BasicObject
  def numeric_case_preserves_cross_numeric_branch: (untyped flag) -> (1 | Float)
  def numeric_case_subject_keeps_both_numeric_types: (untyped flag) -> (1 | Float)
  def numeric_case_keeps_nonmatching_variant: (untyped flag) -> (1 | Float | :unmatched)
  def numeric_case_keeps_large_cross_numeric_unknown: (untyped flag) -> (9007199254740993 | Float | :unmatched)
end
```

## Source-defined String class equality keeps its return

### update

```ruby
class String
  def self.===(_other) = true
end
```

### result

```rbs
class String
  def self.===: (untyped _other) -> true
end
```

## Overloaded case equality keeps both branches

### update

```rbs
class OverloadedMatcher
  def ===: (Integer) -> true
         | (String) -> false
end
```

```ruby
def overloaded_case_dispatch_keeps_both(flag)
  value = if flag
    1
  else
    "value"
  end
  if OverloadedMatcher.new === value
    :matched
  else
    :unmatched
  end
end
```

### result

```rbs
class Object < BasicObject
  def overloaded_case_dispatch_keeps_both: (untyped flag) -> (:matched | :unmatched)
end
```

## Side-effecting dispatch receivers keep both branches

### update

```ruby
class DispatchProbe
  def self.next(flag)
    @seen = flag
    flag ? [] : [1]
  end
end

def dynamic_receiver_empty_case(flag)
  if DispatchProbe.next(flag).empty?
    :empty
  else
    :nonempty
  end
end
```

### result

```rbs
class DispatchProbe
  def self.next: (untyped flag) -> Array[1]
end

class Object < BasicObject
  def dynamic_receiver_empty_case: (untyped flag) -> (:empty | :nonempty)
end
```
