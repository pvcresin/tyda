# Ruby / Method / Backward Propagation

## Call element method inside block over parameter array

### update

```ruby
def process(arr)
  arr.map { |x| x.upcase }
end
process(["hello", "world"])
```

### result

```rbs
class Object
  def process: (Array[String] arr) -> Array[String]
end
```

## Chain after block result

### update

```ruby
def upcased_lengths(arr)
  arr.map { |x| x.upcase }.map { |s| s.length }
end
upcased_lengths(["hi", "bye"])
```

### result

```rbs
class Object
  def upcased_lengths: (Array[String] arr) -> Array[Integer]
end
```

## select + first

### update

```ruby
def first_long(arr)
  arr.select { |x| x.length > 3 }.first
end
first_long(["hi", "hello", "world"])
```

### result

```rbs
class Object
  def first_long: (Array[String] arr) -> String?
end
```

## Expand block from Hash#values

### update

```ruby
def double_values(h)
  h.values.map { |v| v * 2 }
end
double_values({ a: 1, b: 2 })
```

### result

```rbs
class Object
  def double_values: ({ a: Integer, b: Integer } h) -> Array[Integer]
end
```

## Nested map blocks

### update

```ruby
def nested(arr)
  arr.map { |row| row.map { |x| x.upcase } }
end
nested([["a", "b"], ["c", "d"]])
```

### result

```rbs
class Object
  def nested: (Array[Array[String]] arr) -> Array[Array[String]]
end
```

## Update accumulator with each_with_object

### update

```ruby
def collect_lengths(arr)
  arr.each_with_object([]) { |s, acc| acc << s.length }
end
collect_lengths(["hi", "hello"])
```

### result

```rbs
class Object
  def collect_lengths: (Array[String] arr) -> Array[Integer]
end
```

## Propagate union from multiple call sites

### update

```ruby
def shout(arr)
  arr.map { |x| x.upcase }
end
shout(["hello"])
shout([:sym])
```

### result

```rbs
class Object
  def shout: (Array[String | Symbol] arr) -> Array[String | Symbol]
end
```
