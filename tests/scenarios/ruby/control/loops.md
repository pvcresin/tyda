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
  def double_three_times: -> Float | 1
end
```

## Loop-local first assignment is nilable if the loop may not run

### update

```ruby
def maybe_fresh
  counter = 0
  while counter < 2
    fresh = counter * 2
    counter += 1
  end
  fresh
end
```

### result

```rbs
class Object < BasicObject
  def maybe_fresh: -> Integer?
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
  def check: -> 1 | [:a, :b, :c]
end
```
