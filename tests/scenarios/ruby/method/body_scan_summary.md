# Ruby / Method / Body Scan Summary

## String-like call alone does not narrow arg to String

### update

```ruby
def foo(x, y)
  y.strip
  x
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x, untyped y) -> untyped
end
```

## String-like chain in explicit return does not force String

### update

```ruby
def foo(x)
  return x.strip
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> untyped
end
```

## Last expression through begin does not narrow from string-like call

### update

```ruby
def foo(x)
  begin
    x.strip[0, 1]
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> untyped
end
```

## String-like call inside if does not narrow arg

### update

```ruby
def foo(x, flag)
  if flag
    x.strip
  end
  x
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x, untyped flag) -> untyped
end
```

## String-like call inside unless else does not narrow arg

### update

```ruby
def foo(x, flag)
  unless flag
    nil
  else
    x.strip
  end
  x
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x, untyped flag) -> untyped
end
```

## Keep yield arg type inside begin after summary

### update

```ruby
class A
  def foo
    begin
      yield 1
    end
  end

  def bar = foo { |x| x + 1 }
end
```

### result

```rbs
class A
  def foo: { (Integer) -> Integer } -> Integer
  def bar: -> Integer
end
```

## Keep local type for yield arg after summary

### update

```ruby
class A
  def foo
    x = A.new
    yield x if block_given?
  end

  def bar = foo { |x| x }
end
```

### result

```rbs
class A
  def foo: { (A) -> A } -> A?
  def bar: -> A?
end
```
