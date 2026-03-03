# Ruby / Method / Blocks / Accumulators

## each_with_object

### update

```ruby
def use_each_with_object = [1, 2, 3].each_with_object([]) { |x, acc| acc << x }
```

### result

```rbs
class Object
  def use_each_with_object: -> Array[1 | 2 | 3]
end
```

## Update Array accumulator in each_with_object

### update

```ruby
def collect_strings
  [1, 2, 3].each_with_object([]) do |value, result|
    result << value.to_s
  end
end
```

### result

```rbs
class Object
  def collect_strings: -> Array[String]
end
```

## Write dynamic key to Hash accumulator in each_with_object

### update

```ruby
def build_table
  ["a", "b"].each_with_object({}) do |name, table|
    table[name] = true
  end
end
```

### result

```rbs
class Object
  def build_table: -> Hash["a" | "b", true]
end
```

## Append to default array Hash accumulator

### update

```ruby
def group_values(values)
  values.each_with_object(Hash.new { |hash, key| hash[key] = [] }) do |value, table|
    table[value.length] << value
  end
end

group_values(["a", "bb"])
```

### result

```rbs
class Object
  def group_values: (Array[String] values) -> Hash[Integer, Array[String]]
end
```

## Update existing accumulator alias in each_with_object

### update

```ruby
def collect_into_result
  result = []
  [1, 2, 3].each_with_object(result) do |value, list|
    list << value.to_s
  end
  result
end
```

### result

```rbs
class Object
  def collect_into_result: -> Array[String]
end
```

## Infer Hash value type in each_with_object from call site

### update

```ruby
def index_lengths(items)
  items.each_with_object({}) do |item, table|
    table[item] = item.length
  end
end

index_lengths(["hi", "tool"])
```

### result

```rbs
class Object
  def index_lengths: (Array[String] items) -> Hash[String, Integer]
end
```

## concat into Array accumulator in each_with_object

### update

```ruby
def collect_nested_values
  [[1, 2], [3]].each_with_object([]) do |values, result|
    result.concat(values)
  end
end
```

### result

```rbs
class Object
  def collect_nested_values: -> Array[1 | 2 | 3]
end
```

## Update accumulator in Enumerator#with_object

### update

```ruby
def each_with_object_chain
  ["a", "bb"].each.with_object({}) do |value, table|
    table[value] = value.length
  end
end

def reverse_with_object_chain
  [1, 2, 3].reverse_each.with_object([]) do |value, list|
    list << value.to_s
  end
end

def map_with_object_chain
  [1, 2].map.with_object({}) do |value, table|
    table[value] = value.odd?
  end
end
```

### result

```rbs
class Object
  def each_with_object_chain: -> Hash["a" | "bb", 1 | 2]
  def reverse_with_object_chain: -> Array[String]
  def map_with_object_chain: -> Hash[1 | 2, bool]
end
```

## Update accumulator after each_with_index

### update

```ruby
def index_table
  ["a", "bb"].each_with_index.with_object({}) do |(value, index), table|
    table[value] = index
  end
end

def index_rows
  [:a, :b].each_with_index.with_object([]) do |(value, index), rows|
    rows << [value, index]
  end
end

def nested_index_table
  [["a", 1], ["b", 2]].each_with_index.with_object({}) do |((key, value), index), table|
    table[key] = [value, index]
  end
end

def keep_index_rows
  rows = []
  ["a", "bb"].each_with_index.with_object(rows) do |(value, index), list|
    list << [value, index]
  end
  rows
end
```

### result

```rbs
class Object
  def index_table: -> Hash["a" | "bb", Integer]
  def index_rows: -> Array[[:a | :b, Integer]]
  def nested_index_table: -> Hash["a" | "b", [1 | 2, Integer]]
  def keep_index_rows: -> Array[["a" | "bb", Integer]]
end
```

## Update Set accumulator in object blocks

### update

```ruby
def collect_set_values
  [1, 2, 3].each_with_object(Set.new) do |value, set|
    set << value
  end
end

def merge_set_values
  [[1, 2], [3]].each_with_object(Set.new) do |values, set|
    set.merge(values)
  end
end

def collect_into_set
  result = Set.new
  ["a", "bb"].each_with_object(result) do |value, set|
    set.add(value.length)
  end
  result
end
```

### result

```rbs
class Object
  def collect_set_values: -> Set[1 | 2 | 3]
  def merge_set_values: -> Set[1 | 2 | 3]
  def collect_into_set: -> Set[1 | 2]
end
```
