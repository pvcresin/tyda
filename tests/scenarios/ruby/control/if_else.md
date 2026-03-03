# Ruby / Control / If Else

## if else returns different types

### update

```ruby
def foo(x)
  if x
    1
  else
    "hello"
  end
end
```

### result

```rbs
class Object
  def foo: (untyped x) -> (1 | "hello")
end
```

## if else returns same type

### update

```ruby
def foo(x)
  if x
    1
  else
    2
  end
end
```

### result

```rbs
class Object
  def foo: (untyped x) -> (1 | 2)
end
```

## if without else unions with nil

### update

```ruby
def foo(x)
  if x
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
