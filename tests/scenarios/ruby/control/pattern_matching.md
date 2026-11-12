# Ruby / Control / Pattern Matching

## Apply narrowing from `in`

### update

```ruby
def foo(x)
  if x in String
    x
  else
    nil
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> String?
end
```

## Apply local binding from capture pattern

### update

```ruby
def foo(x)
  if x in Integer => y
    y
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> Integer?
end
```

## Treat `expr in Pattern` as bool

### update

```ruby
def foo(x)
  x in Integer
end

foo(1)
foo("x")
```

### result

```rbs
class Object < BasicObject
  def foo: ((Integer | String) x) -> bool
end
```

## Apply pin pattern

### update

```ruby
def foo(x)
  y = [1, 2]
  if x in ^y
    x
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> [1, 2]?
end
```

## Exclude an unmatched case/in path from normal returns

### update

```ruby
def check(x)
  case x
  in 1 | 2
    :ok
  in 3 | 4 | 5
    :ok
  end
end
```

### result

```rbs
class Object < BasicObject
  def check: (untyped x) -> :ok
end
```

## Continue with only matched case/in branches

### update

```ruby
def selected_label(value)
  case value
  in Integer
    label = :integer
  in String
    label = :string
  end
  label
end
```

### result

```rbs
class Object < BasicObject
  def selected_label: (untyped value) -> (:integer | :string)
end
```

## Apply range pattern

### update

```ruby
def check(x)
  case x
  in (0..)
    :ok1
  in -1
    :ok2
  end
end
```

### result

```rbs
class Object < BasicObject
  def check: (untyped x) -> (:ok1 | :ok2)
end
```

## Keep type with right assignment pattern

### update

```ruby
def check(x)
  x => Integer
  x
end

check(1)
begin
  check("x")
rescue NoMatchingPatternError
end
```

### result

```rbs
class Object < BasicObject
  def check: ((Integer | String) x) -> Integer
end
```

## Use array pattern as condition

### update

```ruby
class A
end

def check(x)
  case x
  in 1, 2, 3
    :foo
  in [1, 2, 3, *]
    :bar
  in [String]
    :baz
  in A[1, 2, 3]
    :qux
  in [1,]
    :waldo
  else
    :zzz
  end
end

check([1].to_a)
```

### result

```rbs
class Object < BasicObject
  def check: (Array[Integer] x) -> (:bar | :foo | :waldo | :zzz)
end
```

## Use hash pattern as condition

### update

```ruby
class A
end

def check(x)
  case x
  in { a: Integer }
    :foo
  in { a: String, ** }
    :bar
  in { a: }
    :baz
  in A[a: Integer]
    :qux
  else
    :zzz
  end
end

check({ a: 42 })
```

### result

```rbs
class Object < BasicObject
  def check: ({ a: Integer } x) -> :foo
end
```

## Use numeric literal pattern as condition

### update

```ruby
def check_numeric(x)
  case x
  in 1
    :int
  in 1.0
    :float
  in 1r
    :rational
  in 1i
    :complex
  else
    :zzz
  end
end

check_numeric(1)
```

### result

```rbs
class Object < BasicObject
  def check_numeric: (Integer x) -> (:int | :zzz)
end
```

## Non-literal numeric subjects can match numeric literal patterns of another class

### update

```ruby
class CrossClassNumericPattern
  #: () -> Integer
  def int = 1

  #: () -> Float
  def float = 1.0

  def integer_against_float
    case int
    in 1.0
      :float
    else
      :other
    end
  end

  def float_against_integer
    case float
    in 1
      :int
    else
      :other
    end
  end
end
```

### result

```rbs
class CrossClassNumericPattern
  def int: -> Integer
  def float: -> Float
  def integer_against_float: -> (:float | :other)
  def float_against_integer: -> (:int | :other)
end
```

## Numeric literal patterns stay reachable for other numeric subjects

### update

```ruby
class OtherNumericPattern
  #: () -> Numeric
  def number = 1

  #: () -> Rational
  def ratio = 1r

  #: () -> Integer
  def int = 1

  def numeric_against_integer
    case number
    in 1
      :one
    else
      :other
    end
  end

  def rational_against_integer
    case ratio
    in 1
      :one
    in 1.0
      :again
    else
      :other
    end
  end

  def guarded_value_stays_open
    case int
    in 1 if rand > 0.5
      :guarded
    in 1.0
      :float
    in 2i
      :imaginary
    else
      :other
    end
  end
end
```

### result

```rbs
class OtherNumericPattern
  def number: -> Numeric
  def ratio: -> Rational
  def int: -> Integer
  def numeric_against_integer: -> (:one | :other)
  def rational_against_integer: -> (:one | :other)
  def guarded_value_stays_open: -> (:float | :guarded | :other)
end
```

## Integer subject cannot equal a fractional float pattern

### update

```ruby
class FractionalFloatPattern
  #: () -> Integer
  def int = 1

  def integer_against_fractional
    case int
    in 1.5
      :fraction
    else
      :other
    end
  end
end
```

### result

```rbs
class FractionalFloatPattern
  def int: -> Integer
  def integer_against_fractional: -> :other
end
```

## Use string and symbol literal pattern as condition

### update

```ruby
def check_interpolation(x)
end

def check_text(x)
  case x
  in "foo"
    :string
  in "foo#{ check_interpolation(:ok_str) }"
    :interpolated_string
  in :foo
    :symbol
  in :"foo#{ check_interpolation(:ok_sym) }"
    :interpolated_symbol
  else
    :zzz
  end
end

check_text(:AAA)
```

### result

```rbs
class Object < BasicObject
  def check_interpolation: (untyped x) -> nil
  def check_text: (Symbol x) -> (:interpolated_symbol | :symbol | :zzz)
end
```

## Keep open symbols in literal symbol patterns

### update

```ruby
def check_open_symbol(value)
  case value
  in "foo"
    :string
  in :foo
    :symbol
  else
    :zzz
  end
end

check_open_symbol(:AAA)
```

### result

```rbs
class Object < BasicObject
  def check_open_symbol: (Symbol value) -> (:symbol | :zzz)
end
```

## Use nil bool and special literal pattern as condition

### update

```ruby
def check_special(x)
  case x
  in nil
    :nil
  in false
    :false
  in true
    :false
  in __FILE__
    :file
  in __LINE__
    :line
  in __ENCODING__
    :encoding
  in %w[foo bar]
    :w_lit
  else
    :zzz
  end
end

check_special(nil)
```

### result

```rbs
class Object < BasicObject
  def check_special: (nil x) -> :nil
end
```

## Preserve unmatched nil and boolean pattern branches

### update

```ruby
def check_nil_or_string(value)
  case value
  in nil
    :nil
  else
    :other
  end
end

check_nil_or_string(nil)
check_nil_or_string("text")

def check_integer_against_nil(value)
  case value
  in nil
    :nil
  else
    :other
  end
end

check_integer_against_nil(1)

def check_boolean(value)
  case value
  in false
    :false
  in true
    :true
  end
end

check_boolean(false)
check_boolean(true)

def check_wildcard_before_nil(value)
  case value
  in _
    :first
  in nil
    :second
  else
    :third
  end
end

check_wildcard_before_nil(nil)

def check_variable_before_nil(value)
  case value
  in captured
    :first
  in nil
    :second
  else
    :third
  end
end

check_variable_before_nil(nil)
```

### result

```rbs
class Object < BasicObject
  def check_nil_or_string: (String? value) -> (:nil | :other)
  def check_integer_against_nil: (Integer value) -> :other
  def check_boolean: (bool value) -> (:false | :true)
  def check_wildcard_before_nil: (nil value) -> :first
  def check_variable_before_nil: (nil value) -> :first
end
```

## Branch with constant pattern

### update

```ruby
def check(x)
  case x
  in Integer
    :int
  in String
    :str
  end
end

check(1)
check("x")
```

### result

```rbs
class Object < BasicObject
  def check: ((Integer | String) x) -> (:int | :str)
end
```

## Use find pattern as condition

### update

```ruby
def check(x)
  case x
  in *a, Integer, *b
    :foo
  in *a, String, *b
    :bar
  else
    :zzz
  end
end

check([1].to_a)
```

### result

```rbs
class Object < BasicObject
  def check: (Array[Integer] x) -> (:foo | :zzz)
end
```

## Apply pattern guard

### update

```ruby
def cond?(x) = x

def check(x)
  case x
  in 1 if cond?(:ok)
    :ok
  end
end
```

### result

```rbs
class Object < BasicObject
  def cond?: (Symbol x) -> Symbol
  def check: (untyped x) -> :ok
end
```

## Preserve types captured by a variable pattern

### update

```ruby
def check(x)
  case x
  in y
    y
  end
end

check(1)
check("x")
```

### result

```rbs
class Object < BasicObject
  def check: ((Integer | String) x) -> (Integer | String)
end
```

## Infer types captured by an array pattern

### update

```ruby
def check(x)
  case x
  in a, b, c, *rest
    [a, b, c, rest]
  end
end

check([1, 2, 3, 4])
check(["a", "b", "c", "d"])
```

### result

```rbs
class Object < BasicObject
  def check: (Array[Integer | String] x) -> [Integer | String, Integer | String, Integer | String, Array[Integer | String]]
end
```

## Infer types captured by a hash pattern

### update

```ruby
def check(x)
  case x
  in { a:, b:, c:, **rest }
    [a, b, c, rest]
  end
end

check({ a: 1, b: "one", c: true, d: :left })
check({ a: 2, b: "two", c: false, d: :right })
```

### result

```rbs
class Object < BasicObject
  def check: ({ a: Integer, b: String, c: bool, d: Symbol } x) -> [Integer, String, bool, { d: Symbol }]
end
```

## Array variable pattern keeps tuple elements

### update

```ruby
def check(x)
  case x
  in a, b, c, *rest
    [a, b, c, rest]
  end
end

check([1, 2, 3, 4])
```

### result

```rbs
class Object < BasicObject
  def check: (Array[Integer] x) -> [Integer, Integer, Integer, Array[Integer]]
end
```

## Hash variable pattern keeps record fields

### update

```ruby
def check(x)
  case x
  in { a:, b:, c:, **rest }
    [a, b, c, rest]
  end
end

check({ a: 1, b: 2, c: 3, d: 4 })
```

### result

```rbs
class Object < BasicObject
  def check: ({ a: Integer, b: Integer, c: Integer, d: Integer } x) -> [Integer, Integer, Integer, { d: Integer }]
end
```

## Find pattern keeps before and after rest as arrays

### update

```ruby
def check(x)
  case x
  in *a, Integer, *b
    [a, b]
  end
end

check([1, 2, 3])
```

### result

```rbs
class Object < BasicObject
  def check: (Array[Integer] x) -> [Array[Integer], Array[Integer]]
end
```

## Use local bound by one-line pattern matching later

### update

```ruby
def standalone_in_array
  value = [1, "x"]
  value in [a, b]
  b
end

def standalone_required_hash
  value = {name: "ruby", version: 3}
  value => {name:}
  name
end
```

### result

```rbs
class Object < BasicObject
  def standalone_in_array: -> "x"
  def standalone_required_hash: -> "ruby"
end
```

## Nested capture pattern binds the asserted type

### update

```ruby
class Decoder
  def point(v)
    case v
    in { x: Integer => x, y: Integer => y }
      x + y
    end
  end

  def pair(v)
    case v
    in [String => name, Integer => age]
      [name, age]
    end
  end
end
```

### result

```rbs
class Decoder
  def point: (untyped v) -> Integer
  def pair: (untyped v) -> [String, Integer]
end
```

## Pattern capture uses the binding after match

### update

```ruby
def check(a)
  case a
  in Integer => n
    n + 1
  in String => s
    s.upcase
  end
end

check(1)
check("foo")
```

### result

```rbs
class Object < BasicObject
  def check: ((Integer | String) a) -> (Integer | String)
end
```

## Infer values used by a pattern guard

### update

```ruby
def valid_capacity?(capacity) = capacity < 100

def classify_capacity(value)
  case value
  in [amount] if valid_capacity?(amount)
    :available
  else
    :full
  end
end

classify_capacity([20])
```

### result

```rbs
class Object < BasicObject
  def valid_capacity?: (Integer capacity) -> bool
  def classify_capacity: (Array[Integer] value) -> (:available | :full)
end
```

## Narrow a Hash capture pattern within a Record union

### update

```ruby
class PatternHashCaptureNarrowing
  def hash_capture_union(flag)
    value = flag ? { payload: 1 } : { payload: "x" }
    case value
    in { payload: Integer => payload }
      payload
    else
      nil
    end
  end

  def hash_capture_subclass(flag)
    value = { payload: flag ? PatternChildProbe.new : PatternOtherProbe.new }
    case value
    in { payload: PatternParentProbe => payload }
      payload
    else
      nil
    end
  end
end

class PatternParentProbe; end
class PatternChildProbe < PatternParentProbe; end
class PatternOtherProbe; end
```

### result

```rbs
class PatternHashCaptureNarrowing
  def hash_capture_union: (untyped flag) -> 1?
  def hash_capture_subclass: (untyped flag) -> PatternChildProbe?
end
```

## Include failed predicate paths in captured locals

### update

```ruby
class MatchPredicateFailurePaths
  def unknown_array(value)
    value in [Integer => item]
    item
  end

  def unknown_hash(value)
    value in { payload: Integer => payload }
    payload
  end

  def existing_local(value)
    item = :before
    value in Integer => item
    item
  end
end
```

### result

```rbs
class MatchPredicateFailurePaths
  def unknown_array: (untyped value) -> Integer?
  def unknown_hash: (untyped value) -> Integer?
  def existing_local: (untyped value) -> (Integer | :before)
end
```

## Bind captures in short-circuit pattern predicate expressions

### update

```ruby
class PatternMatchExpressionBindings
  def short_circuit_and(value)
    (value in Integer => number) && number
  end

  def negated_else(value)
    if !(value in Integer => number)
      nil
    else
      number
    end
  end

  def and_condition(value, flag)
    if flag && (value in Integer => number)
      number
    else
      nil
    end
  end

  def short_circuit_and_after(value)
    (value in Integer => number) && true
    number
  end

  def short_circuit_and_rhs(value, flag)
    flag && (value in Integer => number)
    number
  end

  def short_circuit_or(value)
    (value in Integer => number) || true
    number
  end

  def short_circuit_or_rhs(value, flag)
    flag || (value in Integer => number)
    number
  end

  def capture_in_or_condition(value, flag)
    if flag || (value in Integer => number)
      number
    else
      nil
    end
  end

  def capture_in_guard_else(value, flag)
    case value
    in Integer => number if flag
      :matched
    else
      number
    end
  end

  def capture_in_unless_guard_else(value, flag)
    case value
    in Integer => number unless flag
      :matched
    else
      number
    end
  end

  def capture_kept_for_later_arm(value, flag)
    case value
    in Integer => number if flag
      :matched
    in String
      number
    end
  end

  def capture_in_negated_statement(value)
    !(value in Integer => number)
    number
  end

  def assignment_in_short_circuit_rhs(flag)
    flag && (number = 1)
    number
  end

  def assignment_in_or_rhs(flag)
    flag || number = 1
    number
  end

  def partial_capture_on_failed_match(value)
    if (value in [Integer => first, String])
      :matched
    else
      first
    end
  end

  def capture_in_loop_condition(value)
    while (value in Integer => number)
      return number
    end
  end

  def parenthesized_local_assignment
    result = (number = 1)
    number
  end

  def assignment_keeps_statically_matched_literal
    value = [1, "x"]
    matched = (value in [first, second])
    second
  end
end
```

### result

```rbs
class PatternMatchExpressionBindings
  def short_circuit_and: (untyped value) -> (Integer | false)
  def negated_else: (untyped value) -> Integer?
  def and_condition: (untyped value, untyped flag) -> Integer?
  def short_circuit_and_after: (untyped value) -> Integer?
  def short_circuit_and_rhs: (untyped value, untyped flag) -> Integer?
  def short_circuit_or: (untyped value) -> Integer?
  def short_circuit_or_rhs: (untyped value, untyped flag) -> Integer?
  def capture_in_or_condition: (untyped value, untyped flag) -> Integer?
  def capture_in_guard_else: (untyped value, untyped flag) -> (Integer | :matched)?
  def capture_in_unless_guard_else: (untyped value, untyped flag) -> (Integer | :matched)?
  def capture_kept_for_later_arm: (untyped value, untyped flag) -> (Integer | :matched)?
  def capture_in_negated_statement: (untyped value) -> Integer?
  def assignment_in_short_circuit_rhs: (untyped flag) -> 1?
  def assignment_in_or_rhs: (untyped flag) -> 1?
  def partial_capture_on_failed_match: (untyped value) -> (Integer | :matched)?
  def capture_in_loop_condition: (untyped value) -> Integer?
  def parenthesized_local_assignment: -> 1
  def assignment_keeps_statically_matched_literal: -> "x"
end
```

## Keep both case arms because the literal guard is not evaluated

### update

```ruby
class PatternMatchLiteralGuard
  def capture_in_statically_matched_guard_else
    value = -1
    case value
    in Integer => number if value.positive?
      :matched
    else
      number
    end
  end
end
```

### result

```rbs
class PatternMatchLiteralGuard
  def capture_in_statically_matched_guard_else: -> (-1 | :matched)
end
```

## Preserve captures across successful case patterns

### update

```ruby
class PatternMatchExpressionBindings
  def parenthesized_ternary(value)
    (value in [Integer => number]) ? number : nil
  end

  def capture_after_case_match(value)
    case value
    in Integer => number
      :matched
    end
    number
  end

  def capture_in_case_else(value)
    case value
    in Integer => number
      :matched
    else
      number
    end
  end

  def case_match_preserves_existing(value)
    number = :before
    case value
    in Integer => number
      :matched
    end
    number
  end

  def capture_after_case_alternatives(value)
    case value
    in Integer => integer
      :integer
    in String => string
      :string
    end
    [integer, string]
  end
end
```

### result

```rbs
class PatternMatchExpressionBindings
  def parenthesized_ternary: (untyped value) -> Integer?
  def capture_after_case_match: (untyped value) -> Integer
  def capture_in_case_else: (untyped value) -> :matched?
  def case_match_preserves_existing: (untyped value) -> Integer
  def capture_after_case_alternatives: (untyped value) -> ([Integer, nil] | [nil, String])
end
```

## Bind captures in a parenthesized pattern condition

### update

```ruby
class PatternMatchExpressionBindings
  def parenthesized_if(value)
    if (value in Integer => number)
      number
    else
      nil
    end
  end
end
```

### result

```rbs
class PatternMatchExpressionBindings
  def parenthesized_if: (untyped value) -> Integer?
end
```

## Preserve captures after a parenthesized pattern condition

### update

```ruby
class PatternMatchExpressionBindings
  def capture_after_parenthesized_if(value)
    if (value in Integer => number)
      :matched
    end
    number
  end
end
```

### result

```rbs
class PatternMatchExpressionBindings
  def capture_after_parenthesized_if: (untyped value) -> Integer?
end
```

## Do not merge locals from an exiting case/in branch

### update

```ruby
def case_match_local_after_return(value)
  case value
  in Integer
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
  def case_match_local_after_return: (untyped value) -> :returned?
end
```

## Ignore code after every case/in branch exits

### update

```ruby
def case_match_all_paths_return(value)
  case value
  in Integer
    selected = :number
    return :number
  in String
    selected = :text
    return :text
  end
  selected
  :unreachable
end
```

### result

```rbs
class Object < BasicObject
  def case_match_all_paths_return: (untyped value) -> (:number | :text)
end
```

## Bind captures from an assigned pattern predicate

### update

```ruby
class PatternMatchAssignmentBindings
  def assigned(value)
    matched = (value in Integer => number)
    number
  end

  def assigned_hash(value)
    matched = (value in { payload: Integer => payload })
    payload
  end

  def assigned_sequence(value)
    matched = ((value in Integer => number); true)
    number
  end

  def assigned_sequence_after_prefix(value)
    matched = (true; (value in Integer => number); true)
    number
  end

  def assigned_sequence_after_overwrite(value)
    matched = ((value in Integer => number); number = :after; true)
    number
  end

  def preserve_existing_on_failure(value)
    number = :before
    matched = (value in Integer => number)
    number
  end

  def assignment_target_overwrites_capture(value)
    number = (value in Integer => number)
    number
  end
end
```

### result

```rbs
class PatternMatchAssignmentBindings
  def assigned: (untyped value) -> Integer?
  def assigned_hash: (untyped value) -> Integer?
  def assigned_sequence: (untyped value) -> Integer?
  def assigned_sequence_after_prefix: (untyped value) -> Integer?
  def assigned_sequence_after_overwrite: (untyped value) -> :after
  def preserve_existing_on_failure: (untyped value) -> (Integer | :before)
  def assignment_target_overwrites_capture: (untyped value) -> bool
end
```

## Do not bind captures from expressions that may not run

### update

```ruby
class PatternMatchAssignmentEvaluation
  def skipped_short_circuit(value)
    number = :before
    false && (value in Integer => number)
    number
  end

  def skipped_assignment_short_circuit(value)
    matched = (false && (value in Integer => number))
    number
  end

  def deferred_block(value)
    number = :before
    callback = -> { value in Integer => number }
    number
  end
end
```

### result

```rbs
class PatternMatchAssignmentEvaluation
  def skipped_short_circuit: (untyped value) -> :before
  def skipped_assignment_short_circuit: (untyped value) -> nil
  def deferred_block: (untyped value) -> :before
end
```

## Check the class before narrowing a hash pattern

### update

```ruby
class PatternHashBase < Hash
end

class PatternHashChild < PatternHashBase
  def deconstruct_keys(_keys) = {}
end

class PatternHashUnrelated
end

class PatternHashAlwaysMiss
  def self.===(_value) = false
end

class PatternHashClassPatterns
  def self.hash_base
    case { a: 42 }
    in Hash[a: Integer]
      :matched
    else
      :other
    end
  end

  def self.unrelated_class
    case { a: 42 }
    in PatternHashUnrelated[a: Integer]
      :matched
    else
      :other
    end
  end

  def self.hash_subclass
    case { a: 42 }
    in PatternHashChild[a: Integer]
      :matched
    else
      :other
    end
  end

  def self.nested_unrelated_class
    case { outer: { a: 42 } }
    in { outer: PatternHashUnrelated[a: Integer] }
      :matched
    else
      :other
    end
  end

  def self.class_equality_override
    case { a: 42 }
    in PatternHashAlwaysMiss[a: Integer]
      :matched
    else
      :other
    end
  end

  def self.unresolved_constant
    case { a: 42 }
    in MissingHashPattern[a: Integer]
      :matched
    else
      :other
    end
  end

  def self.hash_subclass_deconstruction
    value = PatternHashChild.new
    value[:a] = 42
    case value
    in { a: Integer }
      :matched
    else
      :other
    end
  end
end
```

### result

```rbs
class PatternHashAlwaysMiss
  def self.===: (untyped _value) -> false
end

class PatternHashChild < PatternHashBase
  def deconstruct_keys: (untyped _keys) -> Hash[untyped, untyped]
end

class PatternHashClassPatterns
  def self.hash_base: -> :matched
  def self.unrelated_class: -> :other
  def self.hash_subclass: -> (:matched | :other)
  def self.nested_unrelated_class: -> :other
  def self.class_equality_override: -> (:matched | :other)
  def self.unresolved_constant: -> (:matched | :other)
  def self.hash_subclass_deconstruction: -> (:matched | :other)
end
```

## Keep class pattern branches open under an active equality refinement

### update

```ruby
class RefinedHashPatternTarget
end

module AlwaysMatchModuleCase
  refine Module do
    def ===(_value) = true
  end
end

class RefinedHashPatternProbe
  using AlwaysMatchModuleCase

  def self.classify
    case { a: 42 }
    in RefinedHashPatternTarget[a: Integer]
      :matched
    else
      :other
    end
  end
end
```

### result

```rbs
class RefinedHashPatternProbe
  def self.classify: -> (:matched | :other)
end
```

## Match integer and float literal patterns through numeric equality

### update

```ruby
def integer_matches_float_pattern
  case 1
  in 1.0
    :matched
  else
    :unmatched
  end
end

def float_matches_integer_pattern
  case 1.0
  in 1
    :matched
  else
    :unmatched
  end
end
```

### result

```rbs
class Object < BasicObject
  def integer_matches_float_pattern: -> :matched
  def float_matches_integer_pattern: -> :matched
end
```

## A catch-all binding after a nil pattern excludes nil

### update

```rbs
class NilFirstMatcher
  def value: -> (Integer | String)?
end
```

```ruby
class NilFirstMatcher
  def run
    case value
    in nil then 0
    in other then other
    end
  end
end
```

### result

```rbs
class NilFirstMatcher
  def run: -> (Integer | String)
end
```

## A range pattern on an Integer keeps both branches

### update

```rbs
class RangePatternMatcher
  def score: -> Integer
end
```

```ruby
class RangePatternMatcher
  def label
    case score
    in 1..5 then :low
    else :other
    end
  end
end
```

### result

```rbs
class RangePatternMatcher
  def label: -> (:low | :other)
end
```

## Alternative pattern narrows the subject

```yaml
known_issue: true
```

### update

```ruby
class Matcher
  # @rbs value: Integer | String | Symbol
  def run(value)
    case value
    in Integer | String then value
    else nil
    end
  end
end
```

### result

```rbs
class Matcher
  def run: ((Integer | String | Symbol) value) -> (Integer | String)?
end
```

## Array pattern element capture binds the matched class

```yaml
known_issue: true
```

### update

```ruby
class Matcher
  # @rbs values: Array[Integer | String]
  def run(values)
    case values
    in [Integer => first, *] then first
    else nil
    end
  end
end
```

### result

```rbs
class Matcher
  def run: (Array[Integer | String] values) -> Integer?
end
```

## Find pattern capture binds the matched class

```yaml
known_issue: true
```

### update

```ruby
class Matcher
  # @rbs values: Array[Integer | String]
  def run(values)
    case values
    in [*, String => found, *] then found
    else nil
    end
  end
end
```

### result

```rbs
class Matcher
  def run: (Array[Integer | String] values) -> String?
end
```

## Hash pattern value capture binds the matched class

```yaml
known_issue: true
```

### update

```ruby
class Matcher
  # @rbs data: Hash[Symbol, Integer | String]
  def run(data)
    case data
    in { name: String => name } then name
    else nil
    end
  end
end
```

### result

```rbs
class Matcher
  def run: (Hash[Symbol, Integer | String] data) -> String?
end
```

## Lambda pattern matches through case equality

### update

```ruby
class Classifier
  def classify(value)
    case value
    in ->(n) { n.zero? }
      :zero
    in Integer
      :number
    end
  end
end

Classifier.new.classify(0)
```

### result

```rbs
class Classifier
  def classify: (Integer value) -> (:number | :zero)
end
```
