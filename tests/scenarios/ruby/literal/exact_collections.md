# Ruby / Literal / Exact Collections

## Keep array literals exact

### update

```ruby
def pair = [1, 2]
```

### result

```rbs
class Object
  def pair: -> [1, 2]
end
```

## Apply push through local alias

### update

```ruby
def push_alias
  xs = [1, 2]
  ys = xs
  ys << 3
  xs
end
```

### result

```rbs
class Object
  def push_alias: -> [1, 2, 3]
end
```

## Widen to Array when branch changes length

### update

```ruby
def maybe_push(flag)
  xs = [1, 2]
  if flag
    xs << 3
  end
  xs
end
```

### result

```rbs
class Object
  def maybe_push: (untyped flag) -> Array[1 | 2 | 3]
end
```

## Update record static key

### update

```ruby
def replace_key
  h = { a: 1, b: 2 }
  h[:a] = 3
  h
end
```

### result

```rbs
class Object
  def replace_key: -> { a: 3, b: 2 }
end
```

## Make branch-added keys optional

### update

```ruby
def maybe_insert(flag)
  h = { a: 1, b: 2 }
  if flag
    h[:c] = 3
  end
  h
end
```

### result

```rbs
class Object
  def maybe_insert: (untyped flag) -> { a: 1, b: 2, ?c: 3 }
end
```

## Apply record update through ivar alias

### update

```ruby
class Box
  def initialize
    @h = { a: 1, b: 2 }
  end

  def update
    g = @h
    g[:a] = 3
    @h
  end
end
```

### result

```rbs
class Box
  def initialize: -> void
  def update: -> { a: 3, b: 2 }
end
```

## Hash write with dynamic key applies key and value types

### update

```ruby
def write_dynamic_key
  h = {}
  key = "name"
  h[key] = 1
  h
end
```

### result

```rbs
class Object
  def write_dynamic_key: -> Hash["name", 1]
end
```

## Array write with dynamic index applies element type

### update

```ruby
def replace_by_index
  values = ["a", "b"]
  index = 0
  values[index] = "c"
  values
end
```

### result

```rbs
class Object
  def replace_by_index: -> ["c", "b"]
end
```

## Apply Array writes inside each_with_index to outer array

### update

```ruby
def normalize_values
  values = ["a", "b"]
  values.each_with_index do |value, index|
    values[index] = value.upcase
  end
  values
end
```

### result

```rbs
class Object
  def normalize_values: -> Array[String]
end
```

## Apply array update through concat alias

### update

```ruby
def concat_alias
  values = []
  other = values
  other.concat([1, 2, 3])
  values
end
```

### result

```rbs
class Object
  def concat_alias: -> [1, 2, 3]
end
```

## Apply Hash update through merge! alias

### update

```ruby
def merge_alias
  table = {}
  other = table
  other.merge!(name: "a", count: 1)
  table
end
```

### result

```rbs
class Object
  def merge_alias: -> Hash[:count | :name, 1 | "a"]
end
```

## Identity map on tuple receiver widens toward RBS

### update

```ruby
def identity_map = [1, 2].map { |n| n }
```

### result

```rbs
class Object
  def identity_map: -> Array[1 | 2]
end
```

## min and max on a non-empty tuple are never nil

### update

```ruby
class Stats
  def min_of = [3, 1, 2].min
  def max_of = [3, 1, 2].max
  def min_str = ["c", "a", "b"].min
  def min_and_max = [3, 1, 2].minmax
end
```

### result

```rbs
class Stats
  def min_of: -> 1 | 2 | 3
  def max_of: -> 1 | 2 | 3
  def min_str: -> "a" | "b" | "c"
  def min_and_max: -> [1 | 2 | 3, 1 | 2 | 3]
end
```
