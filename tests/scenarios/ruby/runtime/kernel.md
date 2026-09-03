# Ruby / Runtime / Kernel

## Kernel string formatters return String

### update

```ruby
class Probe
  def via_format   = format("%d", 42)
  def via_sprintf  = sprintf("%d items", 3)
end
```

### result

```rbs
class Probe
  def via_format: -> String
  def via_sprintf: -> String
end
```

## Kernel conversion functions

### update

```ruby
class Probe
  def to_string   = String(42)
  def to_integer  = Integer("42")
  def to_float    = Float("1.5")
end
```

### result

```rbs
class Probe
  def to_string: -> String
  def to_integer: -> 42
  def to_float: -> Float
end
```

## Kernel literal integer conversion

### update

```ruby
class Probe
  def adjacent_integer = Integer("2" "4")
  def binary_integer = Integer("100", 2)
  def float_integer = Integer(-1.9)
end
```

### result

```rbs
class Probe
  def adjacent_integer: -> 24
  def binary_integer: -> 4
  def float_integer: -> -1
end
```

## Kernel debug output returns arguments

### update

```ruby
class Probe
  def p_single = p("value")
  def p_many = p("left", "right")
  def pp_single = pp(:value)
  def display_return = "value".display
  def splat_non_array = p(*11)
end
```

### result

```rbs
class Probe
  def p_single: -> "value"
  def p_many: -> ["left", "right"]
  def pp_single: -> :value
  def display_return: -> nil
  def splat_non_array: -> 11
end
```

## Kernel output and reflection helpers

### update

```ruby
class Probe
  def puts_return = puts("value")
  def print_return = print("value")
  def printf_return = printf("%s", "value")
  def putc_return = putc(65)
  def warn_return = warn("value")
  def at_exit_return = at_exit { "done" }
  def global_names = global_variables

  def local_names
    value = 1
    local_variables
  end
end
```

### result

```rbs
class Probe
  def puts_return: -> nil
  def print_return: -> nil
  def printf_return: -> nil
  def putc_return: -> 65
  def warn_return: -> nil
  def at_exit_return: -> Proc
  def global_names: -> Array[Symbol]
  def local_names: -> Array[Symbol]
end
```

## Kernel proc and lambda builders

### update

```ruby
class Probe
  def via_proc    = proc { |x| x.to_s }
  def via_lambda  = lambda { |x| x.to_s }
end
```

### result

```rbs
class Probe
  def via_proc: -> Proc
  def via_lambda: -> Proc
end
```

## Kernel Array wraps a literal array

### update

```ruby
def ids = Array([1, 2, 3])
```

### result

```rbs
class Object < BasicObject
  def ids: -> Array[1 | 2 | 3]
end
```
