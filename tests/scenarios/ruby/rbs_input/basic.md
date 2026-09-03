# Ruby / RBS Input / Basic

## Track method return from external RBS

### update

```rbs
class String
  def to_i: -> Integer
end
```

```ruby
def parse(s) = s.to_i
parse("42")
```

### result

```rbs
class Object < BasicObject
  def parse: (String s) -> Integer
end
```

## Track Array methods from external RBS

### update

```rbs
class Array
  def length: -> Integer
end
```

```ruby
def count(arr) = arr.length
count([1, 2, 3])
```

### result

```rbs
class Object < BasicObject
  def count: (Array[Integer] arr) -> Integer
end
```

## Method without RBS stays untyped

### update

```rbs
class String
  def to_i: -> Integer
end
```

```ruby
def process(s) = s.upcase
process("hello")
```

### result

```rbs
class Object < BasicObject
  def process: (String s) -> String
end
```

## Class defined in RBS is not emitted

### update

```rbs
class Integer
  def to_s: -> String
end
```

```ruby
def stringify(n) = n.to_s
stringify(42)
```

### result

```rbs
class Object < BasicObject
  def stringify: (Integer n) -> String
end
```
