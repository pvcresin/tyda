# Ruby / Control / Flow Sensitive

## Narrow type with is_a?

### update

```ruby
class TypeChecker
  #: (untyped) -> String
  def check(x)
    if x.is_a?(Integer)
      x.to_s
    else
      "not integer"
    end
  end
end
```

### result

```rbs
class TypeChecker
  def check: (untyped x) -> String
end
```

## Keep different branch types after is_a?

### update

```ruby
class IsAChecker
  #: (Integer | String) -> (Symbol | 1)
  def convert(x)
    if x.is_a?(String)
      x.to_sym
    else
      1
    end
  end
end
```

### result

```rbs
class IsAChecker
  def convert: ((Integer | String) x) -> (Symbol | 1)
end
```

## Correlated locals narrow only along compatible types

### update

```ruby
class CorrelatedFlowValues
  #: () -> Integer
  def integer_subject = 1

  #: () -> String
  def string_subject = "text"
end

def paired_value_after_string_check(flag)
  if flag
    subject = CorrelatedFlowValues.new.integer_subject
    paired = :integer
  else
    subject = CorrelatedFlowValues.new.string_subject
    paired = :string
  end

  return :unmatched unless subject.is_a?(String)
  paired
end

def paired_value_after_subject_reassignment(flag)
  if flag
    subject = CorrelatedFlowValues.new.integer_subject
    paired = :integer
  else
    subject = CorrelatedFlowValues.new.string_subject
    paired = :string
  end

  subject = "replacement"
  return :unmatched unless subject.is_a?(String)
  paired
end

def paired_value_after_branch_reassignment(flag, replacement)
  if flag
    subject = CorrelatedFlowValues.new.integer_subject
    paired = :integer
  else
    subject = CorrelatedFlowValues.new.string_subject
    paired = :string
  end

  if replacement
    subject = "replacement"
  else
    subject = "another replacement"
  end

  return :unmatched unless subject.is_a?(String)
  paired
end

def paired_value_after_many_branch_locals(flag)
  if flag
    subject = 1
    marker_a = :left_a
    marker_b = :left_b
    marker_c = :left_c
    marker_d = :left_d
    marker_e = :left_e
    marker_f = :left_f
    marker_g = :left_g
    marker_h = :left_h
    paired = :integer
  else
    subject = "text"
    marker_a = :right_a
    marker_b = :right_b
    marker_c = :right_c
    marker_d = :right_d
    marker_e = :right_e
    marker_f = :right_f
    marker_g = :right_g
    marker_h = :right_h
    paired = :string
  end

  return :unmatched unless subject.is_a?(String)
  paired
end

def paired_value_after_same_class_check(flag)
  if flag
    subject = "first"
    paired = :first
  else
    subject = "second"
    paired = :second
  end

  return :unmatched unless subject.is_a?(String)
  paired
end
```

### result

```rbs
class CorrelatedFlowValues
  def integer_subject: -> Integer
  def string_subject: -> String
end

class Object < BasicObject
  def paired_value_after_string_check: (untyped flag) -> (:string | :unmatched)
  def paired_value_after_subject_reassignment: (untyped flag) -> (:integer | :string | :unmatched)
  def paired_value_after_branch_reassignment: (untyped flag, untyped replacement) -> (:integer | :string | :unmatched)
  def paired_value_after_many_branch_locals: (untyped flag) -> (:string | :unmatched)
  def paired_value_after_same_class_check: (untyped flag) -> (:first | :second | :unmatched)
end
```

## Correlated locals keep paths up to the world limit

Each independent branch doubles the paths: five branches make 32 worlds and keep the
correlation, and six make 64 and join the values. The joined result stays sound, so this pins
the boundary rather than a goal. Raising `MAX_CORRELATED_WORLDS` should update this case.

### update

```ruby
class CorrelatedWorldLimit
  def branches_5(c)
    if c[0]
      a0 = 1
      b0 = 'x'
    else
      a0 = 'y'
      b0 = 2
    end
    if c[1]
      a1 = 1
      b1 = 'x'
    else
      a1 = 'y'
      b1 = 2
    end
    if c[2]
      a2 = 1
      b2 = 'x'
    else
      a2 = 'y'
      b2 = 2
    end
    if c[3]
      a3 = 1
      b3 = 'x'
    else
      a3 = 'y'
      b3 = 2
    end
    if c[4]
      a4 = 1
      b4 = 'x'
    else
      a4 = 'y'
      b4 = 2
    end
    a0.is_a?(Integer) ? b0 : :other
  end

  def branches_6(c)
    if c[0]
      a0 = 1
      b0 = 'x'
    else
      a0 = 'y'
      b0 = 2
    end
    if c[1]
      a1 = 1
      b1 = 'x'
    else
      a1 = 'y'
      b1 = 2
    end
    if c[2]
      a2 = 1
      b2 = 'x'
    else
      a2 = 'y'
      b2 = 2
    end
    if c[3]
      a3 = 1
      b3 = 'x'
    else
      a3 = 'y'
      b3 = 2
    end
    if c[4]
      a4 = 1
      b4 = 'x'
    else
      a4 = 'y'
      b4 = 2
    end
    if c[5]
      a5 = 1
      b5 = 'x'
    else
      a5 = 'y'
      b5 = 2
    end
    a0.is_a?(Integer) ? b0 : :other
  end
end
```

### result

```rbs
class CorrelatedWorldLimit
  def branches_5: (untyped c) -> ("x" | :other)
  def branches_6: (untyped c) -> (2 | "x" | :other)
end
```

## Narrow type with class case equality

### update

```ruby
class ClassCaseEqualityChecker
  #: (Integer | String) -> (String | 1)
  def convert(x)
    if String === x
      x.upcase
    else
      1
    end
  end
end
```

### result

```rbs
class ClassCaseEqualityChecker
  def convert: ((Integer | String) x) -> (String | 1)
end
```

## Resolve unqualified class constants in is_a? narrowing

### update

```ruby
module ScopedClassNarrowing
  class Base; end
  class LocalRead < Base
    def value = :value
  end
  class Other < Base; end

  class Checker
    def value(flag)
      node = flag ? LocalRead.new : Other.new
      return unless node.is_a?(LocalRead)
      node.value
    end
  end
end
```

### result

```rbs
class ScopedClassNarrowing::Checker
  def value: (untyped flag) -> :value?
end

class ScopedClassNarrowing::LocalRead < Base
  def value: -> :value
end
```

## Resolve type-test targets across project files

### update

`app/models/narrowing.rb`

```ruby
module TypeProf::Core
  class Narrowing
    class Constraint
      def marker = :ok
    end
  end
end
```

`app/models/local_env.rb`

```ruby
module TypeProf::Core
  class LocalEnv
    def initialize
      @ivar_narrowings = {}
    end

    def push_ivar_narrowing(name, narrowing)
      raise unless narrowing.is_a?(Narrowing::Constraint)
      (@ivar_narrowings[name] ||= []) << narrowing
    end
  end
end
```

```ruby
def read_ivar_narrowing
  TypeProf::Core::LocalEnv.new.push_ivar_narrowing(
    :value,
    TypeProf::Core::Narrowing::Constraint.new,
  )
end
```

### result

```rbs
class Object < BasicObject
  def read_ivar_narrowing: -> [Narrowing::Constraint]
end
```

## Narrow a parent type to a checked subclass

### update

```ruby
class ParentTypePredicate
  class Base; end
  class Child < Base
    def child_only = :child
  end

  #: (Base) -> :child?
  def child_value(value)
    return unless value.is_a?(Child)
    value.child_only
  end

  #: (Base) -> :child?
  def class_case_child_value(value)
    return unless Child === value
    value.child_only
  end
end
```

### result

```rbs
class ParentTypePredicate
  def child_value: (Base value) -> :child?
  def class_case_child_value: (Base value) -> :child?
end

class ParentTypePredicate::Child < Base
  def child_only: -> :child
end
```

## Ignore generic arguments when excluding unrelated classes

### update

```ruby
class GenericTypeGuard
end

def value_after_generic_type_guard(value)
  result = {}
  if value.is_a?(GenericTypeGuard)
    result[value] = true
  end
  result
end

def hash_after_generic_type_guard(value)
  result = {}
  if value.is_a?(GenericTypeGuard)
    result[value] = true
  end
  result
end

value_after_generic_type_guard({})
value_after_generic_type_guard(GenericTypeGuard.new)
hash_after_generic_type_guard({})
```

### result

```rbs
class Object < BasicObject
  def value_after_generic_type_guard: ((Hash[untyped, untyped] | GenericTypeGuard) value) -> (Hash[untyped, untyped] | Hash[GenericTypeGuard, true])
  def hash_after_generic_type_guard: (Hash[untyped, untyped] value) -> Hash[untyped, untyped]
end
```

## Keep a subclass check open when its parent is unknown

### update

```ruby
class UnknownTargetBase
  def base_value = :base
end

parent_class = UnknownTargetBase
UnknownTargetChild = Class.new(parent_class)

def unknown_target_value(value)
  return unless value.is_a?(UnknownTargetChild)
  value.base_value
end

def inferred_unknown_target_value
  unknown_target_value(UnknownTargetBase.new)
end
```

### result

```rbs
UnknownTargetChild: singleton(Object)

class Object < BasicObject
  def unknown_target_value: (UnknownTargetBase value) -> (nil | untyped)
  def inferred_unknown_target_value: -> nil | untyped
end

class UnknownTargetBase
  def base_value: -> :base
end
```

## Keep different branch types after instance_of?

### update

```ruby
class InstanceOfChecker
  #: (Integer | String) -> (Symbol | 1)
  def convert(x)
    if x.instance_of?(String)
      x.to_sym
    else
      1
    end
  end
end
```

### result

```rbs
class InstanceOfChecker
  def convert: ((Integer | String) x) -> (Symbol | 1)
end
```

## Narrow type with nil?

### update

```ruby
class NilChecker
  #: (String?) -> String
  def safe_upcase(x)
    if x.nil?
      "default"
    else
      x.upcase
    end
  end
end
```

### result

```rbs
class NilChecker
  def safe_upcase: (String? x) -> String
end
```

## Keep different branch types after nil?

### update

```ruby
class NilBranchKinds
  #: (String?) -> (1 | Symbol)
  def convert(x)
    if x.nil?
      1
    else
      x.to_sym
    end
  end
end
```

### result

```rbs
class NilBranchKinds
  def convert: (String? x) -> (Symbol | 1)
end
```

## Truthiness removes nil and false

### update

```ruby
class TruthyChecker
  #: (String?) -> String
  def check_truthy(x)
    if x
      x.upcase
    else
      "nil"
    end
  end
end
```

### result

```rbs
class TruthyChecker
  def check_truthy: (String? x) -> String
end
```

## Truthiness also removes false

### update

```ruby
class TruthyWithFalse
  #: ((String | bool)?) -> String
  def normalize(x)
    if x
      x.upcase
    else
      "missing"
    end
  end
end
```

### result

```rbs
class TruthyWithFalse
  def normalize: ((String | bool)? x) -> String
end
```

## Keep different branch types after truthiness

### update

```ruby
class TruthyKinds
  #: ((String | bool)?) -> (Symbol | 1)
  def convert(x)
    if x
      x.to_sym
    else
      1
    end
  end
end
```

### result

```rbs
class TruthyKinds
  def convert: ((String | bool)? x) -> (Symbol | 1)
end
```

## Branch nil and false after truthiness

### update

```ruby
class TruthySplitFalsy
  #: ((String | bool)?) -> (Symbol | 1 | 2)
  def convert(x)
    if x
      x.to_sym
    elsif x.nil?
      1
    else
      2
    end
  end
end
```

### result

```rbs
class TruthySplitFalsy
  def convert: ((String | bool)? x) -> (Symbol | 1 | 2)
end
```

## Bool param splits into true and false branches

### update

```ruby
class BoolTruthinessSplit
  def split(x)
    value = "#{x}".empty?
    if value
      { truthy: value }
    else
      { falsy: value }
    end
  end
end
```

### result

```rbs
class BoolTruthinessSplit
  def split: (untyped x) -> ({ falsy: false } | { truthy: true })
end
```

## Narrow type with unless x.nil?

### update

```ruby
class UnlessNil
  #: (Integer?) -> String
  def format(x)
    unless x.nil?
      x.to_s
    else
      "none"
    end
  end
end
```

### result

```rbs
class UnlessNil
  def format: (Integer? x) -> String
end
```

## Keep both branch types with unless x.nil?

### update

```ruby
class UnlessNilKinds
  #: (String?) -> (1 | Symbol)
  def convert(x)
    unless x.nil?
      x.to_sym
    else
      1
    end
  end
end
```

### result

```rbs
class UnlessNilKinds
  def convert: (String? x) -> (Symbol | 1)
end
```

## Narrow type with kind_of?

### update

```ruby
class KindChecker
  #: (untyped) -> String
  def check(x)
    if x.kind_of?(String)
      x.upcase
    else
      "not string"
    end
  end
end
```

### result

```rbs
class KindChecker
  def check: (untyped x) -> String
end
```

## Keep different branch types after kind_of?

### update

```ruby
class KindBranchKinds
  #: (Integer | String) -> (Symbol | 1)
  def convert(x)
    if x.kind_of?(String)
      x.to_sym
    else
      1
    end
  end
end
```

### result

```rbs
class KindBranchKinds
  def convert: ((Integer | String) x) -> (Symbol | 1)
end
```

## Narrow with elsif using is_a? and nil?

### update

```ruby
class CombinedChecker
  #: ((Integer | String)?) -> (Symbol | 1 | 2)
  def convert(x)
    if x.is_a?(String)
      x.to_sym
    elsif x.nil?
      1
    else
      2
    end
  end
end
```

### result

```rbs
class CombinedChecker
  def convert: ((Integer | String)? x) -> (Symbol | 1 | 2)
end
```

## Postfix if narrows after return if x.nil?

### update

```ruby
class GuardNil
  #: (String?) -> String
  def process(x)
    return "" if x.nil?
    x.upcase
  end
end
```

### result

```rbs
class GuardNil
  def process: (String? x) -> String
end
```

## Postfix unless narrows after return unless x

### update

```ruby
class GuardTruthy
  #: (String?) -> String
  def process(x)
    return "" unless x
    x.upcase
  end
end
```

### result

```rbs
class GuardTruthy
  def process: (String? x) -> String
end
```

## Narrow a deferred optional method return after a guard

### update

```ruby
class DeferredPayloadSource
  def data(flag)
    return unless flag
    { title: "title", dynamic: Object.new }
  end
end

class DeferredPayloadGuard
  def call(flag)
    data = DeferredPayloadSource.new.data(flag)
    return "" unless data
    data[:title].upcase
  end
end
```

### result

```rbs
class DeferredPayloadGuard
  def call: (untyped flag) -> String
end

class DeferredPayloadSource
  def data: (untyped flag) -> { title: "title", dynamic: Object }?
end
```

## Postfix if narrows after raise if x.nil?

### update

```ruby
class GuardRaise
  #: (Integer?) -> Integer
  def calculate(x)
    raise "nil!" if x.nil?
    x + 1
  end
end
```

### result

```rbs
class GuardRaise
  def calculate: (Integer? x) -> Integer
end
```

## Postfix block if narrows when then always exits

### update

```ruby
class BlockGuard
  #: (String?) -> String
  def process(x)
    if x.nil?
      return ""
    end
    x.upcase
  end
end
```

### result

```rbs
class BlockGuard
  def process: (String? x) -> String
end
```

## Postfix unless narrows after return unless x.present?

### update

```ruby
class GuardPresent
  #: (String?) -> String
  def process(x)
    return "" unless x.present?
    x.upcase
  end
end
```

### result

```rbs
class GuardPresent
  def process: (String? x) -> String
end
```

## Module is_a? branch narrows receiver

### update

```ruby
module M
  def m_method = :MMM
end

class C
  include M
  def c_method = :CCC
end

class D
  def d_method = :DDD
end

def foo(x)
  if x.is_a?(M)
    [x.m_method, x.c_method]
  else
    x.d_method
  end
end

foo(C.new)
foo(D.new)
```

### result

```rbs
class C
  include M

  def c_method: -> :CCC
end

class D
  def d_method: -> :DDD
end

module M
  def m_method: -> :MMM
end

class Object < BasicObject
  def foo: ((C | D) x) -> (:DDD | [:MMM, :CCC])
end
```

## respond_to? branch narrows project methods

### update

```ruby
module Named
  def name = "name"
end

class Entry
  include Named
end

class Total
  def count = 1
end

def read_with_method_guard(x)
  if x.respond_to?(:name)
    x.name
  else
    x.count
  end
end

read_with_method_guard(Entry.new)
read_with_method_guard(Total.new)
```

### result

```rbs
class Entry
  include Named
end

module Named
  def name: -> "name"
end

class Object < BasicObject
  def read_with_method_guard: ((Entry | Total) x) -> (1 | "name")
end

class Total
  def count: -> 1
end
```

## respond_to? guard clause narrows receiver

### update

```ruby
METHOD_NAME = "amount"

class Price
  def amount = 1
end

class Label
  def text = "text"
end

def read_with_return_guard(x)
  return x.text unless x.respond_to?(METHOD_NAME)
  x.amount
end

read_with_return_guard(Price.new)
read_with_return_guard(Label.new)
```

### result

```rbs
METHOD_NAME: "amount"

class Label
  def text: -> "text"
end

class Object < BasicObject
  def read_with_return_guard: ((Label | Price) x) -> (1 | "text")
end

class Price
  def amount: -> 1
end
```

## AND condition propagates narrowing on both sides

### update

```ruby
class AndChecker
  #: (Integer | String | nil, Integer | String | nil) -> String
  def join(a, b)
    if a.is_a?(String) && b.is_a?(String)
      a + b
    else
      ""
    end
  end
end
```

### result

```rbs
class AndChecker
  def join: ((Integer | String)? a, (Integer | String)? b) -> String
end
```

## OR else branch propagates negative narrowing on both sides

### update

```ruby
class OrChecker
  #: (Integer?, String?) -> String
  def fallback(a, b)
    if a.nil? || b.nil?
      "missing"
    else
      a.to_s + b
    end
  end
end
```

### result

```rbs
class OrChecker
  def fallback: (Integer? a, String? b) -> String
end
```

## case when narrows with literal guards

### update

```ruby
class CaseChecker
  #: (Integer | String | Symbol) -> String
  def classify(v)
    case v
    when Integer
      v.to_s
    when "hello"
      "hi"
    when :world
      "earth"
    else
      "unknown"
    end
  end
end
```

### result

```rbs
class CaseChecker
  def classify: ((Integer | String | Symbol) v) -> String
end
```

## Narrow !x.nil? then branch to non-nil

### update

```ruby
class NegatedNilChecker
  #: (String?) -> Symbol
  def convert(x)
    if !x.nil?
      x.to_sym
    else
      :missing
    end
  end
end
```

### result

```rbs
class NegatedNilChecker
  def convert: (String? x) -> Symbol
end
```

## Narrow unless !x.nil? else branch to non-nil

### update

```ruby
class UnlessNegatedNilChecker
  #: (Integer?) -> String
  def format(x)
    unless !x.nil?
      "none"
    else
      x.to_s
    end
  end
end
```

### result

```rbs
class UnlessNegatedNilChecker
  def format: (Integer? x) -> String
end
```

## Narrow != nil then branch to non-nil

### update

```ruby
class NotEqualNilChecker
  #: (Integer?) -> Integer
  def plus_one(x)
    if x != nil
      x + 1
    else
      0
    end
  end
end
```

### result

```rbs
class NotEqualNilChecker
  def plus_one: (Integer? x) -> Integer
end
```

## Narrow !(x == nil) to non-nil

### update

```ruby
class NegatedEqualNilChecker
  #: (String?) -> String
  def normalize(x)
    if !(x == nil)
      x.upcase
    else
      "missing"
    end
  end
end
```

### result

```rbs
class NegatedEqualNilChecker
  def normalize: (String? x) -> String
end
```

## Narrow local with reversed nil comparison

### update

```ruby
class ReversedNotEqualNilChecker
  #: (String?) -> Symbol
  def convert(x)
    if nil != x
      x.to_sym
    else
      :missing
    end
  end
end
```

### result

```rbs
class ReversedNotEqualNilChecker
  def convert: (String? x) -> Symbol
end
```

## Narrow != true then branch to remaining types

### update

```ruby
class NotEqualTrueChecker
  #: (Integer | true) -> Integer
  def plus_one(x)
    if x != true
      x + 1
    else
      0
    end
  end
end
```

### result

```rbs
class NotEqualTrueChecker
  def plus_one: ((Integer | true) x) -> Integer
end
```

## Narrow !(x == false) then branch to remaining types

### update

```ruby
class NegatedEqualFalseChecker
  #: (Integer | false) -> Integer
  def plus_one(x)
    if !(x == false)
      x + 1
    else
      0
    end
  end
end
```

### result

```rbs
class NegatedEqualFalseChecker
  def plus_one: ((Integer | false) x) -> Integer
end
```

## AND right side uses truthy narrowing from left side

### update

```ruby
class AndRhsNarrowing
  #: (String?) -> String?
  def upcase_if_present(x)
    x && x.upcase
  end
end
```

### result

```rbs
class AndRhsNarrowing
  def upcase_if_present: (String? x) -> String?
end
```

## OR right side after !x narrows x to truthy

### update

```ruby
class OrRhsNarrowing
  #: (String?) -> (String | true)
  def present_or_upcase(x)
    !x || x.upcase
  end
end
```

### result

```rbs
class OrRhsNarrowing
  def present_or_upcase: (String? x) -> (String | true)
end
```

## bool local splits on == true

### update

```ruby
class BoolEqualitySplit
  def split(x)
    flag = "#{x}".empty?
    if flag == true
      { yes: flag }
    else
      { no: flag }
    end
  end
end
```

### result

```rbs
class BoolEqualitySplit
  def split: (untyped x) -> ({ no: false } | { yes: true })
end
```

## Hash#key? narrows record union branches

### update

```ruby
class RecordKeyNarrowing
  #: ({ a: Integer } | { b: bool }) -> (Integer | bool)
  def value(x)
    if x.key?(:a)
      x[:a]
    else
      x[:b]
    end
  end
end
```

### result

```rbs
class RecordKeyNarrowing
  def value: (({ a: Integer } | { b: bool }) x) -> (Integer | bool)
end
```

## Hash#has_key? narrows record union with string key

### update

```ruby
class RecordStringKeyNarrowing
  #: ({ "name" => String } | { active: bool }) -> (String | bool)
  def value(x)
    if x.has_key?("name")
      x["name"]
    else
      x[:active]
    end
  end
end
```

### result

```rbs
class RecordStringKeyNarrowing
  def value: (({ active: bool } | { "name" => String }) x) -> (String | bool)
end
```

## Negated Hash#member? reverses record union narrowing

### update

```ruby
class RecordKeyNegatedNarrowing
  #: ({ a: Integer } | { b: bool }) -> (Integer | bool)
  def value(x)
    if !x.member?(:a)
      x[:b]
    else
      x[:a]
    end
  end
end
```

### result

```rbs
class RecordKeyNegatedNarrowing
  def value: (({ a: Integer } | { b: bool }) x) -> (Integer | bool)
end
```

## Hash#include? short-circuit right side narrows record union

### update

```ruby
class RecordKeyShortCircuitNarrowing
  #: ({ a: Integer } | { b: bool }) -> (Integer | false)
  def value(x)
    x.include?(:a) && x[:a]
  end
end
```

### result

```rbs
class RecordKeyShortCircuitNarrowing
  def value: (({ a: Integer } | { b: bool }) x) -> (Integer | false)
end
```

## Record discriminant narrows union

### update

```ruby
class RecordDiscriminantNarrowing
  #: ({ kind: :text, value: String } | { kind: :count, value: Integer }) -> (Integer | Symbol)
  def value(x)
    if x[:kind] == :text
      x[:value].to_sym
    else
      x[:value] + 1
    end
  end
end
```

### result

```rbs
class RecordDiscriminantNarrowing
  def value: (({ kind: :count, value: Integer } | { kind: :text, value: String }) x) -> (Integer | Symbol)
end
```

## Assignment in AND condition narrows the bound local

### update

```ruby
def foo(z)
  if (y = z) && y.length > 0
    y.to_sym
  end
end
foo("hello")
foo(nil)
```

### result

```rbs
class Object < BasicObject
  def foo: (String? z) -> Symbol?
end
```

## Regexp named capture binds local in the matched branch

### update

```ruby
class C
  def check
    if /(?<a>foo)/ =~ "foo"
      a
    else
      1
    end
  end

  def check_nonmatch
    if /(?<a>foo)/ =~ "bar"
      a
    else
      :not_matched
    end
  end

  def check_dynamic(s)
    if /(?<a>foo)/ =~ s
      a
    else
      :not_matched
    end
  end

  def check_options
    if /(?<a>foo)/i =~ "FOO"
      a
    else
      1
    end
  end

  def check_metacharacter
    if /(?<a>f.o)/ =~ "foo"
      a
    else
      1
    end
  end

  def check_multiple_captures
    if /(?<a>foo)(?<b>bar)/ =~ "foobar"
      a
    else
      1
    end
  end

  def after_match(s)
    /(?<b>bar)/ =~ s
    b
  end
end
```

### result

```rbs
class C
  def check: -> String
  def check_nonmatch: -> :not_matched
  def check_dynamic: (untyped s) -> (String | :not_matched)
  def check_options: -> String | 1
  def check_metacharacter: -> String | 1
  def check_multiple_captures: -> String | 1
  def after_match: (untyped s) -> String?
end
```

## Literal predicate return narrows union member behind a guard

### update

```ruby
class Circle
  #: -> true
  def round? = true

  def size = 1
end

class Square
  #: -> false
  def round? = false

  def size = 2.0
end

class Picker
  def pick(flag)
    shape = flag ? Circle.new : Square.new
    return 0.0 if shape.round?
    shape.size
  end
end
```

### result

```rbs
class Circle
  def round?: -> true
  def size: -> 1
end

class Picker
  def pick: (untyped flag) -> (0.0 | 2.0)
end

class Square
  def round?: -> false
  def size: -> 2.0
end
```

## Narrow ivar with blank? guard removes nil

### update

```ruby
class Corporation
end

class IvarGuard
  def initialize(flag)
    @corporation = flag ? Corporation.new : nil
  end

  def fetch
    return Corporation.new if @corporation.blank?
    @corporation
  end
end
```

### result

```rbs
class IvarGuard
  def initialize: (untyped flag) -> void
  def fetch: -> Corporation
end
```

## Narrow self method reader with blank? guard removes nil

### update

```ruby
class Corporation
end

class ReaderGuard
  #: -> Corporation?
  def corporation
    @corporation
  end

  def fetch
    return Corporation.new if corporation.blank?
    corporation
  end
end
```

### result

```rbs
class ReaderGuard
  def corporation: -> Corporation?
  def fetch: -> Corporation
end
```

## Chained attr_reader guards keep earlier narrowing

### update

```ruby
class Corporation
end

class Owner
end

class ChainedReaderGuard
  #: Corporation?
  attr_reader :corporation

  #: Owner?
  attr_reader :owner

  def fetch
    return Corporation.new if corporation.blank?
    return Owner.new if owner.blank?
    [corporation, owner]
  end
end
```

### result

```rbs
class ChainedReaderGuard
  def corporation: -> Corporation?
  def owner: -> Owner?
  def fetch: -> Corporation | Owner | [Corporation, Owner]
end
```

## Chained AND narrows a later helper call

### update

```ruby
def accept_str(x) = nil
def accept_any(x) = nil

def foo(x)
  accept_any(x) && x.is_a?(String) && accept_str(x)
end

foo(1)
foo("")
```

### result

```rbs
class Object < BasicObject
  def accept_str: (String x) -> nil
  def accept_any: ((Integer | String) x) -> nil
  def foo: ((Integer | String) x) -> false | nil
end
```

## AND of two is_a? narrows helper args

### update

```ruby
def check(x, y) = nil

def foo(x, y)
  x.is_a?(String) && y.is_a?(Integer) && check(x, y)
end

foo(1, 1)
foo(1, "")
foo("", 1)
foo("", "")
```

### result

```rbs
class Object < BasicObject
  def check: (String x, Integer y) -> nil
  def foo: ((Integer | String) x, (Integer | String) y) -> false | nil
end
```

## Truthy AND passes a non-nil value to the helper

### update

```ruby
def accept_str(x) = nil

def foo(x)
  x && accept_str(x)
end

foo(nil)
foo("")
```

### result

```rbs
class Object < BasicObject
  def accept_str: (String x) -> nil
  def foo: (String? x) -> nil
end
```

## Negated is_a? narrows the helper

### update

```ruby
def check(x) = nil

def foo(x)
  !x.is_a?(String) && check(x)
end

foo(1)
foo("")
```

### result

```rbs
class Object < BasicObject
  def check: (Integer x) -> nil
  def foo: ((Integer | String) x) -> false | nil
end
```

## Negated OR is_a? narrows both helpers

### update

```ruby
def check(x, y) = nil

def foo(x, y)
  !(x.is_a?(String) || y.is_a?(Integer)) && check(x, y)
end

foo(1, 1)
foo(1, "")
foo("", 1)
foo("", "")
```

### result

```rbs
class Object < BasicObject
  def check: (Integer x, String y) -> nil
  def foo: ((Integer | String) x, (Integer | String) y) -> false | nil
end
```

## Mixed AND OR precedence narrows branches

### update

```ruby
def accept_int_or_str(x) = nil
def accept_float_or_str(x) = nil
def accept_str(x) = nil
def accept_any(x) = nil

def foo(x)
  if accept_any(x) && x.is_a?(String) && accept_str(x) || x.is_a?(Float)
    accept_float_or_str(x)
  else
    accept_int_or_str(x)
  end
end

foo(1)
foo(1.0)
foo("")
```

### result

```rbs
class Object < BasicObject
  def accept_int_or_str: ((Integer | String) x) -> nil
  def accept_float_or_str: ((Float | String) x) -> nil
  def accept_str: (String x) -> nil
  def accept_any: ((Integer | Float | String) x) -> nil
  def foo: ((Integer | Float | String) x) -> nil
end
```

## respond_to? distinguishes known missing and dynamic methods

### update

```ruby
class DynamicReceiver
  def respond_to_missing?(name, include_private = false)
    name == :virtual
  end

  def method_missing(name, *args)
    "dynamic"
  end
end

def read_known_missing(x)
  if x.respond_to?(:full_messages)
    x.full_messages
  else
    :unavailable
  end
end

def read_dynamic(x)
  if x.respond_to?(:virtual)
    x.virtual
  else
    :unavailable
  end
end

read_known_missing("error")
read_dynamic(DynamicReceiver.new)
```

### result

```rbs
class DynamicReceiver
  def respond_to_missing?: (untyped name, ?bool include_private) -> untyped
  def method_missing: (untyped name, *untyped args) -> "dynamic"
end

class Object < BasicObject
  def read_known_missing: (String x) -> :unavailable
  def read_dynamic: (DynamicReceiver x) -> (untyped | :unavailable)
end
```

## A pure reader stays narrowed through short-circuit conditions

### update

```ruby
class OptionalReader
  attr_reader :value

  def initialize(flag)
    @value = flag ? "value" : nil
  end

  def read = value && value.length
end

def read_optional = OptionalReader.new(true).read
```

### result

```rbs
class Object < BasicObject
  def read_optional: -> 5?
end

class OptionalReader
  def value: -> "value"?
  def initialize: (bool flag) -> void
  def read: -> 5?
end
```

## A memoized reader stays narrowed after a nil guard

### update

```ruby
module Gitlab; end

class MemoizedUser
  def id = 1
end

class MemoizedReader
  def initialize(enabled)
    @enabled = enabled
  end

  def current_user = @enabled ? MemoizedUser.new : nil
  strong_memoize_attr :current_user

  def read
    return nil unless current_user

    current_user.id
  end
end

def read_memoized_user(enabled) = MemoizedReader.new(enabled).read
```

### result

```rbs
class MemoizedReader
  def initialize: (untyped enabled) -> void
  def current_user: -> MemoizedUser?
  def read: -> 1?
end

class MemoizedUser
  def id: -> 1
end

class Object < BasicObject
  def read_memoized_user: (untyped enabled) -> 1?
end
```

## A memoized reader stays narrowed through case branches

### update

```ruby
module Gitlab; end

class ProjectNode
  def project_value = 1
end

class GroupNode
  def group_value = "group"
end

class MemoizedTargetReader
  def initialize(project)
    @project = project
  end

  def target = @project ? ProjectNode.new : GroupNode.new
  strong_memoize_attr :target

  def read
    case target
    when ProjectNode
      target.project_value
    when GroupNode
      target.group_value
    end
  end
end

def read_memoized_target(project) = MemoizedTargetReader.new(project).read
```

### result

```rbs
class GroupNode
  def group_value: -> "group"
end

class MemoizedTargetReader
  def initialize: (untyped project) -> void
  def target: -> GroupNode | ProjectNode
  def read: -> (1 | "group")?
end

class Object < BasicObject
  def read_memoized_target: (untyped project) -> (1 | "group")?
end

class ProjectNode
  def project_value: -> 1
end
```

## A later definition overrides memoized reader stability

### update

```ruby
module Gitlab; end

class DynamicMemoizedReader
  def initialize(enabled)
    @enabled = false
  end

  strong_memoize_attr :value
  def value
    @enabled = !@enabled
    @enabled ? "text" : nil
  end

  def read
    return nil unless value

    value.upcase
  end
end

def read_dynamic_memoized_value(enabled) = DynamicMemoizedReader.new(enabled).read
```

### result

```rbs
class DynamicMemoizedReader
  def initialize: (untyped enabled) -> void
  def value: -> "text"?
  def read: -> String?
end

class Object < BasicObject
  def read_dynamic_memoized_value: (untyped enabled) -> String?
end
```

## Class equality excludes a known disjoint type

### update

```ruby
class ClassEqualityTarget; end

class ClassEqualityGuard
  #: (String) -> String
  def read(value)
    return "other" unless ClassEqualityTarget === value

    value.missing
  end
end
```

### result

```rbs
class ClassEqualityGuard
  def read: (String value) -> String
end
```

## Standard type predicates exclude a known disjoint type

### update

```ruby
class TypePredicateTarget; end
class TypePredicateBase; end
class TypePredicateChild < TypePredicateBase; end

class TypePredicateGuard
  #: (String) -> String
  def read_with_is_a(value)
    return "other" unless value.is_a?(TypePredicateTarget)

    value.missing
  end

  #: (String) -> String
  def read_with_instance_of(value)
    return "other" unless value.instance_of?(TypePredicateTarget)

    value.missing
  end

  #: (TypePredicateChild) -> nil
  def read_with_instance_of_parent(value)
    return nil unless value.instance_of?(TypePredicateBase)

    value.missing
  end
end
```

### result

```rbs
class TypePredicateGuard
  def read_with_is_a: (String value) -> String
  def read_with_instance_of: (String value) -> String
  def read_with_instance_of_parent: (TypePredicateChild value) -> nil
end
```

## A closed hash key check removes an impossible branch

### update

```ruby
def guarded_missing_hash_key
  settings = { available: :handler }.freeze
  if settings.include?(:missing)
    :unreachable
  else
    :present
  end
end
```

### result

```rbs
class Object < BasicObject
  def guarded_missing_hash_key: -> :present
end
```

## A return guard narrows an optional parameter

### update

```ruby
class GuardedUser
  def name = "user"
end

def guarded_user_name(user)
  return nil unless user

  user.name.upcase
end

def read_guarded_user_name = guarded_user_name(GuardedUser.new)
def read_guarded_user_name_without_user = guarded_user_name(nil)
```

### result

```rbs
class GuardedUser
  def name: -> "user"
end

class Object < BasicObject
  def guarded_user_name: (GuardedUser? user) -> String?
  def read_guarded_user_name: -> String?
  def read_guarded_user_name_without_user: -> String?
end
```

## Unknown class ancestry keeps type-test branches open

### update

```ruby
class AncestryBase; end

parent_class = AncestryBase
UnknownAncestryChild = Class.new(parent_class) do
end

#: (UnknownAncestryChild) -> untyped
def read_unknown_is_a(value)
  return :other unless value.is_a?(AncestryBase)

  value.missing
end

#: (AncestryBase) -> untyped
def read_unknown_instance_of(value)
  return :other unless value.instance_of?(UnknownAncestryChild)

  value.missing
end
```

### result

```rbs
class Object < BasicObject
  def read_unknown_is_a: (UnknownAncestryChild value) -> untyped
  def read_unknown_instance_of: (AncestryBase value) -> untyped
end
```

## Source overrides remain authoritative for closed hash predicates

### update

```ruby
class Hash
  #: (Symbol) -> bool
  def key?(_key) = true
end

def overridden_hash_key_predicate
  values = { present: "value" }.freeze
  values.key?(:missing) ? :present : :absent
end
```

### result

```rbs
class Hash[K, V]
  def key?: (Symbol _key) -> bool
end

class Object < BasicObject
  def overridden_hash_key_predicate: -> :absent | :present
end
```

## Active refinements keep type-test branches open

### update

```ruby
class RefinedTarget; end

module AlwaysMatchString
  refine String do
    def is_a?(_target) = true
  end
end

module AlwaysMatchClass
  refine Class do
    def ===(_value) = true
  end
end

using AlwaysMatchString
using AlwaysMatchClass

#: (String) -> untyped
def read_refined_is_a(value)
  return :other unless value.is_a?(RefinedTarget)

  value.missing
end

#: (String) -> untyped
def read_refined_case_equality(value)
  return :other unless RefinedTarget === value

  value.missing
end
```

### result

```rbs
class Object < BasicObject
  def read_refined_is_a: (String value) -> untyped
  def read_refined_case_equality: (String value) -> untyped
end
```

## Class equality narrows an untyped input on its true branch

### update

```ruby
class UnknownCaseEqualityTarget
  #: -> String
  def version = "1.0"
end

class UnknownCaseEqualityGuard
  #: (untyped value) -> String?
  def read(value)
    return unless UnknownCaseEqualityTarget === value

    value.version
  end

  def inferred_case_equality_return(value)
    return :other unless UnknownCaseEqualityTarget === value

    value.version
  end

  #: (untyped value) -> String?
  def read_with_is_a(value)
    return unless value.is_a?(UnknownCaseEqualityTarget)

    value.version
  end
end
```

### result

```rbs
class UnknownCaseEqualityGuard
  def read: (untyped value) -> String?
  def inferred_case_equality_return: (untyped value) -> (String | :other)
  def read_with_is_a: (untyped value) -> String?
end

class UnknownCaseEqualityTarget
  def version: -> String
end
```

## Keep unknown instance_of? open when its method can be overridden

### update

```ruby
class UnknownInstanceOfTarget
  #: -> String
  def version = "1.0"
end

module AlwaysMatchStringInstanceOf
  refine String do
    def instance_of?(_target) = true
  end
end

using AlwaysMatchStringInstanceOf

def inferred_instance_of_return(value = eval("String.new"))
  return :other unless value.instance_of?(UnknownInstanceOfTarget)

  value.version
end
```

### result

```rbs
class Object < BasicObject
  def inferred_instance_of_return: (?untyped value) -> (untyped | :other)
end

class UnknownInstanceOfTarget
  def version: -> String
end
```

## Do not treat a mutable instance variable as a constant

### update

```ruby
class MutableInstanceVariableTruthiness
  def initialize
    @out_active = false
    @out_shifter = 0
    @out_dac = 0
    @out_buffer = 0
    @dma_length_counter = 0
    @buffered = false
  end

  def do_dma
    @buffered = true
    @dma_length_counter -= 1
  end

  #: (Integer dac, Integer buffer) -> Integer
  def set_output(dac, buffer)
    @out_dac = dac
    @out_buffer = buffer
  end

  def clock_dac
    if @out_active
      n = @out_dac + ((@out_buffer & 1) << 2) - 2
      @out_buffer >>= 1
      if 0 <= n && n <= 0x7f && n != @out_dac
        @out_dac = n
        return true
      end
    end
    return false
  end

  def false_after_same_method_write
    @out_active = false
    return true if @out_active

    false
  end

  def clock_dma
    if @out_shifter == 0
      @out_shifter = 7
      @out_active = @buffered
      if @out_active
        @buffered = false
        @out_buffer = 0
        do_dma if @dma_length_counter != 0
      end
    else
      @out_shifter -= 1
    end
  end
end
```

### result

```rbs
class MutableInstanceVariableTruthiness
  def initialize: -> void
  def do_dma: -> Integer
  def set_output: (Integer dac, Integer buffer) -> Integer
  def clock_dac: -> bool
  def false_after_same_method_write: -> false
  def clock_dma: -> Integer?
end
```

## Narrow a parent type with a subclass from another file

### update

### file: parent.rb

```ruby
class Parent
end
```

### file: child.rb

```ruby
class Child < Parent
  def child_only = "child".upcase
end
```

### file: checker.rb

```ruby
class Checker
  def child_value
    value = Parent.new
    return nil unless value.is_a?(Child)
    value.child_only
  end
end
```

### result

```rbs
class Child < Parent
  def child_only: -> String
end

class Checker
  def child_value: -> String?
end

class Parent
end
```

## Preserve an overlapping predicate and literal intersection

### update

```ruby
class CompoundConditionPredicate
  def present?(value)
    return false unless value
    true
  end

  #: (String?) -> String?
  def with_literal_condition(value)
    return nil unless present?(value) && value == "present"
    value
  end
end
```

### result

```rbs
class CompoundConditionPredicate
  def present?: (untyped value) -> bool
  def with_literal_condition: (String? value) -> String?
end
```

## Narrow an assignment before the right side of &&

```ruby
class ConditionAssignmentResource
  def readable? = true
  def value = "value"
end

class ConditionAssignmentFlow
  #: (bool) -> ConditionAssignmentResource?
  def maybe_resource(flag)
    if flag
      ConditionAssignmentResource.new
    end
  end

  def value_if_present(flag)
    if (resource = maybe_resource(flag)) && resource.readable?
      resource.value
    end
  end

  def value_unless_missing(flag)
    unless (resource = maybe_resource(flag)) && resource.readable?
      nil
    else
      resource.value
    end
  end

  def value_from_unless_expression(flag)
    result = unless (resource = maybe_resource(flag)) && resource.readable?
      nil
    else
      resource.value
    end
  end
end
```

### result

```rbs
class ConditionAssignmentFlow
  def maybe_resource: (bool flag) -> ConditionAssignmentResource?
  def value_if_present: (untyped flag) -> "value"?
  def value_unless_missing: (untyped flag) -> "value"?
  def value_from_unless_expression: (untyped flag) -> "value"?
end

class ConditionAssignmentResource
  def readable?: -> true
  def value: -> "value"
end
```

## Apply Array refinements to tuple unions

### update

```ruby
module AlwaysEmptyArray
  refine Array do
    def empty? = true
  end
end
using AlwaysEmptyArray

def refined_array_empty(flag)
  values = flag ? [] : ["item"]
  if values.empty?
    :reachable
  else
    :unreachable
  end
end
refined_array_empty(true)
```

### result

```rbs
class Object < BasicObject
  def refined_array_empty: (bool flag) -> :reachable
end
```

## Apply inherited Array refinements to subclasses

### update

```ruby
module AlwaysEmptyArray
  refine Array do
    def empty? = true
  end
end
using AlwaysEmptyArray

class MyArray < Array; end

def refined_array_subclass_empty
  if MyArray.new.empty?
    :reachable
  else
    :unreachable
  end
end
refined_array_subclass_empty
```

### result

```rbs
class Object < BasicObject
  def refined_array_subclass_empty: -> :reachable
end
```

## Let an ancestor refinement precede an Object fallback

### update

```ruby
class Object
  def refined_addition = :object
end

module ArrayAdditionRefinement
  refine Array do
    def refined_addition = :array
  end
end
using ArrayAdditionRefinement

class MyArray < Array; end

def refined_array_addition
  MyArray.new.refined_addition
end
refined_array_addition
```

### result

```rbs
class Object < BasicObject
  def refined_addition: -> :object
  def refined_array_addition: -> :array
end
```

## A subclass method wins over an ancestor refinement

### update

```ruby
module AlwaysEmptyArray
  refine Array do
    def empty? = true
  end
end
using AlwaysEmptyArray

class NeverEmptyArray < Array
  def empty? = false
end

def subclass_method_wins
  if NeverEmptyArray.new.empty?
    :refined
  else
    :subclass
  end
end
subclass_method_wins
```

### result

```rbs
class NeverEmptyArray < Array
  def empty?: -> false
end

class Object < BasicObject
  def subclass_method_wins: -> :subclass
end
```

## An active String#=~ refinement replaces literal regexp folding

### update

```ruby
module NeverMatchStringRegexp
  refine String do
    def =~(_regexp) = false
  end
end
using NeverMatchStringRegexp

def refined_regexp
  if "haystack" =~ /hay/
    :matched
  else
    :unmatched
  end
end
refined_regexp
```

### result

```rbs
class Object < BasicObject
  def refined_regexp: -> :unmatched
end
```

## Narrow an ivar inside a modifier if across files

### update

```ruby
module TypeProf
  module Core
    module AST
      class Node
      end

      class LoopNode
        #: (Node) -> void
        def initialize(cond)
          @cond = cond
        end

        def value
          @cond.var if @cond.is_a?(LocalVariableReadNode)
        end
      end
    end
  end
end
```

### file: ast/variable.rb

```ruby
module TypeProf
  module Core
    module AST
      class LocalVariableReadNode < Node
        def var = "var".upcase
      end
    end
  end
end
```

### result

```rbs
class TypeProf::Core::AST::LoopNode
  def initialize: (Node cond) -> void
  def value: -> String?
end
```
