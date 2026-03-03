# Ruby / Method / Blocks / Enumerator Chains

## chunk.map Enumerator chain

### update

```ruby
def test_chunk_map
  [1, 1, 2, 2, 3].chunk { |x| x }.map { |key, arr| arr.length }
end

def chunk_pairs
  ["a", "bb"].chunk { |value| value.length }.to_a
end

def chunk_table
  ["a", "bb"].chunk { |value| value.length }.to_h
end

def slice_pair_table
  [:name, "a", :count, 1].each_slice(2).to_h
end

def cons_pair_table
  [:name, "a", :count].each_cons(2).to_h
end
```

### result

```rbs
class Object
  def test_chunk_map: -> Array[Integer]
  def chunk_pairs: -> Array[[1 | 2, Array["a" | "bb"]]]
  def chunk_table: -> Hash[1 | 2, Array["a" | "bb"]]
  def slice_pair_table: -> Hash[:count | :name, 1 | "a"]
  def cons_pair_table: -> Hash["a" | :name, "a" | :count]
end
```

## Static to_enum method names keep element type

### update

```ruby
def default_enum_values
  ["a", "b"].to_enum.map(&:upcase)
end

def enum_value_lengths
  { name: "a", code: "bb" }.enum_for(:each_value).map(&:length)
end

def enum_key_names
  { name: "a", code: "bb" }.to_enum(:each_key).map(&:to_s)
end

def enum_pair_rows
  { name: "a", count: 1 }.to_enum(:each_pair).map { |key, value| [key, value] }
end

def enum_hash_table
  { name: "a", count: 1 }.to_enum.to_h
end

def enum_index_table
  ["a", "b"].enum_for(:each_with_index).to_h
end

def enum_slice_table
  [:name, "a", :count, 1].to_enum(:each_slice, 2).to_h
end

def enum_cons_rows
  [1, "a", 2].enum_for(:each_cons, 2).map { |left, right| [left, right] }
end
```

### result

```rbs
class Object
  def default_enum_values: -> Array["A" | "B"]
  def enum_value_lengths: -> Array[1 | 2]
  def enum_key_names: -> Array["code" | "name"]
  def enum_pair_rows: -> Array[[:count | :name, 1 | "a"]]
  def enum_hash_table: -> Hash[:count | :name, 1 | "a"]
  def enum_index_table: -> Hash["a" | "b", Integer]
  def enum_slice_table: -> Hash[:count | :name, 1 | "a"]
  def enum_cons_rows: -> Array[[1 | "a", 2 | "a"]]
end
```

## String to_enum methods keep element type

### update

```ruby
def scan_enum_rows
  "a1 b2".to_enum(:scan, /([a-z])(\d)/).to_a
end

def scan_enum_texts
  "a,b".enum_for(:scan, /[a-z]/).map(&:upcase)
end

def line_enum_values
  "a\nb".to_enum(:each_line).map(&:chomp)
end

def byte_enum_values
  "AZ".enum_for(:each_byte).to_a
end

def codepoint_enum_values
  "AZ".to_enum(:each_codepoint).map { |code| code + 1 }
end
```

### result

```rbs
class Object
  def scan_enum_rows: -> Array[[String?, String?]]
  def scan_enum_texts: -> Array[String]
  def line_enum_values: -> Array[String]
  def byte_enum_values: -> Array[Integer]
  def codepoint_enum_values: -> Array[Integer]
end
```

## Enumerator.new yielder values keep element type

### update

```ruby
class Source
  def enumerator_direct_values
    Enumerator.new do |yielder|
      yielder << "a"
      yielder.yield "b"
    end.map(&:upcase)
  end

  def enumerator_nested_values
    Enumerator.new do |yielder|
      [1, 2].each { |value| yielder << value }
    end.to_a
  end

  def enumerator_chained_values
    Enumerator.new do |yielder|
      yielder << :first << :second
    end.to_a
  end

  def enumerator_pair_table
    Enumerator.new do |yielder|
      yielder.yield :name, "a"
      yielder << [:count, 1]
    end.to_h
  end

  def enumerator_forwarded_lines
    Enumerator.new do |yielder|
      "a\nb".each_line(&yielder)
    end.map(&:chomp)
  end

  def enumerator_forwarded_pairs
    Enumerator.new do |yielder|
      { name: "a", count: 1 }.each_pair(&yielder)
    end.to_h
  end
end
```

### result

```rbs
class Source
  def enumerator_direct_values: -> Array["A" | "B"]
  def enumerator_nested_values: -> Array[1 | 2]
  def enumerator_chained_values: -> Array[:first | :second]
  def enumerator_pair_table: -> Hash[:count | :name, 1 | "a"]
  def enumerator_forwarded_lines: -> Array[String]
  def enumerator_forwarded_pairs: -> Hash[:count | :name, 1 | "a"]
end
```

## Pass Enumerable#chain array element type to later chain

### update

```ruby
def chain_values
  [1, 2].chain(["a"]).to_a
end

def chain_multiple_sources
  [1].chain([2], [3]).to_a
end

def enumerator_plus_values
  ([1, 2].each + ["a"].each).map { |value| value }
end
```

### result

```rbs
class Object
  def chain_values: -> Array[1 | 2 | "a"]
  def chain_multiple_sources: -> Array[1 | 2 | 3]
  def enumerator_plus_values: -> Array[1 | 2 | "a"]
end
```

## Resolve Enumerable#chain Hash and index chain

### update

```ruby
def chain_entry_table
  { a: 1 }.chain([[:b, 2]]).to_h
end

def chain_map_values
  ("a".."c").chain(["d"]).map { |value| value.upcase }
end

def chain_with_index_table
  ["a"].chain(["b"]).each.with_index.to_h { |value, index| [value, index] }
end

def chain_surrounding_windows
  [nil].chain([1, 2]).chain([nil]).each_cons(3).map { |left, middle, right| [left, middle, right] }
end
```

### result

```rbs
class Object
  def chain_entry_table: -> Hash[:a | :b, 1 | 2]
  def chain_map_values: -> Array[String]
  def chain_with_index_table: -> Hash["a" | "b", Integer]
  def chain_surrounding_windows: -> Array[[(1 | 2)?, (1 | 2)?, (1 | 2)?]]
end
```

## Do not confuse project method chain with Enumerable#chain

### update

```ruby
class Holder
  def chain(values)
    values
  end
end

def project_chain_method
  Holder.new.chain(["local"])
end
```

### result

```rbs
class Holder
  def chain: (Array[String] values) -> Array[String]
end

class Object
  def project_chain_method: -> Array[String]
end
```

## Pass Enumerator::Lazy element type to lazy chain

### update

```ruby
def lazy_force_lengths
  ["a", "bb"].lazy.map { |value| value.length }.take(1).force
end

def lazy_selected_symbols
  ["a", "", "bb"].lazy.select { |value| value.length > 0 }.map(&:to_sym).force
end

def lazy_flattened_values
  [[1], [2, 3]].lazy.flat_map { |values| values }.take(2).force
end

def lazy_eager_values
  [1, 2].lazy.map { |value| value.to_s }.eager.map(&:upcase)
end

def lazy_find_value
  [nil, "a"].lazy.map { |value| value&.upcase }.find(&:itself)
end

def lazy_filter_map_values
  [1, nil, 2].lazy.filter_map(&:itself).force
end
```

### result

```rbs
class Object
  def lazy_force_lengths: -> Array[1 | 2]
  def lazy_selected_symbols: -> Array[Symbol]
  def lazy_flattened_values: -> Array[1 | 2 | 3]
  def lazy_eager_values: -> Array[String]
  def lazy_find_value: -> String?
  def lazy_filter_map_values: -> Array[1 | 2]
end
```

## each_slice.map Enumerator chain

### update

```ruby
def test_each_slice_map
  [1, 2, 3, 4, 5, 6].each_slice(2).map { |chunk| chunk.sum }
end
```

### result

```rbs
class Object
  def test_each_slice_map: -> Array[Integer]
end
```

## Three-step map chain

### update

```ruby
def test_triple_map
  [1, 2, 3].map { |x| x.to_s }.map { |s| s.length }.map { |n| n > 0 }
end
```

### result

```rbs
class Object
  def test_triple_map: -> Array[bool]
end
```

## select + first

### update

```ruby
def test_select_first = [1, 2, 3, 4].select { |x| x > 2 }.first
```

### result

```rbs
class Object
  def test_select_first: -> (1 | 2 | 3 | 4)?
end
```

## sort with block

### update

```ruby
def test_sort_block = [3, 1, 2].sort { |a, b| a <=> b }
```

### result

```rbs
class Object
  def test_sort_block: -> Array[1 | 2 | 3]
end
```

## flat_map with nested map

### update

```ruby
def test_flat_map_nested
  [[1, 2], [3, 4]].flat_map { |arr| arr.map { |x| x.to_s } }
end
```

### result

```rbs
class Object
  def test_flat_map_nested: -> Array[String]
end
```

## each_slice + to_a

### update

```ruby
def test_each_slice_to_a = [1, 2, 3, 4, 5, 6].each_slice(3).to_a
```

### result

```rbs
class Object
  def test_each_slice_to_a: -> Array[[1 | 4, 2 | 5, 3 | 6]]
end
```

## Branch inside lambda

### update

```ruby
def test_lambda_branch
  f = -> (x) {
    if x > 0
      x.to_s
    else
      "negative"
    end
  }
  f.call(1)
end
```

### result

```rbs
class Object
  def test_lambda_branch: -> String
end
```

## Multi-step tap chain

### update

```ruby
def test_tap_multi_chain
  [1, 2, 3]
    .tap { |a| a }
    .select { |x| x > 1 }
    .tap { |a| a }
    .map { |x| x.to_s }
end
```

### result

```rbs
class Object
  def test_tap_multi_chain: -> Array[String]
end
```

## uniq with block

### update

```ruby
def test_uniq_block = [1, -1, 2, -2, 3].uniq { |x| x.abs }
```

### result

```rbs
class Object
  def test_uniq_block: -> Array[-2 | -1 | 1 | 2 | 3]
end
```

## each_cons and map chain

### update

```ruby
def test_each_cons_map
  [1, 2, 3, 4].each_cons(2).map { |pair| pair.sum }
end
```

### result

```rbs
class Object
  def test_each_cons_map: -> Array[Integer]
end
```

## filter_map removes nil

### update

```ruby
def test_filter_map = [1, 2, nil, 3].filter_map { |x| x }
```

### result

```rbs
class Object
  def test_filter_map: -> Array[1 | 2 | 3]
end
```

## filter_map with transformation

### update

```ruby
def test_filter_map_transform = ["1", "2", "abc"].filter_map { |s| s.to_i }
```

### result

```rbs
class Object
  def test_filter_map_transform: -> Array[Integer]
end
```

## reverse_each / take_while chains

### update

```ruby
def reverse_filter_map
  [1, nil, 2].reverse_each.filter_map { |value| value }
end

def take_prefix
  ["a", "bb", "ccc"].take_while { |value| value.length < 3 }
end

def drop_prefix
  ["a", "bb", "ccc"].drop_while { |value| value.length < 3 }
end
```

### result

```rbs
class Object
  def reverse_filter_map: -> Array[1 | 2]
  def take_prefix: -> Array["a" | "bb" | "ccc"]
  def drop_prefix: -> Array["a" | "bb" | "ccc"]
end
```

## sum with block

### update

```ruby
def test_sum_with_block = ["hello", "world"].sum { |s| s.length }
```

### result

```rbs
class Object
  def test_sum_with_block: -> Integer
end
```

## map + select chain

### update

```ruby
def test_map_select_chain
  [1, 2, 3, 4, 5].map { |x| x.to_s }.select { |s| s.length > 0 }
end
```

### result

```rbs
class Object
  def test_map_select_chain: -> Array[String]
end
```

## reject + map chain

### update

```ruby
def test_reject_map_chain
  [1, 2, 3, 4, 5].reject { |x| x > 3 }.map { |x| x.to_s }
end
```

### result

```rbs
class Object
  def test_reject_map_chain: -> Array[String]
end
```

## flat_map chain with select

### update

```ruby
def test_flat_map_select
  [[1, 2], [3, 4]].flat_map { |a| a }.select { |x| x > 1 }
end
```

### result

```rbs
class Object
  def test_flat_map_select: -> Array[1 | 2 | 3 | 4]
end
```

## each_with_index

### update

```ruby
def test_each_with_index
  result = []
  ["a", "b", "c"].each_with_index { |x, i| result << i }
  result
end
```

### result

```rbs
class Object
  def test_each_with_index: -> Array[Integer]
end
```

## map returns new array type

### update

```ruby
def test_map_integer_to_bool = [1, 2, 3].map { |x| x > 1 }
```

### result

```rbs
class Object
  def test_map_integer_to_bool: -> Array[bool]
end
```

## sort_by returns same element type

### update

```ruby
def test_sort_by_strings = ["banana", "apple", "cherry"].sort_by { |s| s.length }
```

### result

```rbs
class Object
  def test_sort_by_strings: -> Array["apple" | "banana" | "cherry"]
end
```

## max_by returns element | nil

### update

```ruby
def test_max_by = [3, 1, 2].max_by { |x| x }
```

### result

```rbs
class Object
  def test_max_by: -> (1 | 2 | 3)?
end
```

## select + count chain

### update

```ruby
def test_select_count = [1, 2, 3, 4, 5].select { |x| x > 2 }.count
```

### result

```rbs
class Object
  def test_select_count: -> Integer
end
```

## map + flat_map chain

### update

```ruby
def test_map_flat_map
  [[1, 2], [3, 4]].map { |a| a.map { |x| x.to_s } }.flat_map { |a| a }
end
```

### result

```rbs
class Object
  def test_map_flat_map: -> Array[String]
end
```

## reduce with string init

### update

```ruby
def test_reduce_string
  [1, 2, 3].reduce("") { |acc, x| acc + x.to_s }
end
```

### result

```rbs
class Object
  def test_reduce_string: -> String
end
```

## inject without init

### update

```ruby
def test_inject_no_init
  [1, 2, 3].inject { |sum, x| sum + x }
end
```

### result

```rbs
class Object
  def test_inject_no_init: -> Integer
end
```

## map + compact chain

### update

```ruby
def test_map_compact
  [1, nil, 2, nil, 3].map { |x| x }.compact
end
```

### result

```rbs
class Object
  def test_map_compact: -> Array[1 | 2 | 3]
end
```

## select + map + reduce 3-chain

### update

```ruby
def test_select_map_reduce
  [1, 2, 3, 4, 5].select { |x| x > 2 }.map { |x| x.to_s }.reduce("") { |acc, s| acc + s }
end
```

### result

```rbs
class Object
  def test_select_map_reduce: -> String
end
```

## flat_map + select + map 3-chain

### update

```ruby
def test_flat_map_select_map
  [[1, 2], [3, 4]].flat_map { |a| a }.select { |x| x > 1 }.map { |x| x.to_s }
end
```

### result

```rbs
class Object
  def test_flat_map_select_map: -> Array[String]
end
```

## sort_by + first chain

### update

```ruby
def test_sort_by_first
  ["banana", "apple", "cherry"].sort_by { |s| s.length }.first
end
```

### result

```rbs
class Object
  def test_sort_by_first: -> ("apple" | "banana" | "cherry")?
end
```

## Hash#each_value with block

### update

```ruby
def test_hash_each_value
  h = { a: 1, b: 2 }
  h.each_value { |v| v }
end
```

### result

```rbs
class Object
  def test_hash_each_value: -> { a: 1, b: 2 }
end
```

## map chain preserving type transformation

### update

```ruby
def test_map_to_float = [1, 2, 3].map { |x| x.to_f }
```

### result

```rbs
class Object
  def test_map_to_float: -> Array[Float]
end
```

## any? with string

### update

```ruby
def test_any_string = ["hello", "world"].any? { |s| s.length > 3 }
```

### result

```rbs
class Object
  def test_any_string: -> bool
end
```

## Hash#each_pair with block

### update

```ruby
def test_hash_each_pair
  h = { a: 1, b: 2 }
  h.each_pair { |k, v| v }
end
```

### result

```rbs
class Object
  def test_hash_each_pair: -> { a: 1, b: 2 }
end
```

## Hash#each return propagates key and value bindings

### update

```ruby
class H
  def each_pair_return
    { a: 1 }.each do |k, v|
      return [k, v]
    end
  end
end
```

### result

```rbs
class H
  def each_pair_return: -> { a: 1 } | [:a, 1]
end
```

## Hash#each single block arg keeps pair tuple

### update

```ruby
class H
  def each_tuple_return
    { a: 1 }.each do |kv|
      return kv
    end
  end
end
```

### result

```rbs
class H
  def each_tuple_return: -> { a: 1 } | [:a, 1]
end
```

## Enumerator produce and product

### update

```ruby
def produce_values
  Enumerator.produce(1, &:succ).take(3)
end

def produce_first
  Enumerator.produce("a") { |value| value.succ }.first
end

def product_rows
  Enumerator.product([1, 2], ["a", "b"]).to_a
end

def product_pairs
  Enumerator.product([:a, :b], [1, 2]).map { |key, value| [key, value] }
end

def product_side_effect_rows
  rows = []
  Enumerator.product([:a], [1]) { |row| rows << row }
  rows
end
```

### result

```rbs
class Object
  def produce_values: -> Array[1 | 2]
  def produce_first: -> String?
  def product_rows: -> Array[[1 | 2, "a" | "b"]]
  def product_pairs: -> Array[[:a | :b, 1 | 2]]
  def product_side_effect_rows: -> Array[Array[1 | :a]]
end
```

## ObjectSpace.each_object keeps static module element type

### update

```ruby
class Item
  def title = "item"
end

def object_space_item_titles
  ObjectSpace.each_object(Item).map(&:title)
end

def object_space_class_names
  ObjectSpace.each_object(Class).filter_map(&:name)
end

def object_space_module_names
  ObjectSpace.each_object(Module).filter_map(&:name)
end

def object_space_item_count
  ObjectSpace.each_object(Item) { |item| item.title }
end

def object_space_first_title
  ObjectSpace.each_object(Item) { |item| break item.title }
end
```

### result

```rbs
class Item
  def title: -> "item"
end

class Object
  def object_space_item_titles: -> Array["item"]
  def object_space_class_names: -> Array[String]
  def object_space_module_names: -> Array[String]
  def object_space_item_count: -> Integer
  def object_space_first_title: -> Integer | "item"
end
```

## User iterator enum keeps yield type

### update

```ruby
class Source
  def each
    return to_enum(__method__) unless block_given?
    yield "a"
    yield "b"
  end

  def values
    each.map(&:upcase)
  end

  def direct_values
    to_enum(:each).map(&:upcase)
  end
end
```

### result

```rbs
class Source
  def each: { (String) -> untyped } -> untyped | Enumerator["a" | "b", Source]
  def values: -> Array["A" | "B"]
  def direct_values: -> Array["A" | "B"]
end
```

## User iterator enum substitutes call args

### update

```ruby
class PairSource
  def each_value(value)
    return enum_for(__method__, value) unless block_given?
    yield value
  end

  def block_value
    each_value("a") { |value| value.upcase }
  end

  def enum_value
    each_value("b").map(&:upcase)
  end

  def direct_enum_value
    to_enum(:each_value, "c").map(&:upcase)
  end
end
```

### result

```rbs
class PairSource
  def each_value: (String value) { (untyped) -> String } -> (String | Enumerator[String, untyped])
  def block_value: -> String
  def enum_value: -> Array["B"]
  def direct_enum_value: -> Array["C"]
end
```

## User iterator enum substitutes keyword args

### update

```ruby
class KeywordSource
  def each_value(prefix:)
    return to_enum(__method__, prefix: prefix) unless block_given?
    yield "#{prefix}:a"
  end

  def values
    each_value(prefix: "x").map(&:upcase)
  end

  def direct_values
    to_enum(:each_value, prefix: "y").map(&:upcase)
  end
end
```

### result

```rbs
class KeywordSource
  def each_value: (prefix: String) { (String) -> untyped } -> (untyped | Enumerator[String, untyped])
  def values: -> Array[String]
  def direct_values: -> Array[String]
end
```

## Receiver user iterator enum keeps yield type

### update

```ruby
class Source
  def each
    return enum_for(:each) unless block_given?
    yield 1
  end
end

class Store
  def values
    source = Source.new
    source.to_enum(:each).map(&:to_s)
  end
end
```

### result

```rbs
class Source
  def each: { (Integer) -> untyped } -> untyped | Enumerator[1, Source]
end

class Store
  def values: -> Array["1"]
end
```

## User iterator enum follows named block

### update

```ruby
class BlockSource
  def each(&block)
    return to_enum(__method__) unless block
    block.call("a")
  end

  def block_value
    each { |value| value.upcase }
  end

  def enum_value
    each.map(&:upcase)
  end
end
```

### result

```rbs
class BlockSource
  def each: { (String) -> String } -> String | Enumerator["a", BlockSource]
  def block_value: -> String
  def enum_value: -> Array["A"]
end
```
