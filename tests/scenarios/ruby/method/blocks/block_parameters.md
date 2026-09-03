# Ruby / Method / Blocks / Block Parameters

## it parameter in Ruby 3.4+

### update

```ruby
def test_it_select = [1, 2, 3, 4, 5].select { it > 3 }
```

### result

```rbs
class Object < BasicObject
  def test_it_select: -> Array[1 | 2 | 3 | 4 | 5]
end
```

## it parameter in map

### update

```ruby
def test_it_map = [1, 2, 3].map { it.to_s }
```

### result

```rbs
class Object < BasicObject
  def test_it_map: -> Array[String]
end
```

## Numbered parameter _1

### update

```ruby
def test_numbered = [1, 2, 3].select { _1 > 2 }
```

### result

```rbs
class Object < BasicObject
  def test_numbered: -> Array[1 | 2 | 3]
end
```

## Numbered parameters _1 and _2 with hash

### update

```ruby
def test_numbered_hash = { a: 1, b: 2 }.each { puts "#{_1}: #{_2}" }
```

### result

```rbs
class Object < BasicObject
  def test_numbered_hash: -> { a: 1, b: 2 }
end
```

## Numbered parameters in reduce

### update

```ruby
def test_numbered_reduce = [1, 2, 3].reduce(0) { _1 + _2 }
```

### result

```rbs
class Object < BasicObject
  def test_numbered_reduce: -> Integer
end
```

## block local variable (`;`)

### update

```ruby
def block_local_shadow = [1, 2].map { |x; memo| memo = x.to_s; memo }
```

### result

```rbs
class Object < BasicObject
  def block_local_shadow: -> Array[String]
end
```

## destructuring block parameter

### update

```ruby
def destructured_pairs = [[1, "a"], [2, "b"]].map { |(n, s)| "#{n}:#{s}" }
```

### result

```rbs
class Object < BasicObject
  def destructured_pairs: -> Array["1:a" | "1:b" | "2:a" | "2:b"]
end
```

## nested destructuring block parameter

### update

```ruby
class A
  def nested_block_args
    [[1, [2, 3]]].map do |one, (two, (three, four))|
      [one, two, three, four]
    end
  end
end
```

### result

```rbs
class A
  def nested_block_args: -> Array[[1, 2, 3, nil]]
end
```

## destructuring block parameter with splat

### update

```ruby
class A
  def destructured_splat_block_arg
    [[1, 2, 3]].map do |(i, *args)|
      [i, args]
    end
  end
end
```

### result

```rbs
class A
  def destructured_splat_block_arg: -> Array[[1, [2, 3]]]
end
```

## block parameter tuple expansion with rest

### update

```ruby
class A
  def block_tuple_rest
    [[1, "x"]].map do |x, y, *z|
      [x, y, z]
    end
  end
end
```

### result

```rbs
class A
  def block_tuple_rest: -> Array[[1, "x", [ ]]]
end
```

## block parameter tuple expansion with trailing param

### update

```ruby
class A
  def block_tuple_trailing
    [[1, "x", :y]].map do |a, *b, c|
      [a, b, c]
    end
  end
end
```

### result

```rbs
class A
  def block_tuple_trailing: -> Array[[1, ["x"], :y]]
end
```

## optional block parameter uses default when tuple element is missing

### update

```ruby
class A
  def block_optional_default
    [[1], [1, 2]].map do |x, y = 0|
      [x, y]
    end
  end
end
```

### result

```rbs
class A
  def block_optional_default: -> Array[[1, 0 | 2]]
end
```

## optional block parameter keeps explicit nil

### update

```ruby
class A
  def block_optional_nil
    [[1, nil]].map do |x, y = 0|
      y
    end
  end
end
```

### result

```rbs
class A
  def block_optional_nil: -> Array[nil]
end
```

## optional block parameter with generic array expansion

### update

```ruby
class A
  #: -> Array[Integer]
  def numbers
    []
  end

  def block_optional_generic_array
    [numbers].map do |x, y = 0, *z|
      [x, y, z]
    end
  end
end
```

### result

```rbs
class A
  def numbers: -> Array[Integer]
  def block_optional_generic_array: -> Array[[Integer?, Integer, Array[Integer]]]
end
```

## optional block parameter with trailing param keeps tuple reservation

### update

```ruby
class A
  def block_optional_with_trailing
    [[1, :x], [1, 2, :x]].map do |a, b = 0, c|
      [a, b, c]
    end
  end
end
```

### result

```rbs
class A
  def block_optional_with_trailing: -> Array[[1, 0 | 2, :x]]
end
```

## trailing block parameter with generic array expansion

### update

```ruby
class A
  #: -> Array[Integer]
  def numbers
    []
  end

  def block_trailing_generic_array
    [numbers].map do |a, *b, c|
      [a, b, c]
    end
  end
end
```

### result

```rbs
class A
  def numbers: -> Array[Integer]
  def block_trailing_generic_array: -> Array[[Integer?, Array[Integer], Integer?]]
end
```

## `next` with value in `map`

### update

```ruby
def map_with_next = [1, 2, 3].map { |x| next x.to_s if x.even?; x }
```

### result

```rbs
class Object < BasicObject
  def map_with_next: -> Array[String | 1 | 3]
end
```

## `next` with multiple values in `map`

### update

```ruby
def map_with_next_multiple = [1].map { next 1, "two" }
```

### result

```rbs
class Object < BasicObject
  def map_with_next_multiple: -> Array[[1, "two"]]
end
```

## `break` in `each`

### update

```ruby
def each_with_break = [1, 2, 3].each { |x| break :done if x.even?; x }
```

### result

```rbs
class Object < BasicObject
  def each_with_break: -> :done | [1, 2, 3]
end
```

## `break` in `map`

### update

```ruby
def map_with_break = [1, 2, 3].map { |x| break :done if x.even?; x.to_s }
```

### result

```rbs
class Object < BasicObject
  def map_with_break: -> :done | Array[String]
end
```

## Block local variable does not affect outer local

### update

```ruby
def block_local_outer
  memo = :outer
  [1].each { |x; memo| memo = x.to_s }
  memo
end
```

### result

```rbs
class Object < BasicObject
  def block_local_outer: -> :outer
end
```

## Block arg shadowing does not affect outer local

### update

```ruby
class A
  def shadowed_block_arg
    arg = 123
    [1].each do |arg|
      arg.to_s
    end
    arg
  end
end
```

### result

```rbs
class A
  def shadowed_block_arg: -> 123
end
```

## `each` with `next`

### update

```ruby
def each_with_next = [1, 2, 3].each { |x| next if x.even?; x.to_s }
```

### result

```rbs
class Object < BasicObject
  def each_with_next: -> [1, 2, 3]
end
```

## it parameter in then

```yaml
ruby_version: 3.3.0
```

### update

```ruby
def test_it_then = 42.then { it.to_s }
```

### result

```rbs
class Object < BasicObject
  def test_it_then: -> String
end
```

## block-dependent Enumerator#with_index chains

### update

```ruby
def map_pairs
  ["a", "b"].map.with_index { |value, index| [value, index] }
end

def map_hash
  Hash[["a", "b"].map.with_index { |value, index| [value, index] }]
end

def filter_map_pairs
  [1, nil, 2].filter_map.with_index { |value, index| value && [value, index] }
end

def flat_map_values
  ["a", "b"].flat_map.with_index { |value, index| [value, index] }
end
```

### result

```rbs
class Object < BasicObject
  def map_pairs: -> Array[["a" | "b", Integer]]
  def map_hash: -> Hash["a" | "b", Integer]
  def filter_map_pairs: -> Array[[1 | 2, Integer]]
  def flat_map_values: -> Array[Integer | "a" | "b"]
end
```

## Enumerator#with_index to_h chain keeps value type

### update

```ruby
def each_pairs
  ["a", "b"].each.with_index.to_h
end

def indexed_record_names
  [{ name: "one" }, { name: "two" }].each_with_index.to_h do |entry, index|
    [entry[:name], index]
  end
end
```

### result

```rbs
class Object < BasicObject
  def each_pairs: -> Hash["a" | "b", Integer]
  def indexed_record_names: -> Hash["one" | "two", Integer]
end
```

## Enumerator#with_index filter chain keeps receiver value type

### update

```ruby
def select_with_index
  [1, 2, 3].select.with_index { |value, index| index > 0 }
end

def reject_with_index
  [1, 2, 3].reject.with_index { |value, index| index.zero? }
end

def filter_with_index
  [1, 2, 3].filter.with_index { |value, index| index > 0 && value.odd? }
end

def find_all_with_index
  [1, 2, 3].find_all.with_index { |value, index| index > 0 && value.odd? }
end

def partition_with_index
  [1, 2, 3].partition.with_index { |_value, index| index > 0 }
end
```

### result

```rbs
class Object < BasicObject
  def select_with_index: -> Array[1 | 2 | 3]
  def reject_with_index: -> Array[1 | 2 | 3]
  def filter_with_index: -> Array[1 | 2 | 3]
  def find_all_with_index: -> Array[1 | 2 | 3]
  def partition_with_index: -> [Array[1 | 2 | 3], Array[1 | 2 | 3]]
end
```

## Enumerator#with_index sort_by chain keeps value type

### update

```ruby
def sort_by_with_index
  ["bb", "a"].sort_by.with_index { |value, index| [value.length, index] }
end
```

### result

```rbs
class Object < BasicObject
  def sort_by_with_index: -> Array["a" | "bb"]
end
```

## Enumerator#with_index map bang chain updates receiver

### update

```ruby
def map_in_place_with_index
  values = ["a", "bb"]
  result = values.map!.with_index { |value, index| value.length + index }
  [result, values]
end

def collect_in_place_with_index
  values = [1, 2]
  changed = values.collect!.with_index { |value, index| value.to_s + index.to_s }
  [changed, values]
end
```

### result

```rbs
class Object < BasicObject
  def map_in_place_with_index: -> [Array[Integer], Array[Integer]]
  def collect_in_place_with_index: -> [Array[String], Array[String]]
end
```

## it parameter stays untyped in Ruby 3.2

```yaml
ruby_version: 3.2.0
```

### update

```ruby
def stringify = [1].map { it.to_s }
```

### result

```rbs
class Object < BasicObject
  def stringify: -> Array[String]
end
```

## it parameter is inferred in Ruby 3.3

```yaml
ruby_version: 3.3.0
```

### update

```ruby
def stringify = [1].map { it.to_s }
```

### result

```rbs
class Object < BasicObject
  def stringify: -> Array[String]
end
```

## Record block return type for receiver method call site

### update

```ruby
class Processor
  def each_item
    yield "hello"
    yield "world"
  end

  def collect
    results = []
    each_item { |s| results << s.upcase }
    results
  end
end
```

### result

```rbs
class Processor
  def each_item: { (String) -> [String] } -> [String]
  def collect: -> Array[String]
end
```

## Infer receiver method chain with block

### update

```ruby
class Batch
  def chain = [1, 2, 3].select { |x| x > 1 }.map { |x| x.to_f }
end
```

### result

```rbs
class Batch
  def chain: -> Array[Float]
end
```

## `proc` and `lambda` infer return through `.call`

### update

```ruby
class A
  def pl = ->(x) { x + 1 }.call(10)
  def pr = proc { |x| x + 1 }.call(10)
  def la = lambda { |x| x.to_s }.call(1)
  def pn = Proc.new { |x| x * 2 }.call(5)
end
```

### result

```rbs
class A
  def pl: -> Integer
  def pr: -> Integer
  def la: -> String
  def pn: -> Integer
end
```
