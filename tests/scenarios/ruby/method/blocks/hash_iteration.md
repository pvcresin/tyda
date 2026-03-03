# Ruby / Method / Blocks / Hash Iteration

## Hash#transform_values with block

### update

```ruby
def test_transform_values
  h = { a: 1, b: 2 }
  h.transform_values { |v| v.to_s }
end
```

### result

```rbs
class Object
  def test_transform_values: -> { a: String, b: String }
end
```

## Hash#transform_values! updates receiver

### update

```ruby
def transform_value_entries
  data = { a: "1", b: "2" }
  result = data.transform_values! { |value| value.to_i }
  [result, data]
end
```

### result

```rbs
class Object
  def transform_value_entries: -> [{ a: Integer, b: Integer }, { a: Integer, b: Integer }]
end
```

## Hash#transform_values preserves records

### update

```ruby
class Entry
  def label = "label"
end

def value_labels
  { first: Entry.new, second: Entry.new }.transform_values(&:label)
end

def value_strings
  { name: :alice, count: 1 }.transform_values(&method(:stringify))
end

def stringify(value)
  value.to_s
end
```

### result

```rbs
class Entry
  def label: -> "label"
end

class Object
  def value_labels: -> { first: "label", second: "label" }
  def value_strings: -> { name: String, count: String }
  def stringify: (untyped value) -> String
end
```

## Hash#transform_keys with block

### update

```ruby
def test_transform_keys
  h = { a: 1, b: 2 }
  h.transform_keys { |k| k.to_s }
end
```

### result

```rbs
class Object
  def test_transform_keys: -> Hash[String, 1 | 2]
end
```

## Hash#transform_keys with static mapping

### update

```ruby
def remap_symbol_keys
  data = { mon: 1, mday: 2, year: 2026 }
  data.transform_keys(mon: :month, mday: :day)
end

def remap_string_keys
  data = { "name" => "a", "count" => 1 }
  data.transform_keys("name" => :label)
end

def remap_keys_in_place
  data = { name: "a", count: 1 }
  data.transform_keys!(name: :label)
  data
end
```

### result

```rbs
class Object
  def remap_symbol_keys: -> { month: 1, day: 2, year: 2026 }
  def remap_string_keys: -> { label: "a", "count" => 1 }
  def remap_keys_in_place: -> { label: "a", count: 1 }
end
```

## Hash#transform_keys with block updates receiver

### update

```ruby
def transform_key_strings
  data = { "name" => "a", "count" => 1 }
  data.transform_keys { |key| key.to_sym }
end

def transform_key_strings_in_place
  data = { "name" => "a", "count" => 1 }
  result = data.transform_keys! { |key| key.to_sym }
  [result, data]
end

def transform_key_symbols_in_place
  data = { name: "a", count: 1 }
  result = data.transform_keys!(&:to_s)
  [result, data]
end

def transform_key_symbols
  { name: "a", count: 1 }.transform_keys(&:to_s)
end
```

### result

```rbs
class Object
  def transform_key_strings: -> Hash[:count | :name, 1 | "a"]
  def transform_key_strings_in_place: -> [Hash[:count | :name, 1 | "a"], Hash[:count | :name, 1 | "a"]]
  def transform_key_symbols_in_place: -> [{ "name" => "a", "count" => 1 }, { "name" => "a", "count" => 1 }]
  def transform_key_symbols: -> { "name" => "a", "count" => 1 }
end
```

## Hash#map with block (2 params)

### update

```ruby
def test_hash_map_block
  h = { a: 1, b: 2 }
  h.map { |k, v| v }
end
```

### result

```rbs
class Object
  def test_hash_map_block: -> Array[1 | 2]
end
```

## Hash#select with block (2 params)

### update

```ruby
def test_hash_select_block
  h = { a: 1, b: 2 }
  h.select { |k, v| v > 1 }
end
```

### result

```rbs
class Object
  def test_hash_select_block: -> Hash[:a | :b, 1 | 2]
end
```

## Hash destructive filters update receiver

### update

```ruby
def delete_entry_pairs
  data = { a: 1, b: nil, c: 2 }
  result = data.delete_if { |_key, value| value.nil? }
  [result, data]
end

def keep_entry_pairs
  data = { a: 1, b: 2 }
  result = data.keep_if { |key, _value| key == :a }
  [result, data]
end

def select_entry_pairs
  data = { a: 1, b: 2 }
  result = data.select! { |_key, value| value > 1 }
  [result, data]
end
```

### result

```rbs
class Object
  def delete_entry_pairs: -> [Hash[:a | :b | :c, (1 | 2)?], Hash[:a | :b | :c, (1 | 2)?]]
  def keep_entry_pairs: -> [Hash[:a | :b, 1 | 2], Hash[:a | :b, 1 | 2]]
  def select_entry_pairs: -> [Hash[:a | :b, 1 | 2]?, Hash[:a | :b, 1 | 2]]
end
```

## tally

### update

```ruby
def test_tally = ["a", "b", "a"].tally
```

### result

```rbs
class Object
  def test_tally: -> Hash["a" | "b", Integer]
end
```

## Hash#each_key and invert chains

### update

```ruby
def map_entry_keys
  data = { a: 1, b: 2 }
  data.each_key.map { |key| key.to_s }
end

def invert_then_transform_values
  data = { a: 1, b: 2 }
  data.invert.transform_values { |value| value.to_s }
end
```

### result

```rbs
class Object
  def map_entry_keys: -> Array[String]
  def invert_then_transform_values: -> Hash[1 | 2, String]
end
```

## Resolve map and sort chain on Hash entries

### update

```ruby
def map_entry_keys_exact
  data = { a: 1, b: 2 }
  data.map { |key, value| key }
end

def sort_entries
  data = { a: 1, b: 2 }
  data.sort_by { |key, value| key.to_s }
end

def sort_entries_with_symbol_proc
  { a: 1, b: 2 }.sort_by(&:first)
end

def sort_entries_to_hash
  data = { a: 1, b: 2 }
  data.sort_by { |key, value| key.to_s }.to_h
end
```

### result

```rbs
class Object
  def map_entry_keys_exact: -> Array[:a | :b]
  def sort_entries: -> Array[[:a | :b, 1 | 2]]
  def sort_entries_with_symbol_proc: -> Array[[:a | :b, 1 | 2]]
  def sort_entries_to_hash: -> Hash[:a | :b, 1 | 2]
end
```

## Resolve group partition and find chain on Hash entries

### update

```ruby
def group_entry_pairs
  { a: 1, b: 2 }.group_by { |key, value| value.odd? }
end

def partition_entry_pairs
  { a: 1, b: 2 }.partition { |key, value| value > 1 }
end

def find_entry_pair
  { a: 1, b: 2 }.find { |key, value| value > 1 }
end
```

### result

```rbs
class Object
  def group_entry_pairs: -> Hash[bool, Array[[:a | :b, 1 | 2]]]
  def partition_entry_pairs: -> [Array[[:a | :b, 1 | 2]], Array[[:a | :b, 1 | 2]]]
  def find_entry_pair: -> [:a | :b, 1 | 2]?
end
```

## Resolve min and max chain on Hash entries

### update

```ruby
def min_entry_pair
  { a: 1, b: 2 }.min_by { |key, value| value }
end

def max_entry_pair
  { a: 1, b: 2 }.max_by { |key, value| value }
end

def min_entry_pair_by_symbol
  { a: 1, b: 2 }.min_by(&:last)
end

def max_entry_pairs_by_count
  { a: 1, b: 2 }.max_by(2) { |key, value| value }
end

def min_entry_pairs_by_symbol_count
  { a: 1, b: 2 }.min_by(1, &:last)
end
```

### result

```rbs
class Object
  def min_entry_pair: -> [:a | :b, 1 | 2]?
  def max_entry_pair: -> [:a | :b, 1 | 2]?
  def min_entry_pair_by_symbol: -> [:a | :b, 1 | 2]?
  def max_entry_pairs_by_count: -> Array[[:a | :b, 1 | 2]]
  def min_entry_pairs_by_symbol_count: -> Array[[:a | :b, 1 | 2]]
end
```

## Resolve minmax chain on Hash entries

### update

```ruby
def minmax_entry_pairs
  { a: 1, b: 2 }.minmax
end

def minmax_entry_pairs_by_block
  { a: 1, b: 2 }.minmax_by { |key, value| value }
end

def minmax_entry_pairs_by_symbol
  { a: 1, b: 2 }.minmax_by(&:last)
end
```

### result

```rbs
class Object
  def minmax_entry_pairs: -> [[:a | :b, 1 | 2]?, [:a | :b, 1 | 2]?]
  def minmax_entry_pairs_by_block: -> [[:a | :b, 1 | 2]?, [:a | :b, 1 | 2]?]
  def minmax_entry_pairs_by_symbol: -> [[:a | :b, 1 | 2]?, [:a | :b, 1 | 2]?]
end
```

## Resolve no-block enumerable chain on Hash entries

### update

```ruby
def min_entry_without_block
  { a: 1, b: 2 }.min
end

def max_entry_count
  { a: 1, b: 2 }.max(2)
end

def tally_entry_pairs
  { a: 1, b: 1 }.tally
end

def sorted_entries
  { a: 1, b: 2 }.sort
end

def listed_entries
  { a: 1, b: 2 }.entries
end
```

### result

```rbs
class Object
  def min_entry_without_block: -> [:a | :b, 1 | 2]?
  def max_entry_count: -> Array[[:a | :b, 1 | 2]]
  def tally_entry_pairs: -> Hash[[:a | :b, 1], Integer]
  def sorted_entries: -> Array[[:a | :b, 1 | 2]]
  def listed_entries: -> Array[[:a | :b, 1 | 2]]
end
```

## transform_keys to_sym keeps a string-keyed record shape

### update

```ruby
def sym_keyed = { "name" => "Alice", "age" => 30 }.transform_keys(&:to_sym)
```

### result

```rbs
class Object
  def sym_keyed: -> { name: "Alice", age: 30 }
end
```

## transform_values on a mixed record widens each value to String

### update

```ruby
def up = { x: 1, y: "hello" }.transform_values { |v| v.to_s }
```

### result

```rbs
class Object
  def up: -> { x: String, y: String }
end
```
