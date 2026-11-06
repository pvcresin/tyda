# Ruby / Control / Narrowing

## Truthiness narrows then branch to non-nil

### update

```ruby
class Truthiness
  #: (String?) -> String
  def check(x)
    if x
      x.upcase
    else
      "none"
    end
  end
end
```

### result

```rbs
class Truthiness
  def check: (String? x) -> String
end
```

## Unless guard narrows body after early return

### update

```ruby
class UnlessGuard
  #: (Integer?) -> Integer
  def calc(x)
    unless x
      return 0
    end
    x + 1
  end
end
```

### result

```rbs
class UnlessGuard
  def calc: (Integer? x) -> Integer
end
```

## Early return guard narrows rest of method

### update

```ruby
class EarlyReturn
  #: (String?) -> String
  def process(x)
    return "" unless x
    x.upcase
  end
end
```

### result

```rbs
class EarlyReturn
  def process: (String? x) -> String
end
```

## Raise guard narrows rest of method

### update

```ruby
class RaiseGuard
  #: (Integer?) -> Integer
  def calc(x)
    raise "nil" if x.nil?
    x + 1
  end
end
```

### result

```rbs
class RaiseGuard
  def calc: (Integer? x) -> Integer
end
```

## nil? splits nil and non-nil branches

### update

```ruby
class NilCheck
  #: (String?) -> String
  def up(x)
    if x.nil?
      "none"
    else
      x.upcase
    end
  end
end
```

### result

```rbs
class NilCheck
  def up: (String? x) -> String
end
```

## is_a? narrows both branches of a union

### update

```ruby
class IsAGuard
  #: (Integer | String) -> String
  def check(x)
    if x.is_a?(Integer)
      x.to_s
    else
      x.upcase
    end
  end
end
```

### result

```rbs
class IsAGuard
  def check: ((Integer | String) x) -> String
end
```

## case when narrows the subject per branch

### update

```ruby
class CaseWhen
  #: (Integer | String) -> String
  def classify(v)
    case v
    when Integer
      v.to_s
    when String
      v.upcase
    else
      "unknown"
    end
  end
end
```

### result

```rbs
class CaseWhen
  def classify: ((Integer | String) v) -> String
end
```

## case in narrows the subject per pattern

### update

```ruby
class CaseIn
  #: (Integer | String) -> String
  def classify(v)
    case v
    in Integer
      v.to_s
    in String
      v.upcase
    end
  end
end
```

### result

```rbs
class CaseIn
  def classify: ((Integer | String) v) -> String
end
```

## else branch narrows to the complement

### update

```ruby
class ElseComplement
  #: (String?) -> (Symbol | 1)
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
class ElseComplement
  def convert: (String? x) -> (Symbol | 1)
end
```

## Reassignment resets narrowing

### update

```ruby
class ReassignReset
  #: (String?) -> String?
  def process(x)
    return "" unless x
    x = fetch
    x
  end

  #: -> String?
  def fetch = rand > 0.5 ? "s" : nil
end
```

### result

```rbs
class ReassignReset
  def process: (String? x) -> String?
  def fetch: -> String?
end
```

## AND conjunction narrows both operands

### update

```ruby
class AndConjunction
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
class AndConjunction
  def join: ((Integer | String)? a, (Integer | String)? b) -> String
end
```

## Unnarrowable condition keeps the type unchanged

### update

```ruby
class ComplexCond
  #: (String?) -> String?
  def process(x)
    if helper(x)
      x
    else
      x
    end
  end

  #: (String?) -> bool
  def helper(x) = !x.nil?
end
```

### result

```rbs
class ComplexCond
  def process: (String? x) -> String?
  def helper: (String? x) -> bool
end
```

## Truthy ivar guard narrows to a non-nil class method

### update

```ruby
class Journal
  def user = "u"
end

class Issue
  def guard_if
    @current_journal = rand < 0.5 ? Journal.new : nil
    if @current_journal
      @current_journal.user
    end
  end
end
```

### result

```rbs
class Issue
  def guard_if: -> "u"?
end

class Journal
  def user: -> "u"
end
```

## Ivar short-circuit AND keeps the narrowed return and nil branch

### update

```ruby
class Journal
  def user = "u"
end

class Issue
  def guard_and
    @current_journal = rand < 0.5 ? Journal.new : nil
    @current_journal && @current_journal.user
  end
end
```

### result

```rbs
class Issue
  def guard_and: -> "u"?
end

class Journal
  def user: -> "u"
end
```

## unless ivar.nil? guard narrows the ivar in the body

### update

```ruby
class Journal
  def user = "u"
end

class Issue
  def guard_unless_nil
    @current_journal = rand < 0.5 ? Journal.new : nil
    unless @current_journal.nil?
      @current_journal.user
    end
  end
end
```

### result

```rbs
class Issue
  def guard_unless_nil: -> "u"?
end

class Journal
  def user: -> "u"
end
```

## A known nil ivar makes its truthy branch unreachable

### update

```ruby
class NilIvarGuard
  def initialize
    @value = nil
  end

  def guarded
    if @value
      @value
    end
  end
end
```

### result

```rbs
class NilIvarGuard
  def initialize: -> void
  def guarded: -> nil
end
```

## Diverging helper on or RHS narrows the LHS

### update

```ruby
def src = rand < 0.5 ? "x" : nil
def fail_now = raise "no"

def go
  x = src or fail_now
  x
end
```

### result

```rbs
class Object < BasicObject
  def src: -> "x"?
  def fail_now: -> bot
  def go: -> "x"
end
```

## Inherited always-raising helper narrows after a nil guard

### update

```ruby
class Base
  def boom(_msg) = raise "stop"
end

class Sub < Base
  def run
    x = rand < 0.5 ? "hello" : nil
    boom("nil") if x.nil?
    x
  end
end
```

### result

```rbs
class Base
  def boom: (untyped _msg) -> bot
end

class Sub < Base
  def run: -> "hello"
end
```

## Included always-raising helper narrows after a nil guard

### update

```ruby
module Helpers
  def boom(_msg) = raise "stop"
end

class Worker
  include Helpers

  def run
    x = rand < 0.5 ? "hello" : nil
    boom("nil") if x.nil?
    x
  end
end
```

### result

```rbs
module Helpers
  def boom: (untyped _msg) -> bot
end

class Worker
  include Helpers

  def run: -> "hello"
end
```

## Implicit-self always-raising helper narrows after a nil guard

### update

```ruby
def boom = raise "stop"

def go
  x = rand < 0.5 ? "hello" : nil
  boom if x.nil?
  x
end
```

### result

```rbs
class Object < BasicObject
  def boom: -> bot
  def go: -> "hello"
end
```

## Raise unless narrows nil away

### update

```ruby
def foo
  n = [1, nil].sample
  raise "x" unless n
  n
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> 1
end
```

## Raise assignment leaves a falsy residual

### update

```ruby
def foo
  n = [1, nil].sample
  if n
    n = raise
    1
  end
  n
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> nil
end
```

## Literal equality else is the complement

### update

```ruby
def foo(x)
  if x == :a
    :yes
  else
    x
  end
end

foo(:a)
foo(:b)
```

### result

```rbs
class Object < BasicObject
  def foo: (Symbol x) -> Symbol
end
```

## Short-circuit filters nil from a deferred method return

### update

```ruby
module DeferredOr
  class Result
    def success? = true
  end

  class << self
    def find(login, password, project:, request:)
      result = first_check(login, password, project) ||
        second_check(login, password) ||
        third_check(login, password, request: request) ||
        Result.new
      result
    end

    def first_only(login, password, project)
      result = first_check(login, password, project) && Result.new
      result
    end

    def first_check(login, password, project) = rand < 0.5 ? Result.new : nil
    def second_check(login, password) = rand < 0.5 ? Result.new : nil
    def third_check(login, password, request:) = rand < 0.5 ? Result.new : nil
  end
end
```

### result

```rbs
module DeferredOr
  def self.find: (untyped login, untyped password, project: untyped, request: untyped) -> DeferredOr::Result
  def self.first_only: (untyped login, untyped password, untyped project) -> DeferredOr::Result?
  def self.first_check: (untyped login, untyped password, untyped project) -> DeferredOr::Result?
  def self.second_check: (untyped login, untyped password) -> DeferredOr::Result?
  def self.third_check: (untyped login, untyped password, request: untyped) -> DeferredOr::Result?
end

class DeferredOr::Result
  def success?: -> true
end
```

## Truthiness narrows a repeated pure implicit self reader

### update

```ruby
class ProjectTarget
  def name = "project"
end

class ProjectFinder
  #: () -> ProjectTarget?
  def project = @project

  def project_name
    if project
      project.name
    else
      "none"
    end
  end
end
```

### result

```rbs
class ProjectFinder
  def project: -> ProjectTarget?
  def project_name: -> "none" | "project"
end

class ProjectTarget
  def name: -> "project"
end
```

## Resolve type-test constants in the current namespace

### update

```ruby
module FirstNamespace
  class Foo
    def first_only = :first
  end

  def self.narrow(value)
    if value.is_a?(Foo)
      value.first_only
    else
      :other
    end
  end
end

module SecondNamespace
  class Foo
    def second_only = :second
  end
end

FirstNamespace.narrow(FirstNamespace::Foo.new)
FirstNamespace.narrow(SecondNamespace::Foo.new)
```

### result

```rbs
module FirstNamespace
  def self.narrow: ((FirstNamespace::Foo | SecondNamespace::Foo) value) -> (:first | :other)
end

class FirstNamespace::Foo
  def first_only: -> :first
end

class SecondNamespace::Foo
  def second_only: -> :second
end
```

## Resolve nested type-test constant paths in the current namespace

### update

```ruby
module NamespacePath
  module Nested
    class Child
      def nested_only = :nested
    end
  end

  def self.narrow(value)
    if value.is_a?(Nested::Child)
      value.nested_only
    else
      :other
    end
  end
end

module Nested
  class Child
    def root_only = :root
  end
end

NamespacePath.narrow(NamespacePath::Nested::Child.new)
NamespacePath.narrow(Nested::Child.new)
```

### result

```rbs
module NamespacePath
  def self.narrow: ((NamespacePath::Nested::Child | Nested::Child) value) -> (:nested | :other)
end

class NamespacePath::Nested::Child
  def nested_only: -> :nested
end

class Nested::Child
  def root_only: -> :root
end
```

## Keep unresolved nested type-test paths open when a top-level class collides

### update

```ruby
class CollisionTarget::Child
  def root_only = :root
end

module CollisionScope
  module CollisionTarget
  end

  def self.narrow(value)
    return :other unless value.is_a?(CollisionTarget::Child)

    value.root_only
  end
end

CollisionScope.narrow(eval("CollisionTarget::Child.new"))
```

### result

```rbs
module CollisionScope
  def self.narrow: (untyped value) -> (untyped | :other)
end

class CollisionTarget::Child
  def root_only: -> :root
end
```

## Narrow an unknown value after a class predicate

### update

```ruby
class DynamicNarrowTarget
  def marker = :narrowed
end

def narrow_dynamic_value(value)
  return :other unless value.is_a?(DynamicNarrowTarget)

  value.marker
end

narrow_dynamic_value(eval("DynamicNarrowTarget.new"))
```

### result

```rbs
class DynamicNarrowTarget
  def marker: -> :narrowed
end

class Object < BasicObject
  def narrow_dynamic_value: (untyped value) -> (:narrowed | :other)
end
```

## Keep unknown receivers open when is_a is overridden

### update

```ruby
class OverriddenIsATarget
  def is_a?(_target) = true
  def marker = :narrowed
end

def narrow_with_overridden_is_a(value)
  return :other unless value.is_a?(OverriddenIsATarget)

  value.marker
end

narrow_with_overridden_is_a(eval("OverriddenIsATarget.new"))
```

### result

```rbs
class Object < BasicObject
  def narrow_with_overridden_is_a: (untyped value) -> (untyped | :other)
end

class OverriddenIsATarget
  def is_a?: (untyped _target) -> true
  def marker: -> :narrowed
end
```

## Keep unknown receivers open under an active type-test refinement

### update

```ruby
class RefinedIsATarget
  def marker = :narrowed
end

module AlwaysMatchString
  refine String do
    def is_a?(_target) = true
  end
end

using AlwaysMatchString

def narrow_with_refined_is_a(value)
  return :other unless value.is_a?(RefinedIsATarget)

  value.marker
end

narrow_with_refined_is_a(eval("RefinedIsATarget.new"))
```

### result

```rbs
class Object < BasicObject
  def narrow_with_refined_is_a: (untyped value) -> (untyped | :other)
end

class RefinedIsATarget
  def marker: -> :narrowed
end
```

## An or condition proves nothing for the taken branch when one side is opaque

### update

```ruby
class OrConditionNarrowing
  def left_guard(flag)
    value = flag ? 1 : "value"
    if value.is_a?(Integer) || value.frozen?
      value
    else
      :other
    end
  end

  def right_guard(flag)
    value = flag ? 1 : "value"
    if value.frozen? || value.is_a?(Integer)
      value
    else
      :other
    end
  end

  def falsy_side_still_narrows(flag)
    value = flag ? 1 : "value"
    return :early if value.frozen? || value.is_a?(Integer)
    value
  end

  def and_keeps_taken_branch(flag)
    value = flag ? 1 : "value"
    if value.is_a?(Integer) && value.frozen?
      value
    else
      :other
    end
  end
end
```

### result

```rbs
class OrConditionNarrowing
  def left_guard: (untyped flag) -> (1 | "value" | :other)
  def right_guard: (untyped flag) -> (1 | "value" | :other)
  def falsy_side_still_narrows: (untyped flag) -> ("value" | :early)
  def and_keeps_taken_branch: (untyped flag) -> (1 | :other)
end
```

## Source predicates narrow guarded positional arguments

### update

```rbs
class SourcePredicateNarrowing
  def maybe_string: -> String?
end
```

```ruby
class SourcePredicateNarrowing
  def both_present?(left, right)
    return false unless left
    return nil unless right
    true
  end

  def always_true?(_value)
    true
  end

  def reassigned_guard?(value)
    return false unless value
    value = nil
    true
  end

  def expression_guard?(value)
    unless value
      false
    end
    true
  end

  def truthy_exit_guard?(value)
    return :early unless value
    true
  end

  def guarded_arguments
    left = maybe_string
    right = maybe_string
    return unless both_present?(left, right)

    left.upcase
    right.upcase
  end

  def nil_actual_control
    value = maybe_string
    return unless both_present?(nil, value)

    value.upcase
  end

  def unconditional_true_control
    value = maybe_string
    return unless always_true?(value)

    value.upcase
  end

  def reassignment_control
    value = maybe_string
    return unless reassigned_guard?(value)

    value.upcase
  end

  def expression_exit_control
    value = maybe_string
    return unless expression_guard?(value)

    value.upcase
  end

  def truthy_exit_control
    value = maybe_string
    return unless truthy_exit_guard?(value)

    value.upcase
  end
end
```

### result

```rbs
class SourcePredicateNarrowing
  def both_present?: (String? left, String? right) -> bool?
  def always_true?: (String? _value) -> true
  def reassigned_guard?: (String? value) -> bool
  def expression_guard?: (String? value) -> true
  def truthy_exit_guard?: (String? value) -> (true | :early)
  def guarded_arguments: -> String?
  def nil_actual_control: -> String?
  def unconditional_true_control: -> String?
  def reassignment_control: -> String?
  def expression_exit_control: -> String?
  def truthy_exit_control: -> String?
end
```

## Numeric equality keeps the subject's numeric class

### update

```rbs
class NumericEqualityNarrowing
  def maybe_integer: -> Integer?
  def maybe_float: -> Float?
end
```

```ruby
class NumericEqualityNarrowing
  def integer_against_integral_float
    value = maybe_integer
    value if value == 1.0
  end

  def integer_against_fractional_float
    value = maybe_integer
    value if value == 1.5
  end

  def float_against_integer
    value = maybe_float
    value if value == 1
  end
end
```

### result

```rbs
class NumericEqualityNarrowing
  def integer_against_integral_float: -> Integer?
  def integer_against_fractional_float: -> nil
  def float_against_integer: -> Float?
end
```
