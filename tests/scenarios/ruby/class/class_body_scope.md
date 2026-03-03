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
