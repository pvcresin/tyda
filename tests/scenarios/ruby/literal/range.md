# Ruby / Literal / Range

## Range literal

### update

```ruby
def range_test
  a = 1..10
  b = 1...10
  c = 'a'..'z'
  a
end
```

### result

```rbs
class Object < BasicObject
  def range_test: -> Range[Integer]
end
```

## beginless / endless range

### update

```ruby
def foo
  0..1
end

def bar
  0..
end

def baz
  ..1
end

def qux
  nil..nil
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> Range[Integer]
  def bar: -> Range[Integer]
  def baz: -> Range[Integer]
  def qux: -> Range[nil]
end
```
