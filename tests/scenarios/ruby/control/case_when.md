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

## An assigned local keeps its concrete domain

### update

```ruby
def case_local_assignment(value)
  value = "local"
  case value
  when String
    :local
  else
    :unreachable
  end
end

def case_open_alias(value)
  candidate = value
  case candidate
  when Integer
    :integer
  else
    :other
  end
end

case_local_assignment(:unused)
case_open_alias(1)
```

### result

```rbs
class Object < BasicObject
  def case_local_assignment: (Symbol value) -> :local
  def case_open_alias: (Integer value) -> :integer
end
```

## An optional default does not close the input domain

### update

```ruby
def case_optional_default_domain(value = "default")
  case value
  when String
    :text
  else
    :other
  end
end

case_optional_default_domain
case_optional_default_domain(1)
```

### result

```rbs
class Object < BasicObject
  def case_optional_default_domain: (?(Integer | String) value) -> (:other | :text)
end
```

## An omitted optional argument narrows to its default

### update

```ruby
def case_optional_default_only(value = "default")
  case value
  when String
    :text
  else
    :other
  end
end

case_optional_default_only
```

### result

```rbs
class Object < BasicObject
  def case_optional_default_only: (?String value) -> :text
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

## Explicit parameter contracts remain closed during case narrowing

### update

```ruby
class CaseWhenExplicitContract
  #: (String) -> Symbol
  def classify(value)
    case value
    when String
      :text
    else
      :other
    end
  end
end
```

### result

```rbs
class CaseWhenExplicitContract
  def classify: (String value) -> Symbol
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

## Case when keeps sibling branch types separate

### update

```ruby
def case_when_time_like(value)
  value
end

def case_when_scalar(value)
  value
end

def case_when_sibling_types(value)
  case value
  when Array
    :array
  when Time, String
    case_when_time_like(value)
  when Integer, TrueClass, FalseClass
    case_when_scalar(value)
  else
    :other
  end
end

case_when_sibling_types([])
case_when_sibling_types(Time.now)
case_when_sibling_types("time-like")
case_when_sibling_types(1)
case_when_sibling_types(true)
case_when_sibling_types(:other)
```

### result

```rbs
class Object < BasicObject
  def case_when_time_like: ((String | Time) value) -> (String | Time)
  def case_when_scalar: ((Integer | bool) value) -> (Integer | bool)
  def case_when_sibling_types: ((Integer | String | Symbol | bool | Time | [ ]) value) -> (Integer | String | bool | :array | :other | Time)
end
```

## Case when keeps values returned by an optional block

### update

```ruby
def case_when_block_time(value)
  value
end

def case_when_block_integer(value)
  value
end

def case_when_optional_block(value, &block)
  value = yield value if block
  case value
  when Time
    case_when_block_time(value)
  when Integer
    case_when_block_integer(value)
  else
    :other
  end
end

case_when_optional_block(Time.now)
case_when_optional_block(Time.now) { |time| time.to_i }
```

### result

```rbs
class Object < BasicObject
  def case_when_block_time: (Time value) -> Time
  def case_when_block_integer: (Integer value) -> Integer
  def case_when_optional_block: (Time value, ?untyped &block) -> (Integer | :other | Time)
end
```

## Case when keeps unresolved yielded values reachable

### update

```ruby
class CaseWhenItem
  def name
    "item"
  end
end

class CaseWhenUnknownBlockArg
  def search(query)
    local_search(query)
  end

  def local_search(query)
    case query
    when CaseWhenItem
      search_by_name(query)
    else
      :other
    end
  end

  def search_by_name(value)
    value
  end

end

CaseWhenUnknownBlockArg.new.search(T.unsafe("query"))
```

### result

```rbs
class CaseWhenItem
  def name: -> "item"
end

class CaseWhenUnknownBlockArg
  def search: (untyped query) -> (:other | CaseWhenItem)
  def local_search: (untyped query) -> (:other | CaseWhenItem)
  def search_by_name: (CaseWhenItem value) -> CaseWhenItem
end
```

## Case when Array preserves unknown element types

### update

```ruby
class CaseWhenArrayContents
  def first(value)
    case value
    when Array
      value[0]
    end
  end

  def hash_value(value)
    case value
    when Hash
      value[:key]
    end
  end
end

class CaseWhenArrayFields
  def initialize(value)
    case value
    when Array
      @field, = value
    when String
      @field = value
    end
  end

  attr_reader :field
end
```

### result

```rbs
class CaseWhenArrayContents
  def first: (untyped value) -> (nil | untyped)
  def hash_value: (untyped value) -> (nil | untyped)
end

class CaseWhenArrayFields
  def initialize: (untyped value) -> void
  def field: -> String | nil | untyped
end
```

## Preserve observed nil setter calls after class equality

### update

```ruby
class OpenCaseNilSetterTarget
  def self.===(_other) = _other.is_a?(self)
end

class OpenCaseNilSetter
  def value=(value)
    @value = value
  end

  def assign(value)
    case value
    when OpenCaseNilSetterTarget
      :other
    when NilClass
      self.value = nil
    end
  end
end

OpenCaseNilSetter.new.assign("known")
OpenCaseNilSetter.new.assign(nil)
```

### result

```rbs
class OpenCaseNilSetter
  def value=: (nil value) -> nil
  def assign: (String? value) -> :other?
end

class OpenCaseNilSetterTarget
  def self.===: (untyped _other) -> bool
end
```
