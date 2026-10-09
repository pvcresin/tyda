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

```

### result

```rbs
class Array
  def !: -> true
end

class FalseClass
  def !: -> false
end

class Integer
  def ===: (untyped _other) -> true
end

class Object < BasicObject
  def ===: (untyped _other) -> true
  def negated_false: -> :falsy
  def array_negation_case: -> :matched
  def literal_case: -> :matched
  def range_case: -> :unmatched
  def inherited_literal_case: -> :matched
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
end

class Range
  def ===: (untyped _other) -> false
end

class String
  def self.===: ((Integer | String) _other) -> bool
end
```
