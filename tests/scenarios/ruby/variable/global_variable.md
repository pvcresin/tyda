# Ruby / Variable / Global Variable

## Built-in global variables

### update

```ruby
def use_globals = $stdout
```

### result

```rbs
class Object
  def use_globals: -> IO
end
```

## File::Stat method through `$stdout.stat`

### update

```ruby
def stat_dev = $stdout.stat.dev

def stat_ino = $stderr.stat.ino
```

### result

```rbs
class Object
  def stat_dev: -> Integer
  def stat_ino: -> Integer
end
```

## Custom global variables

### update

```ruby
def custom_global
  $custom = "value"
  $custom
end
```

### result

```rbs
class Object
  def custom_global: -> "value"
end
```

## Compact global assignment in eval argument

### update

```ruby
eval$s=%w'"ok"'.join

def compact_global_source = $s
```

### result

```rbs
class Object
  def compact_global_source: -> "\"ok\""
end
```

## Cross-method read of a top-level global

### update

```ruby
$config = 1

def read_config
  $config
end
```

### result

```rbs
class Object
  def read_config: -> 1
end
```

## Forward reference: method defined before the global write

### update

```ruby
def read_late
  $late
end

$late = "ready"
```

### result

```rbs
class Object
  def read_late: -> "ready"
end
```

## Intra-method op-assign stays flow-sensitive

### update

```ruby
def toggle
  $state = true
  $state &&= "yes"
  $state
end
```

### result

```rbs
class Object
  def toggle: -> "yes"
end
```

## Global variable alias shares the original value

### update

```ruby
$foo = 123
alias $bar $foo

def f = $bar
```

### result

```rbs
class Object
  def f: -> 123
end
```

## defined? global splits both branches

### update

```ruby
def foo
  if defined?($x)
    1
  else
    "str"
  end
end
```

### result

```rbs
class Object
  def foo: -> 1 | "str"
end
```
