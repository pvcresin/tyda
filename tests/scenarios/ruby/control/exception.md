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
class Object
  def raise_test: -> bot
end
```

## Multiple rescue clauses

### update

```ruby
def multi_rescue
  begin
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
class Object
  def multi_rescue: -> 1 | "arg_error" | "type_error"
end
```

## A return inside rescue contributes to the method type

### update

```ruby
class Loader
  def value_or_nil
    "loaded"
  rescue
    return nil
  end

  def with_fallback
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

### update

```ruby
def rescue_var
  begin
    1 / 0
  rescue => e
    e
  end
end
```

### result

```rbs
class Object
  def rescue_var: -> Integer | StandardError
end
```

## rescue narrows by exception class

### update

```ruby
def rescue_specific
  begin
    1 / 0
  rescue ArgumentError => e
    e
  end
end
```

### result

```rbs
class Object
  def rescue_specific: -> Integer | ArgumentError
end
```

## rescue narrows by multiple exception classes

### update

```ruby
def rescue_multiple_specific
  begin
    1 / 0
  rescue ArgumentError, TypeError => e
    e
  end
end
```

### result

```rbs
class Object
  def rescue_multiple_specific: -> Integer | ArgumentError | TypeError
end
```

## rescue narrows by splat exception classes

### update

```ruby
ERROR_CLASSES = [ArgumentError, TypeError]

def rescue_splat_specific
  begin
    1 / 0
  rescue *ERROR_CLASSES => e
    e
  end
end
```

### result

```rbs
ERROR_CLASSES: [singleton(ArgumentError), singleton(TypeError)]

class Object
  def rescue_splat_specific: -> Integer | ArgumentError | TypeError
end
```

## rescue reference variable stays nilable later

### update

```ruby
def rescue_var_after
  begin
    1
  rescue => e
    e
  end

  e
end
```

### result

```rbs
class Object
  def rescue_var_after: -> StandardError?
end
```

## rescue reference variable unions with existing local

### update

```ruby
def rescue_var_after_existing
  e = :before

  begin
    1
  rescue ArgumentError => e
    e
  end

  e
end
```

### result

```rbs
class Object
  def rescue_var_after_existing: -> :before | ArgumentError
end
```

## Method-level ensure keeps explicit return types

### update

```ruby
def explicit_return_with_ensure
  return 1
ensure
  cleanup
end

def conditional_return_with_ensure(flag)
  return "early" if flag
  42
ensure
  cleanup
end
```

### result

```rbs
class Object
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
class Object
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
class Object
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
class Object
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
class Object
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
class Object
  def rescue_modifier: -> :fallback
end
```

## Modifier rescue unions success and rescue expressions

### update

```ruby
def rescue_modifier_union
  1 rescue :fallback
end
```

### result

```rbs
class Object
  def rescue_modifier_union: -> 1 | :fallback
end
```

## retry

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
class Object
  def with_retry: -> "failed" | "success"
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
class Object
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
class Object
  def retry_without_value: -> :handled
end
```

## Rescue splat unions the clause literals

### update

```ruby
def foo
  begin
    :a
  rescue *[StandardError]
    :b
  end
end
```

### result

```rbs
class Object
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
class Object
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
class Object
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
class Object
  def foo: -> 1 | "str"
end
```
