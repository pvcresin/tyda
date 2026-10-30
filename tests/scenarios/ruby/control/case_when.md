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
  :unreachable
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

## case when narrowing changes method return

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

## class case keeps an unknown subject open

### update

```ruby
def unknown_class_case(value)
  case value
  when Integer
    value
  else
    "other"
  end
end
```

### result

```rbs
class Object < BasicObject
  def unknown_class_case: (untyped value) -> ("other" | Integer)
end
```

## class case includes a later unrelated caller

### update

```ruby
def later_class_case(value)
  case value
  when Integer
    :number
  else
    :other
  end
end

later_class_case(1)
later_class_case("text")
```

### result

```rbs
class Object < BasicObject
  def later_class_case: ((Integer | String) value) -> (:number | :other)
end
```

## class case keeps custom class equality possible

### update

```ruby
class CaseWhenMatcher
  def self.===(_value)
    true
  end
end

def custom_class_case(value)
  case value
  when CaseWhenMatcher
    :matched
  else
    value
  end
end

custom_class_case("text")
```

### result

```rbs
class CaseWhenMatcher
  def self.===: (untyped _value) -> true
end

class Object < BasicObject
  def custom_class_case: (String value) -> (String | :matched)
end
```

## class case keeps unknown superclass matches possible

### update

```ruby
def runtime_case_parent
  Class.new
end

class DynamicCaseChild < runtime_case_parent
end

def dynamic_parent_case(value)
  case value
  when DynamicCaseChild
    :matched
  else
    value
  end
end

dynamic_parent_case("text")
```

### result

```rbs
class Object < BasicObject
  def runtime_case_parent: -> singleton(Object)
  def dynamic_parent_case: (String value) -> (String | :matched)
end
```

## class case keeps anonymous Struct parent possible

### update

```ruby
class Person < Struct.new(:name)
end

class StructParentMatcher
  def match(value)
    case value
    when Struct
      :struct
    else
      :other
    end
  end
end

StructParentMatcher.new.match(Person.new("Ada"))
```

### result

```rbs
class Person
  def name: -> String
  def name=: (String name) -> String
  def initialize: (String name) -> void
  def self.members: -> Array[:name]
end

class StructParentMatcher
  def match: (Person value) -> (:other | :struct)
end
```

## class case keeps anonymous Data parent possible

### update

```ruby
class Point < Data.define(:x)
end

class DataParentMatcher
  def match(value)
    case value
    when Data
      :data
    else
      :other
    end
  end
end

DataParentMatcher.new.match(Point.new(x: 1))
```

### result

```rbs
class DataParentMatcher
  def match: (Point value) -> (:data | :other)
end

class Point
  def x: -> Integer
  def initialize: (x: Integer) -> void
  def self.members: -> Array[:x]
  def with: (?x: Integer) -> Point
end
```

## class case keeps subclass matches possible

### update

```ruby
class CaseWhenBase
end

class CaseWhenChild < CaseWhenBase
end

def subclass_class_case(value)
  case value
  when CaseWhenChild
    :child
  else
    :base
  end
end

subclass_class_case(CaseWhenBase.new)
subclass_class_case(CaseWhenChild.new)
```

### result

```rbs
class Object < BasicObject
  def subclass_class_case: ((CaseWhenBase | CaseWhenChild) value) -> (:base | :child)
end
```

## class case accounts for later equality definitions

### update

```ruby
def case_before_equality_override
  value = "text"
  case value
  when Integer
    :matched
  else
    :other
  end
end

class Integer
  def self.===(_value)
    true
  end
end

case_before_equality_override
```

### result

```rbs
class Integer < Numeric
  def self.===: (untyped _value) -> bool
end

class Object < BasicObject
  def case_before_equality_override: -> :matched | :other
end
```

## class case sees later equality definitions with prior call sites

### update

```ruby
def invoke_later_equality_case
  later_equality_case("text")
end

def later_equality_case(value)
  case value
  when Integer
    :matched
  else
    :other
  end
end

class Integer
  def self.===(_value)
    true
  end
end

invoke_later_equality_case
```

### result

```rbs
class Integer < Numeric
  def self.===: (untyped _value) -> bool
end

class Object < BasicObject
  def invoke_later_equality_case: -> :matched | :other
  def later_equality_case: (String value) -> (:matched | :other)
end
```

## class case preserves the local subject after a later Module equality override

```yaml
known_issue: true
```

### update

```ruby
def module_override_class_case
  value = "text"
  case value
  when Integer
    value
  else
    "other"
  end
end

module Module
  def ===(_value)
    true
  end
end

module_override_class_case
```

### result

```rbs
module Module
  def ===: (untyped other) -> bool
end

class Object < BasicObject
  def module_override_class_case: -> "other" | "text"
end
```

## class case replays a parameter after a later Module equality override

### update

```ruby
def module_override_unknown_case(value)
  case value
  when Integer
    value
  else
    "other"
  end
end

module Module
  def ===(_value)
    true
  end
end

module_override_unknown_case
```

### result

```rbs
module Module
  def ===: (untyped other) -> bool
end

class Object < BasicObject
  def module_override_unknown_case: (untyped value) -> (untyped | "other")
end
```
