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
    value = x.nil?
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
  def plus_one: ((Integer | bool) x) -> Integer
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
  def plus_one: ((Integer | bool) x) -> Integer
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
    flag = x.nil?
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
  def value: (({ kind: Symbol, value: Integer } | { kind: Symbol, value: String }) x) -> (Integer | Symbol)
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

  def after_match(s)
    /(?<b>bar)/ =~ s
    b
  end
end
```

### result

```rbs
class C
  def check: -> String | 1
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
