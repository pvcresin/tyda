# Ruby / Control / Loops

## while loop returns nil

### update

```ruby
def foo
  while true
    1
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> nil
end
```

## until loop returns nil

### update

```ruby
def foo
  until false
    "hello"
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> nil
end
```

## Return value after loop

### update

```ruby
def foo
  x = 0
  while x < 10
    x = x + 1
  end
  x
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> Integer
end
```

## while break value

### update

```ruby
def while_break
  while true
    break :done
  end
end
```

### result

```rbs
class Object < BasicObject
  def while_break: -> :done
end
```

## until break value

### update

```ruby
def until_break
  until false
    break :done
  end
end
```

### result

```rbs
class Object < BasicObject
  def until_break: -> :done
end
```

## while bare break returns nil

### update

```ruby
def while_bare_break
  while true
    break
  end
end
```

### result

```rbs
class Object < BasicObject
  def while_bare_break: -> nil
end
```

## while break with multiple values

### update

```ruby
def while_break_multiple
  while true
    break 1, "two"
  end
end
```

### result

```rbs
class Object < BasicObject
  def while_break_multiple: -> [1, "two"]
end
```

## for break value

### update

```ruby
def for_break_value
  for x in [1, 2]
    break :done
  end
end
```

### result

```rbs
class Object < BasicObject
  def for_break_value: -> :done
end
```

## Kernel#loop break value

### update

```ruby
def kernel_loop_break
  loop do
    break :done
  end
end
```

### result

```rbs
class Object < BasicObject
  def kernel_loop_break: -> :done
end
```

## Kernel#loop bare break

### update

```ruby
def kernel_loop_bare_break
  loop do
    break
  end
end
```

### result

```rbs
class Object < BasicObject
  def kernel_loop_bare_break: -> nil
end
```

## redo does not affect loop break value

### update

```ruby
def loop_with_redo
  loop do
    redo if false
    break :done
  end
end
```

### result

```rbs
class Object < BasicObject
  def loop_with_redo: -> :done
end
```

## modifier while

### update

```ruby
def while_modifier
  x = 0
  x += 1 while x < 3
  x
end
```

### result

```rbs
class Object < BasicObject
  def while_modifier: -> Integer
end
```

## modifier until

### update

```ruby
def until_modifier
  x = 0
  x += 1 until x == 3
  x
end
```

### result

```rbs
class Object < BasicObject
  def until_modifier: -> 3
end
```

## post-condition while

### update

```ruby
def begin_while
  x = 0
  begin
    x = 1
  end while false
  x
end
```

### result

```rbs
class Object < BasicObject
  def begin_while: -> 1
end
```

## While-loop multiply widens a constant accumulator

```yaml
known_issue: true
```

### update

```ruby
def double_three_times
  d = 1
  i = 0
  while i < 3
    d *= 2
    i += 1
  end
  d
end
```

### result

```rbs
class Object < BasicObject
  def double_three_times: -> Integer
end
```

## Loop-local first assignment is nilable if the loop may not run

### update

```ruby
def maybe_fresh(flag)
  counter = 0
  while counter < 2 && flag
    fresh = counter * 2
    counter += 1
  end
  fresh
end
```

### result

```rbs
class Object < BasicObject
  def maybe_fresh: (untyped flag) -> Integer?
end
```

## Until false break value

### update

```ruby
def foo
  until false
    break :a
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> :a
end
```

## While-loop push fixpoint types collected indices

### update

```ruby
def collect_indices
  acc = []
  m = 0
  while m < 3
    acc.push(m)
    m += 1
  end
  acc
end
```

### result

```rbs
class Object < BasicObject
  def collect_indices: -> Array[Integer]
end
```

## Truthy while nils the loop variable

### update

```ruby
def test
  x = [1, nil].sample
  while x
    x + 1
  end
  x
end
```

### result

```rbs
class Object < BasicObject
  def test: -> nil
end
```

## Multi-value break in times

```yaml
known_issue: true
```

### update

```ruby
def check
  1.times do
    break :a, :b, :c
  end
end
```

### result

```rbs
class Object < BasicObject
  def check: -> [:a, :b, :c]
end
```

## Ignore code after loop returns from the method

```yaml
known_issue: true
```

### update

```ruby
def loop_returns
  loop do
    return :returned
  end
  :unreachable
end

def loop_returns_from_begin
  loop do
    begin
      return :returned
    end
  end
  :unreachable
end

def while_true_returns_from_branches
  while true
    case [true, false].sample
    when true
      return :first
    else
      return :second
    end
  end
  :unreachable
end

def while_true_returns_from_ternary
  while true
    return([true, false].sample ? :first : :second)
  end
  :unreachable
end

def begin_modifier_returns
  begin
    return :returned
  end while false
  :unreachable
end

def loop_break_continues
  loop do
    break
  end
  :after
end

def while_true_returns
  while true
    return :returned
  end
  :unreachable
end

def until_false_returns
  until false
    return :returned
  end
  :unreachable
end

def while_truthy_literal_returns
  while 1
    return :returned
  end
  :unreachable
end

def until_nil_returns
  until nil
    return :returned
  end
  :unreachable
end

def while_parenthesized_true_returns
  while (true)
    return :returned
  end
  :unreachable
end

def while_return_before_unreachable
  while true
    return :returned
    :unreachable
  end
  :unreachable
end

def while_break_before_return_continues
  while true
    if [true, false].sample
      break
    end
    return :returned
  end
  :after
end

def loop_break_before_return_continues
  loop do
    if [true, false].sample
      break
    end
    return :returned
  end
  :after
end

def while_modifier_break_or_return
  while true
    break if [true, false].sample
    return :returned
  end
  :after
end

def while_short_circuit_break_or_return
  while true
    [true, false].sample && break
    return :returned
  end
  :after
end

def while_short_circuit_or_break_or_return
  while true
    [true, false].sample || break
    return :returned
  end
  :after
end

def while_false_and_break_then_return
  while true
    false && break
    return :returned
  end
  :after
end

def while_true_or_break_then_return
  while true
    true || break
    return :returned
  end
  :after
end

def while_true_and_break_returns_after
  while true
    true && break
    return :unreachable
  end
  :after
end

def while_false_or_break_returns_after
  while true
    false || break
    return :unreachable
  end
  :after
end

def while_if_true_break_then_return
  while true
    if true
      break
    end
    return :unreachable
  end
  :after
end

def while_if_true_break_skips_else_return
  while true
    if true
      break
    else
      return :unreachable
    end
    return :unreachable_after
  end
  :after
end

def while_if_false_break_skips_then_return
  while true
    if false
      return :unreachable
    else
      break
    end
    return :unreachable_after
  end
  :after
end

def while_elsif_literal_break_skips_dead_returns
  while true
    if false
      return :unreachable
    elsif true
      break
    else
      return :unreachable_else
    end
    return :unreachable_after
  end
  :after
end

def while_if_false_break_then_return
  while true
    if false
      break
    end
    return :returned
  end
  :after
end

def while_if_not_false_break_then_return
  while true
    if !false
      break
    end
    return :unreachable
  end
  :after
end

def while_unless_false_break_then_return
  while true
    unless false
      break
    end
    return :unreachable
  end
  :after
end

def while_unless_false_break_skips_else_return
  while true
    unless false
      break
    else
      return :unreachable
    end
    return :unreachable_after
  end
  :after
end

def while_unless_true_break_then_return
  while true
    unless true
      break
    end
    return :returned
  end
  :after
end

def while_ternary_true_break_then_return
  while true
    true ? break : nil
    return :unreachable
  end
  :after
end

def while_ternary_false_break_then_return
  while true
    false ? break : nil
    return :returned
  end
  :after
end

def while_case_literal_break_then_return
  while true
    case :finish
    when :finish
      break
    else
      nil
    end
    return :unreachable
  end
  :after
end

def while_case_literal_miss_then_return
  while true
    case :other
    when :finish
      break
    else
      nil
    end
    return :returned
  end
  :after
end

def while_case_literal_later_match_then_return
  while true
    case :finish
    when :other
      return :unreachable
    when :finish
      break
    end
    return :unreachable
  end
  :after
end

def while_case_without_predicate_break_then_return
  while true
    case
    when false
      break
    when true
      break
    end
    return :unreachable
  end
  :after
end

def while_case_integer_float_match_then_return
  while true
    case 1
    when 1.0
      break
    else
      nil
    end
    return :unreachable
  end
  :after
end

def while_case_integer_range_match_then_return
  while true
    case 2
    when 1..3
      break
    else
      return :unreachable
    end
    return :unreachable_after
  end
  :after
end

def while_case_integer_range_miss_then_return
  while true
    case 4
    when 1...4
      break
    else
      return :returned
    end
    return :unreachable_after
  end
  :after
end

def while_static_branch_break_value
  while true
    if true
      break :selected
    else
      break :unreachable
    end
  end
end

def while_short_circuit_break_value
  while true
    true && break
  end
end

def for_static_branch_break_value
  for value in [:item]
    if true
      break :selected
    else
      break :unreachable
    end
  end
end

def while_unknown_condition_break_then_return(flag)
  while true
    if flag
      break
    end
    return :returned
  end
  :after
end

def while_unknown_and_break_then_return(flag)
  while true
    flag && break
    return :returned
  end
  :after
end

def while_unknown_or_break_then_return(flag)
  while true
    flag || break
    return :returned
  end
  :after
end

def while_unknown_and_return_then_break(flag)
  while true
    return :returned if flag
    break :after
  end
end

def while_unknown_or_return_then_break(flag)
  while true
    return :returned unless flag
    break :after
  end
end

def while_false_continues
  while false
    return :unreachable
  end
  :reachable
end

def until_true_continues
  until true
    return :unreachable
  end
  :reachable
end

def for_nonempty_array_returns
  for item in [1]
    return :returned
  end
  :unreachable
end

def for_empty_array_continues
  for item in []
    return :unreachable
  end
  :reachable
end

def for_nonempty_hash_returns
  for item in {key: 1}
    return :returned
  end
  :unreachable
end

def for_unknown_splat_continues(values)
  for item in [*values]
    return :returned
  end
  :reachable
end
```

### result

```rbs
class Object < BasicObject
  def loop_returns: -> :returned
  def loop_returns_from_begin: -> :returned
  def while_true_returns_from_branches: -> :first | :second
  def while_true_returns_from_ternary: -> :first | :second
  def begin_modifier_returns: -> :returned
  def loop_break_continues: -> :after
  def while_true_returns: -> :returned
  def until_false_returns: -> :returned
  def while_truthy_literal_returns: -> :returned
  def until_nil_returns: -> :returned
  def while_parenthesized_true_returns: -> :returned
  def while_return_before_unreachable: -> :returned
  def while_break_before_return_continues: -> :after | :returned
  def loop_break_before_return_continues: -> :after | :returned
  def while_modifier_break_or_return: -> :after | :returned
  def while_short_circuit_break_or_return: -> :after | :returned
  def while_short_circuit_or_break_or_return: -> :after | :returned
  def while_false_and_break_then_return: -> :returned
  def while_true_or_break_then_return: -> :returned
  def while_true_and_break_returns_after: -> :after
  def while_false_or_break_returns_after: -> :after
  def while_if_true_break_then_return: -> :after
  def while_if_true_break_skips_else_return: -> :after
  def while_if_false_break_skips_then_return: -> :after
  def while_elsif_literal_break_skips_dead_returns: -> :after
  def while_if_false_break_then_return: -> :returned
  def while_if_not_false_break_then_return: -> :after
  def while_unless_false_break_then_return: -> :after
  def while_unless_false_break_skips_else_return: -> :after
  def while_unless_true_break_then_return: -> :returned
  def while_ternary_true_break_then_return: -> :after
  def while_ternary_false_break_then_return: -> :returned
  def while_case_literal_break_then_return: -> :after
  def while_case_literal_miss_then_return: -> :returned
  def while_case_literal_later_match_then_return: -> :after
  def while_case_without_predicate_break_then_return: -> :after
  def while_case_integer_float_match_then_return: -> :after
  def while_case_integer_range_match_then_return: -> :after
  def while_case_integer_range_miss_then_return: -> :returned
  def while_static_branch_break_value: -> :selected
  def while_short_circuit_break_value: -> nil
  def for_static_branch_break_value: -> :selected
  def while_unknown_condition_break_then_return: (untyped flag) -> (:after | :returned)
  def while_unknown_and_break_then_return: (untyped flag) -> (:after | :returned)
  def while_unknown_or_break_then_return: (untyped flag) -> (:after | :returned)
  def while_unknown_and_return_then_break: (untyped flag) -> (:after | :returned)
  def while_unknown_or_return_then_break: (untyped flag) -> (:after | :returned)
  def while_false_continues: -> :reachable
  def until_true_continues: -> :reachable
  def for_nonempty_array_returns: -> :returned
  def for_empty_array_continues: -> :reachable
  def for_nonempty_hash_returns: -> :returned
  def for_unknown_splat_continues: (untyped values) -> (:reachable | :returned)
end
```

## for uses source-defined Array#each

```yaml
known_issue: true
```

```ruby
class Array
  def each
    yield :item
  end
end

class ForEachOverride
  def result
    for value in []
      return value
    end
    :after
  end
end
```

### result

```rbs
class Array
  def each: { (Symbol) -> untyped } -> untyped
end

class ForEachOverride
  def result: -> :item
end
```

## for uses source-defined Hash#each

```yaml
known_issue: true
```

```ruby
class Hash
  def each
    yield :item
  end
end

class ForHashEachOverride
  def result
    for value in {}
      return value
    end
    :after
  end
end
```

### result

```rbs
class ForHashEachOverride
  def result: -> :item
end

class Hash
  def each: { (Symbol) -> untyped } -> untyped
end
```

## for uses prepended source-defined Array#each

```yaml
known_issue: true
```

```ruby
module ForEachPrepend
  def each
    yield :item
  end
end

Array.prepend(ForEachPrepend)

class ForEachPrepended
  def result
    for value in []
      return value
    end
    :after
  end
end
```

### result

```rbs
class Array[E]
  prepend ForEachPrepend
end

module ForEachPrepend
  def each: { (Symbol) -> untyped } -> untyped
end

class ForEachPrepended
  def result: -> :item
end
```
