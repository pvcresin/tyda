# Ruby / Control / Edge Cases

## String interpolation

### update

```ruby
def greet(name) = "hello #{name}"
```

### result

```rbs
class Object
  def greet: (untyped name) -> String
end
```

## unless statement

### update

```ruby
def foo(x)
  unless x
    1
  end
end
```

### result

```rbs
class Object
  def foo: (untyped x) -> 1?
end
```

## unless else statement

### update

```ruby
def foo(x)
  unless x
    "no"
  else
    "yes"
  end
end
```

### result

```rbs
class Object
  def foo: (untyped x) -> ("no" | "yes")
end
```

## Module definition

### update

```ruby
module Helpers
  def foo = 42
end
```

### result

```rbs
module Helpers
  def foo: -> 42
end
```

## self reference

### update

```ruby
class Foo
  def bar = self
end
```

### result

```rbs
class Foo
  def bar: -> Foo
end
```

## Multi-line side effects with last expression return

### update

```ruby
def foo
  puts "hello"
  1
end
```

### result

```rbs
class Object
  def foo: -> 1
end
```

## Assign variable across lines and return it

### update

```ruby
def foo
  x = 42
  y = "hello"
  x
end
```

### result

```rbs
class Object
  def foo: -> 42
end
```

## Symbol interpolation

### update

```ruby
def foo = :"hello_#{42}"
```

### result

```rbs
class Object
  def foo: -> :hello_42
end
```

## Use defined? as branch condition

### update

```ruby
def foo
  if defined?($x)
    1
  else
    "x"
  end
end
```

### result

```rbs
class Object
  def foo: -> 1 | "x"
end
```

## Return defined? as expression

### update

```ruby
def defined_literal
  defined?(1)
end

def defined_local
  x = 1
  defined?(x)
end

def defined_constant_path
  defined?(Some::CONSTANT)
end

def defined_nested
  defined?(defined?(missing_method))
end
```

### result

```rbs
class Object
  def defined_literal: -> String?
  def defined_local: -> String?
  def defined_constant_path: -> String?
  def defined_nested: -> String?
end
```
