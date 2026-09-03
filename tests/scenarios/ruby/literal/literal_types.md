# Ruby / Literal / Literal Types

## Symbol literal

### update

```ruby
def status = :ok
```

### result

```rbs
class Object < BasicObject
  def status: -> :ok
end
```

## Union of symbol literals

### update

```ruby
def state(cond) = cond ? :ok : :error
```

### result

```rbs
class Object < BasicObject
  def state: (untyped cond) -> (:error | :ok)
end
```

## true literal

### update

```ruby
def enabled = true
```

### result

```rbs
class Object < BasicObject
  def enabled: -> true
end
```

## Union of true and false

### update

```ruby
def flag(cond) = cond ? true : false
```

### result

```rbs
class Object < BasicObject
  def flag: (untyped cond) -> bool
end
```

## Integer literal

### update

```ruby
def answer = 42
```

### result

```rbs
class Object < BasicObject
  def answer: -> 42
end
```

## String literal

### update

```ruby
def greeting = "hello"
```

### result

```rbs
class Object < BasicObject
  def greeting: -> "hello"
end
```

## Float literal

### update

```ruby
def pi = 3.14
```

### result

```rbs
class Object < BasicObject
  def pi: -> 3.14
end
```

## Mixed union of Symbol and Integer

### update

```ruby
def mixed(cond) = cond ? :ok : 42
```

### result

```rbs
class Object < BasicObject
  def mixed: (untyped cond) -> (42 | :ok)
end
```

## Symbol branches with multiple returns

### update

```ruby
def classify(x)
  if x > 0
    :positive
  elsif x < 0
    :negative
  else
    :zero
  end
end
```

### result

```rbs
class Object < BasicObject
  def classify: (untyped x) -> (:negative | :positive | :zero)
end
```

## Resolve methods on literal type

### update

```ruby
def literal_method = 42.to_s
```

### result

```rbs
class Object < BasicObject
  def literal_method: -> String
end
```

## Resolve methods on literal type union

### update

```ruby
def union_method
  arr = [1, 2, 3]
  arr.map { |x| x.to_s }
end
```

### result

```rbs
class Object < BasicObject
  def union_method: -> Array[String]
end
```

## String interpolation returns base type

### update

```ruby
def interpolated(name) = "Hello, #{name}"
```

### result

```rbs
class Object < BasicObject
  def interpolated: (untyped name) -> String
end
```

## Tuple type from array literal

### update

```ruby
def tuple_test = [1, "hello", :ok]
```

### result

```rbs
class Object < BasicObject
  def tuple_test: -> [1, "hello", :ok]
end
```

## Record type with symbol keys

### update

```ruby
def record_symbol = { name: "Alice", age: 30 }
```

### result

```rbs
class Object < BasicObject
  def record_symbol: -> { name: "Alice", age: 30 }
end
```

## Record type with string keys

### update

```ruby
def record_string = { "name" => "Alice", "age" => 30 }
```

### result

```rbs
class Object < BasicObject
  def record_string: -> { "name" => "Alice", "age" => 30 }
end
```

## Record type with mixed keys

### update

```ruby
def record_mixed = { name: "Alice", "age" => 30 }
```

### result

```rbs
class Object < BasicObject
  def record_mixed: -> { name: "Alice", "age" => 30 }
end
```

## Convert Tuple to Array with <<

### update

```ruby
def push_test
  arr = []
  arr << 1
  arr << "hello"
  arr
end
```

### result

```rbs
class Object < BasicObject
  def push_test: -> [1, "hello"]
end
```

## Subsumption from Integer or 1 to Integer

### update

```ruby
def subsume_test(cond) = cond ? 1 : 1 + 2
```

### result

```rbs
class Object < BasicObject
  def subsume_test: (untyped cond) -> Integer
end
```

## Widen parameter type

### update

```ruby
def param_widen(x) = x.to_s
param_widen(42)
```

### result

```rbs
class Object < BasicObject
  def param_widen: (Integer x) -> String
end
```

## Literal type in RBS comment

### update

```ruby
class Config
  #: -> :production
  def env = :production

  #: -> { host: String, "port" => Integer }
  def connection = { host: "localhost", "port" => 3000 }
end
```

### result

```rbs
class Config
  def env: -> :production
  def connection: -> { host: String, "port" => Integer }
end
```
