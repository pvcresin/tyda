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

## Apply alternative pattern

```yaml
known_issue: true
```

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

## Apply range pattern

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

## Use string and symbol literal pattern as condition

```yaml
known_issue: true
```

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
  def check_text: (Symbol x) -> (:interpolated_symbol | :zzz)
end
```

## Use nil bool and special literal pattern as condition

```yaml
known_issue: true
```

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

## Branch with constant pattern

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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

## Bind captures in pattern predicate expressions

```yaml
known_issue: true
```

### update

```ruby
class PatternMatchExpressionBindings
  def assigned(value)
    matched = (value in Integer => number)
    number
  end

  def assigned_hash(value)
    matched = (value in { payload: Integer => payload })
    payload
  end

  def parenthesized_if(value)
    if (value in Integer => number)
      number
    else
      nil
    end
  end

  def parenthesized_ternary(value)
    (value in [Integer => number]) ? number : nil
  end

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

  def parenthesized_assignment_sequence(value)
    matched = (value in Integer => number; true)
    number
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

  def capture_after_parenthesized_if(value)
    if (value in Integer => number)
      :matched
    end
    number
  end

  def capture_in_or_condition(value, flag)
    if flag || (value in Integer => number)
      number
    else
      nil
    end
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

  def capture_in_statically_matched_guard_else
    value = -1
    case value
    in Integer => number if value.positive?
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

  def parenthesized_local_assignment
    result = (number = 1)
    number
  end

  def assignment_preserves_existing(value)
    number = :before
    matched = (value in Integer => number)
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
  def assigned: (untyped value) -> Integer?
  def assigned_hash: (untyped value) -> Integer?
  def parenthesized_if: (untyped value) -> Integer?
  def parenthesized_ternary: (untyped value) -> Integer?
  def short_circuit_and: (untyped value) -> (Integer | false)
  def negated_else: (untyped value) -> Integer?
  def and_condition: (untyped value, untyped flag) -> Integer?
  def parenthesized_assignment_sequence: (untyped value) -> Integer?
  def short_circuit_and_after: (untyped value) -> Integer?
  def short_circuit_and_rhs: (untyped value, untyped flag) -> Integer?
  def short_circuit_or: (untyped value) -> Integer?
  def short_circuit_or_rhs: (untyped value, untyped flag) -> Integer?
  def capture_after_parenthesized_if: (untyped value) -> Integer?
  def capture_in_or_condition: (untyped value, untyped flag) -> Integer?
  def capture_after_case_match: (untyped value) -> Integer
  def capture_in_case_else: (untyped value) -> :matched?
  def capture_in_statically_matched_guard_else: -> -1
  def case_match_preserves_existing: (untyped value) -> Integer
  def capture_after_case_alternatives: (untyped value) -> ([Integer, nil] | [nil, String])
  def parenthesized_local_assignment: -> 1
  def assignment_preserves_existing: (untyped value) -> (Integer | :before)
  def assignment_keeps_statically_matched_literal: -> "x"
end
```

## Do not merge locals from an exiting case/in branch

```yaml
known_issue: true
```

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
end
```

### result

```rbs
class Object < BasicObject
  def case_match_all_paths_return: (untyped value) -> (:number | :text)
end
```
