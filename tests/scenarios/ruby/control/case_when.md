# Ruby / Control / Case When

## Basic case when

### update

```ruby
def check(x)
  case x
  when 1
    "one"
  when 2
    "two"
  else
    "other"
  end
end
check(1)
```

### result

```rbs
class Object < BasicObject
  def check: (Integer x) -> ("one" | "other" | "two")
end
```

## case when with range

### update

```ruby
def range_case(x)
  case x
  when 1..5
    "low"
  when 6..10
    "high"
  end
end
range_case(3)
```

### result

```rbs
class Object < BasicObject
  def range_case: (Integer x) -> ("high" | "low")?
end
```

## branch-only local is nilable after case

```yaml
known_issue: true
```

### update

```ruby
def case_branch_local(value)
  case value
  when Integer
    selected = :number
  when String
    selected = :text
  end
  selected
end
```

### result

```rbs
class Object < BasicObject
  def case_branch_local: (untyped value) -> (:number | :text)?
end
```

## assignment in an exiting case branch is not reachable afterwards

```yaml
known_issue: true
```

### update

```ruby
def case_branch_local_after_return(value)
  case value
  when Integer
    selected = :number
    return :returned
  else
    :other
  end
  selected
end
```

### result

```rbs
class Object < BasicObject
  def case_branch_local_after_return: (untyped value) -> :returned?
end
```

## ignore code after every case branch exits

### update

```ruby
def case_all_paths_return(value)
  case value
  when Integer
    selected = :number
    return :number
  else
    return :other
  end
  selected
end
```

### result

```rbs
class Object < BasicObject
  def case_all_paths_return: (untyped value) -> (:number | :other)
end
```

## case when with class

### update

```ruby
def class_case(x)
  case x
  when Integer
    "integer"
  when String
    "string"
  when Symbol
    "symbol"
  else
    "other"
  end
end
class_case(42)
```

### result

```rbs
class Object < BasicObject
  def class_case: (Integer x) -> "integer"
end
```

## case without expression

### update

```ruby
def no_expr_case(x)
  case
  when x > 0
    "positive"
  when x < 0
    "negative"
  else
    "zero"
  end
end
no_expr_case(1)
```

### result

```rbs
class Object < BasicObject
  def no_expr_case: (Integer x) -> ("negative" | "positive" | "zero")
end
```

## case when narrows class before method call

### update

```ruby
class Formatter
  #: (untyped) -> String
  def format(x)
    case x
    when Integer
      x.to_s
    when String
      x.upcase
    else
      "unknown"
    end
  end
end
```

### result

```rbs
class Formatter
  def format: (untyped x) -> String
end
```

## case narrowing keeps possible descendant matches

### update

```rbs
module CaseWhenMarker
end

class CaseWhenBase
end

class CaseWhenChild < CaseWhenBase
  include CaseWhenMarker
  def child_value: -> Integer
end

class CaseWhenOther
end

class CaseWhenFactory
  def self.base_value: -> CaseWhenBase
  def self.basic_object_value: -> BasicObject
  def self.mixed_value: -> (CaseWhenBase | CaseWhenOther)
  def self.marker_value: -> CaseWhenMarker
end
```

```ruby
module CaseWhenMarker
end

class CaseWhenBase
end

class CaseWhenChild < CaseWhenBase
  include CaseWhenMarker

  def child_value = 1
end

class CaseWhenOther
end

def possible_module_case(value)
  case value
  when CaseWhenMarker
    :matched
  else
    :other
  end
end

def preserve_residual_after_subclass_case(value)
  case value
  when CaseWhenChild
    value.child_value
  when CaseWhenOther
    :unreachable
  else
    "base"
  end
end

def assigned_case_preserves_residual(value)
  result = case value
  when CaseWhenChild
    value.child_value
  when CaseWhenOther
    :unreachable
  else
    "base"
  end
  result
end

def implicit_object_case_matches(value)
  case value
  when Object
    value
  else
    :unreachable
  end
end

def implicit_basic_object_case_matches(value)
  case value
  when BasicObject
    value
  else
    :unreachable
  end
end

def object_case_keeps_basic_object_residual(value)
  case value
  when Object
    :object
  else
    :basic_object
  end
end

def module_case_may_match_class(value)
  case value
  when CaseWhenBase
    :base
  else
    :other
  end
end

def matched_class_value(value)
  value
end

def residual_union_value(value)
  case value
  when CaseWhenChild
    :child
  else
    case_when_residual(value)
  end
end

def case_when_residual(value)
  value
end

def multiple_class_targets(value)
  case value
  when CaseWhenChild, CaseWhenOther
    matched_class_value(value)
  else
    :base
  end
end

possible_module_case(CaseWhenFactory.base_value)
preserve_residual_after_subclass_case(CaseWhenFactory.base_value)
assigned_case_preserves_residual(CaseWhenFactory.base_value)
implicit_object_case_matches(CaseWhenFactory.base_value)
implicit_basic_object_case_matches(CaseWhenFactory.base_value)
object_case_keeps_basic_object_residual(CaseWhenFactory.basic_object_value)
module_case_may_match_class(CaseWhenFactory.marker_value)
residual_union_value(CaseWhenFactory.mixed_value)
multiple_class_targets(CaseWhenFactory.mixed_value)
```

### result

```rbs
class CaseWhenChild < CaseWhenBase
  include CaseWhenMarker

  def child_value: -> Integer
end

class Object < BasicObject
  def possible_module_case: (CaseWhenBase value) -> (:matched | :other)
  def preserve_residual_after_subclass_case: (CaseWhenBase value) -> (Integer | "base")
  def assigned_case_preserves_residual: (CaseWhenBase value) -> (Integer | "base")
  def implicit_object_case_matches: (CaseWhenBase value) -> CaseWhenBase
  def implicit_basic_object_case_matches: (CaseWhenBase value) -> CaseWhenBase
  def object_case_keeps_basic_object_residual: (BasicObject value) -> (:basic_object | :object)
  def module_case_may_match_class: (CaseWhenMarker value) -> (:base | :other)
  def matched_class_value: ((CaseWhenChild | CaseWhenOther) value) -> (CaseWhenChild | CaseWhenOther)
  def residual_union_value: ((CaseWhenBase | CaseWhenOther) value) -> (:child | CaseWhenBase | CaseWhenOther)
  def case_when_residual: ((CaseWhenBase | CaseWhenOther) value) -> (CaseWhenBase | CaseWhenOther)
  def multiple_class_targets: ((CaseWhenBase | CaseWhenOther) value) -> (:base | CaseWhenChild | CaseWhenOther)
end
```

## unresolved dynamic superclass keeps class case reachable

### update

```ruby
class DynamicCaseBase
end

class DynamicCaseAlternate
end

class DynamicCaseChild < (true ? DynamicCaseBase : DynamicCaseAlternate)
end

def dynamic_superclass_case(value)
  case value
  when DynamicCaseBase
    :matched
  else
    :other
  end
end

dynamic_superclass_case(DynamicCaseChild.new)
```

### result

```rbs
class Object < BasicObject
  def dynamic_superclass_case: (DynamicCaseChild value) -> (:matched | :other)
end
```

## unresolved Class.new superclass keeps class case reachable

### update

```ruby
class DynamicNewCaseBase
end

class DynamicNewCaseAlternate
end

DynamicNewCaseChild = Class.new(true ? DynamicNewCaseBase : DynamicNewCaseAlternate)

def dynamic_new_superclass_case(value)
  case value
  when DynamicNewCaseBase
    :matched
  else
    :other
  end
end

dynamic_new_superclass_case(DynamicNewCaseChild.new)
```

### result

```rbs
class Object < BasicObject
  def dynamic_new_superclass_case: (DynamicNewCaseChild value) -> (:matched | :other)
end
```

## Struct.new classes keep the Struct ancestor

### update

```ruby
class StructCaseChild < Struct.new(:value)
end

def struct_parent_case(value)
  case value
  when Struct
    :matched
  else
    :other
  end
end

struct_parent_case(StructCaseChild.new(1))
```

### result

```rbs
class Object < BasicObject
  def struct_parent_case: (StructCaseChild value) -> :matched
end

class StructCaseChild
  def value: -> Integer
  def value=: (Integer value) -> Integer
  def initialize: (Integer value) -> void
  def self.members: -> Array[:value]
end
```

## case when narrowing changes method return

### update

```ruby
def describe(val)
  case val
  when Integer
    val + 1
  when String
    val.length
  else
    nil
  end
end
describe(42)
```

### result

```rbs
class Object < BasicObject
  def describe: (Integer val) -> Integer
end
```

## case when with multiple conditions

### update

```ruby
def numeric_check(x)
  case x
  when Integer, Float
    x.to_f
  when String
    x.to_f
  end
end
numeric_check(42)
```

### result

```rbs
class Object < BasicObject
  def numeric_check: (Integer x) -> Float
end
```

## case when return can be nil

### update

```ruby
def nil_case(value)
  case value
  when Integer
    nil
  else
    "other"
  end
end
nil_case(1)
```

### result

```rbs
class Object < BasicObject
  def nil_case: (Integer value) -> nil
end
```

## `when ... then` form

### update

```ruby
def case_then(x)
  case x
  when 1 then :one
  when 2 then :two
  else :other
  end
end
```

### result

```rbs
class Object < BasicObject
  def case_then: (untyped x) -> (:one | :other | :two)
end
```

## case when narrows record discriminant

### update

```ruby
class RecordDiscriminantCase
  #: ({ kind: :text, value: String } | { kind: :count, value: Integer }) -> (Integer | Symbol)?
  def value(x)
    case x[:kind]
    when :text
      x[:value].to_sym
    when :count
      x[:value] + 1
    else
      nil
    end
  end
end
```

### result

```rbs
class RecordDiscriminantCase
  def value: (({ kind: :count, value: Integer } | { kind: :text, value: String }) x) -> (Integer | Symbol)?
end
```

## case when narrowing resolves RBS method return

### update

```rbs
class Stringifiable
  def stringify: -> String
end

class Numerable
  def to_num: -> Integer
end
```

```ruby
def convert(obj)
  case obj
  when Stringifiable
    obj.stringify
  when Numerable
    obj.to_num
  else
    nil
  end
end
```

### result

```rbs
class Object < BasicObject
  def convert: (untyped obj) -> (Integer | String)?
end
```

## Pattern matching in Ruby 3+

```yaml
known_issue: true
```

### update

```ruby
def pattern_match(x)
  case x
  in Integer
    "integer"
  in String
    "string"
  end
end
pattern_match(42)
```

### result

```rbs
class Object < BasicObject
  def pattern_match: (Integer x) -> "integer"
end
```

## Use pattern match captured local in return

```yaml
known_issue: true
```

### update

```ruby
def capture_match(x)
  if x in Integer => captured
    captured
  end
end

capture_match(1)
```

### result

```rbs
class Object < BasicObject
  def capture_match: (Integer x) -> Integer
end
```

## Case when reassigns a shared local

### update

```ruby
def test(type, val)
  case type
  when :int
    val = val.to_i
  when :sym
    val = val.to_sym
  else
    val = val
  end
  val
end

test(:int, "42")
test(:sym, "hello")
```

### result

```rbs
class Object < BasicObject
  def test: (Symbol type, String val) -> (Integer | String | Symbol)
end
```

## Case when unions Integer or String

### update

```ruby
def foo(x)
  case x
  when Integer, String
    x
  else
    :other
  end
end

foo(1)
foo("s")
foo(:a)
```

### result

```rbs
class Object < BasicObject
  def foo: ((Integer | String | Symbol) x) -> (Integer | String | :other)
end
```
