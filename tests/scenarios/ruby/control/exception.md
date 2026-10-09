# Ruby / Control / Exception

## raise

### update

```ruby
def raise_test
  raise "error"
end
```

### result

```rbs
class Object < BasicObject
  def raise_test: -> bot
end
```

## Multiple rescue clauses

### update

```ruby
def multi_rescue
  begin
    raise ArgumentError if [true, false].sample
    raise TypeError if [true, false].sample
    1
  rescue ArgumentError
    "arg_error"
  rescue TypeError
    "type_error"
  end
end
```

### result

```rbs
class Object < BasicObject
  def multi_rescue: -> 1 | "arg_error" | "type_error"
end
```

## A return inside rescue contributes to the method type

### update

```ruby
class Loader
  def value_or_nil
    raise "unavailable" if [true, false].sample
    "loaded"
  rescue
    return nil
  end

  def with_fallback
    raise "unavailable" if [true, false].sample
    return 1
  rescue
    return "error"
  end
end
```

### result

```rbs
class Loader
  def value_or_nil: -> "loaded"?
  def with_fallback: -> 1 | "error"
end
```

## rescue => e

```yaml
known_issue: true
```

### update

```ruby
def rescue_var
  begin
    raise ZeroDivisionError if [true, false].sample
    :normal
  rescue => e
    e
  end
end
```

### result

```rbs
class Object < BasicObject
  def rescue_var: -> (:normal | ZeroDivisionError)
end
```

## rescue narrows by exception class

### update

```ruby
def rescue_specific
  begin
    raise ArgumentError if [true, false].sample
    1
  rescue ArgumentError => e
    e
  end
end
```

### result

```rbs
class Object < BasicObject
  def rescue_specific: -> 1 | ArgumentError
end
```

## rescue narrows by multiple exception classes

### update

```ruby
def rescue_multiple_specific
  begin
    raise ArgumentError if [true, false].sample
    raise TypeError if [true, false].sample
    1
  rescue ArgumentError, TypeError => e
    e
  end
end
```

### result

```rbs
class Object < BasicObject
  def rescue_multiple_specific: -> 1 | ArgumentError | TypeError
end
```

## rescue narrows by splat exception classes

### update

```ruby
ERROR_CLASSES = [ArgumentError, TypeError]

def rescue_splat_specific
  begin
    raise ERROR_CLASSES.sample if [true, false].sample
    1
  rescue *ERROR_CLASSES => e
    e
  end
end
```

### result

```rbs
ERROR_CLASSES: [singleton(ArgumentError), singleton(TypeError)]

class Object < BasicObject
  def rescue_splat_specific: -> 1 | ArgumentError | TypeError
end
```

## rescue reference variable stays nilable later

### update

```ruby
def rescue_var_after
  begin
    raise StandardError if [true, false].sample
    1
  rescue => e
    e
  end

  e
end
```

### result

```rbs
class Object < BasicObject
  def rescue_var_after: -> StandardError?
end
```

## rescue reference variable unions with existing local

### update

```ruby
def rescue_var_after_existing
  e = :before

  begin
    raise ArgumentError if [true, false].sample
    1
  rescue ArgumentError => e
    e
  end

  e
end
```

### result

```rbs
class Object < BasicObject
  def rescue_var_after_existing: -> :before | ArgumentError
end
```

## Method-level ensure keeps explicit return types

### update

```ruby
def explicit_return_with_ensure
  return 1
ensure
  nil
end

def conditional_return_with_ensure(flag)
  return "early" if flag
  42
ensure
  nil
end
```

### result

```rbs
class Object < BasicObject
  def explicit_return_with_ensure: -> 1
  def conditional_return_with_ensure: (untyped flag) -> (42 | "early")
end
```

## Method-level ensure return overrides the body

### update

```ruby
def ensure_return_wins
  return 1
ensure
  return 2
end

def ensure_return_over_value
  "value"
ensure
  return :overridden
end
```

### result

```rbs
class Object < BasicObject
  def ensure_return_wins: -> 2
  def ensure_return_over_value: -> :overridden
end
```

## ensure block

### update

```ruby
def with_ensure
  begin
    42
  ensure
    "cleanup"
  end
end
```

### result

```rbs
class Object < BasicObject
  def with_ensure: -> 42
end
```

## ensure return overrides begin rescue return

### update

```ruby
def ensure_return_overrides
  begin
    1
  rescue
    2
  ensure
    return 3
  end
end
```

### result

```rbs
class Object < BasicObject
  def ensure_return_overrides: -> 3
end
```

## ensure raise removes begin rescue return

### update

```ruby
def ensure_raise_overrides
  begin
    1
  rescue
    2
  ensure
    raise "boom"
  end
end
```

### result

```rbs
class Object < BasicObject
  def ensure_raise_overrides: -> bot
end
```

## modifier rescue

### update

```ruby
def rescue_modifier
  raise "boom" rescue :fallback
end
```

### result

```rbs
class Object < BasicObject
  def rescue_modifier: -> :fallback
end
```

## Modifier rescue unions success and rescue expressions

### update

```ruby
def rescue_modifier_union
  ([true, false].sample ? raise("boom") : 1) rescue :fallback
end
```

### result

```rbs
class Object < BasicObject
  def rescue_modifier_union: -> 1 | :fallback
end
```

## retry

```yaml
known_issue: true
```

### update

```ruby
def with_retry
  attempts = 0
  begin
    attempts += 1
    raise "fail" if attempts < 3
    "success"
  rescue
    retry if attempts < 3
    "failed"
  end
end
```

### result

```rbs
class Object < BasicObject
  def with_retry: -> "success"
end
```

## Modifier rescue at method level

### update

```ruby
def foo
  if rand > 0.5
    raise
  end
rescue
  1
end

def bar
  raise
rescue
  1
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> 1?
  def bar: -> 1
end
```

## retry does not affect return value

### update

```ruby
def retry_without_value
  begin
    raise "fail"
  rescue
    retry if false
    :handled
  end
end
```

### result

```rbs
class Object < BasicObject
  def retry_without_value: -> :handled
end
```

## Rescue splat unions the clause literals

### update

```ruby
def foo
  begin
    raise StandardError if [true, false].sample
    :a
  rescue *[StandardError]
    :b
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> :a | :b
end
```

## Guard raise then rescue message

### update

```ruby
def foo(n)
  raise if n != 0
  n.to_s
rescue StandardError => e
  e.message
end

foo(1)
```

### result

```rbs
class Object < BasicObject
  def foo: (Integer n) -> String
end
```

## Begin rescue else tracks local per clause

### update

```ruby
def rescue_path
  x = :a
  begin
    x = :b
    raise
    x = :c
  rescue
    x
  end
end

def else_path
  x = :a
  begin
    x = :b
    raise if [true, false].sample
    x = :c
  rescue
    x = :d
  else
    x
  end
end

def after_path(flag)
  x = :a
  begin
    x = :b
    raise if flag
    x = :c
  rescue
    x = :d
  else
    x = :e
  end
  x
end

after_path(true)
after_path(false)
```

### result

```rbs
class Object < BasicObject
  def rescue_path: -> :b
  def else_path: -> :c | :d
  def after_path: (bool flag) -> (:d | :e)
end
```

## Retry unions the begin return across re-entry

### update

```ruby
def foo
  n = 1
  begin
    raise if rand < 0.5
    n
  rescue
    n = "str"
    retry
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> 1 | "str"
end
```

## Ignore code after a terminating begin body

### update

```ruby
def begin_return_only
  begin
    return :returned
  end
  :unreachable
end

def begin_ensure_return_only
  begin
    return :returned
  ensure
    :ensured
  end
  :unreachable
end

def begin_return_with_rescue
  begin
    return :returned
  rescue
    :rescued
  end
  :unreachable
end

def begin_raise_rescue_continues
  begin
    raise
  rescue
    :rescued
  end
  :reachable
end

def rescue_modifier_return
  return :returned rescue :rescued
  :unreachable
end

def rescue_modifier_raise_continues
  raise rescue :rescued
  :reachable
end

def begin_return_after_possible_raise(flag)
  begin
    raise if flag
    return :returned
  rescue
    :rescued
  end
  :after
end
```

### result

```rbs
class Object < BasicObject
  def begin_return_only: -> :returned
  def begin_ensure_return_only: -> :returned
  def begin_return_with_rescue: -> :returned
  def begin_raise_rescue_continues: -> :reachable
  def rescue_modifier_return: -> :returned
  def rescue_modifier_raise_continues: -> :reachable
  def begin_return_after_possible_raise: (untyped flag) -> (:after | :returned)
end
```

## Exiting rescue branches do not widen fallthrough locals

```ruby
def exiting_rescue_does_not_widen_local(flag)
  value = :before
  begin
    if flag
      value = :normal
    else
      value = :before_raise
      raise
    end
  rescue
    value = :rescued
    raise
  end
  value
end
```

### result

```rbs
class Object < BasicObject
  def exiting_rescue_does_not_widen_local: (untyped flag) -> :normal
end
```

## Source-defined control-flow method names remain ordinary calls

```yaml
known_issue: true
```

### update

```ruby
class ControlFlowMethodOverrides
  def raise
    :raised
  end

  def fail
    :failed
  end

  def exit
    :exited
  end

  def abort
    :aborted
  end

  def loop
    :looped
  end

  def with_yield
    yield
  end

  def no_yield
    :ignored
  end

  def bare_calls
    raise
    fail
    exit
    abort
    :after_calls
  end

  def explicit_calls
    self.raise
    self.fail
    self.exit
    self.abort
    :after_calls
  end

  def loop_call
    loop { :from_block }
    :after_loop
  end

  def loop_nonlocal_return
    loop { return :from_block }
    :after_loop
  end

  def no_yield_nonlocal_return
    no_yield { return :from_block }
    :after_no_yield
  end

  def explicit_no_yield_nonlocal_return
    self.no_yield { return :from_block }
    :after_no_yield
  end

  def yielding_nonlocal_return
    with_yield { return :from_block }
    :after_yield
  end
end
```

### result

```rbs
class ControlFlowMethodOverrides
  def raise: -> :raised
  def fail: -> :failed
  def exit: -> :exited
  def abort: -> :aborted
  def loop: -> :looped
  def with_yield: -> bot
  def no_yield: -> :ignored
  def bare_calls: -> :after_calls
  def explicit_calls: -> :after_calls
  def loop_call: -> :after_loop
  def loop_nonlocal_return: -> :after_loop
  def no_yield_nonlocal_return: -> :after_no_yield
  def explicit_no_yield_nonlocal_return: -> :after_no_yield
  def yielding_nonlocal_return: -> :from_block
end
```
