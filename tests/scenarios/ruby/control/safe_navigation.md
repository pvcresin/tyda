# Ruby / Control / Safe Navigation

## Basic safe navigation call

### update

```ruby
class String
  def upcase = "HELLO"
end

def foo(s) = s&.upcase
foo("hello")
```

### result

```rbs
class Object < BasicObject
  def foo: (String s) -> "HELLO"
end

class String
  def upcase: -> "HELLO"
end
```

## Method chain with safe navigation

### update

```ruby
def bar(x) = x&.to_s
bar(42)
```

```rbs
class Integer
  def to_s: -> String
end
```

### result

```rbs
class Object < BasicObject
  def bar: (Integer x) -> String
end
```

## Nilable receiver keeps nil in result

### update

```ruby
class Widget
  def name = "w"
end

def foo(w) = w&.name
foo(nil)
foo(Widget.new)
```

### result

```rbs
class Object < BasicObject
  def foo: (Widget? w) -> "w"?
end

class Widget
  def name: -> "w"
end
```

## Plain dot call does not add nil

### update

```ruby
def baz(x) = x.to_s
baz(42)
```

```rbs
class Integer
  def to_s: -> String
end
```

### result

```rbs
class Object < BasicObject
  def baz: (Integer x) -> String
end
```

## Safe-nav truthy edge narrows the receiver for a later call

### update

```ruby
def bare(flag)
  v = flag ? "[x]" : nil
  return "" unless v&.length
  v.upcase
end
```

### result

```rbs
class Object < BasicObject
  def bare: (untyped flag) -> String
end
```

## Safe-nav truthy edge plus AND RHS uses the narrowed receiver

### update

```ruby
def in_and(flag)
  v = flag ? "[x]" : nil
  return 0 unless v&.start_with?("[") && v.end_with?("]")
  v.length
end
```

### result

```rbs
class Object < BasicObject
  def in_and: (untyped flag) -> (0 | 3)
end
```
