# Ruby / Class / Class Body Scope

## `class C` creates a new scope

### update

```ruby
outer_val = "x"

class A
  def f = outer_val
end
```

### result

```rbs
class A
  def f: -> untyped
end
```

## `def f` does not capture class body locals

### update

```ruby
class A
  body_local = "x"

  def f = body_local
end
```

### result

```rbs
class A
  def f: -> untyped
end
```

## Yielded block defines a method in the enclosing class body

### update

```ruby
class Module
  def install_generic(method)
    yield unless method_defined?(method)
  end

  def ignore_generic
    nil
  end
end

class ClassBodyYield
  install_generic(:generated) do
    def generated = "generated"
  end

  ignore_generic do
    def ignored = "ignored"
  end
end

def read_generated = ClassBodyYield.new.generated
```

### result

```rbs
class ClassBodyYield
  def generated: -> "generated"
end

class Module
  def install_generic: (untyped method) { () -> untyped } -> (nil | untyped)
  def ignore_generic: -> nil
end

class Object < BasicObject
  def read_generated: -> "generated"
end
```
