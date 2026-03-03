# Ruby / Method / Array Hash

## Keep hash value omission exact

### update

```ruby
def build_hash
  x = 1
  {x:}
end
```

### result

```rbs
class Object
  def build_hash: -> { x: 1 }
end
```

## Hash value omission is untyped in Ruby 3.0

```yaml
ruby_version: 3.0.0
```

### update

```ruby
def build_hash
  x = 1
  {x:}
end
```

### result

```rbs
class Object
  def build_hash: -> { x: untyped }
end
```

## Return array literal

### update

```ruby
def foo = [1, 2, 3]
```

### result

```rbs
class Object
  def foo: -> [1, 2, 3]
end
```

## Return empty array

### update

```ruby
def empty_list = []
```

### result

```rbs
class Object
  def empty_list: -> [ ]
end
```

## Return hash literal

### update

```ruby
def foo = { name: "Alice", age: 30 }
```

### result

```rbs
class Object
  def foo: -> { name: "Alice", age: 30 }
end
```

## Return empty hash

### update

```ruby
def empty_map = {}
```

### result

```rbs
class Object
  def empty_map: -> Hash[untyped, untyped]
end
```

## Apply Hash.new default value to value type

### update

```ruby
def count_map = Hash.new(0)
```

### result

```rbs
class Object
  def count_map: -> Hash[untyped, 0]
end
```

## Apply Array.new default and block return to element type

### update

```ruby
def empty_array_new = Array.new

def nil_array_new = Array.new(2)

def filled_array_new = Array.new(3, "x")

def block_array_new
  Array.new(3) { |index| index.to_s }
end

def nested_array_new
  Array.new(2) { [] }
end
```

### result

```rbs
class Object
  def empty_array_new: -> Array
  def nil_array_new: -> Array[nil]
  def filled_array_new: -> Array["x"]
  def block_array_new: -> Array[String]
  def nested_array_new: -> Array[[ ]]
end
```

## Apply Hash.new default proc return to value type

### update

```ruby
def list_map
  Hash.new { [] }
end
```

### result

```rbs
class Object
  def list_map: -> Hash[untyped, [ ]]
end
```

## Apply assignment in Hash.new default proc to value type

### update

```ruby
def grouped_map
  Hash.new { |hash, key| hash[key] = [] }
end
```

### result

```rbs
class Object
  def grouped_map: -> Hash[untyped, [ ]]
end
```

## Apply indexed default collection mutations to Hash value type

### update

```ruby
def grouped_values
  table = Hash.new { |hash, key| hash[key] = [] }
  table[:group] << "one"
  table[:group] << "two"
  table
end

def nested_values
  table = Hash.new { |hash, key| hash[key] = {} }
  table[:group][:count] = 1
  table
end
```

### result

```rbs
class Object
  def grouped_values: -> Hash[:group, Array["one" | "two"]]
  def nested_values: -> Hash[:group, Hash[untyped, untyped] | Hash[:count, 1]]
end
```

## Apply Set.new enumerable element type

### update

```ruby
def id_set
  Set.new([1, 2, 3])
end
```

### result

```rbs
class Object
  def id_set: -> Set[1 | 2 | 3]
end
```

## Apply Set.new block return to element type

### update

```ruby
def name_set
  Set.new([1, 2]) { |value| value.to_s }
end
```

### result

```rbs
class Object
  def name_set: -> Set[String]
end
```

## Array#to_set applies element and block return to Set

### update

```ruby
def id_set_from_array
  [1, 2, 3].to_set
end

def name_set_from_block
  [{ name: "one" }, { name: "two" }].to_set { |entry| entry[:name] }
end

def name_set_from_symbol_proc
  [1, 2].to_set(&:to_s)
end
```

### result

```rbs
class Object
  def id_set_from_array: -> Set[1 | 2 | 3]
  def name_set_from_block: -> Set["one" | "two"]
  def name_set_from_symbol_proc: -> Set["1" | "2"]
end
```

## Hash#to_set applies entry key and value types to Set

### update

```ruby
def key_set_from_hash
  { name: "one", count: "two" }.each_key.to_set
end

def value_set_from_hash
  { name: "one", count: "two" }.each_value.to_set
end

def entry_set_from_hash
  { name: "one", count: "two" }.to_set
end
```

### result

```rbs
class Object
  def key_set_from_hash: -> Set[:count | :name]
  def value_set_from_hash: -> Set["one" | "two"]
  def entry_set_from_hash: -> Set[[:count | :name, "one" | "two"]]
end
```

## Hash#to_set applies block return to Set

### update

```ruby
def remapped_entry_set
  { name: 1, count: 2 }.to_set { |key, value| [key.to_s, value] }
end

def flattened_value_set
  { a: [1], b: [2, 3] }.each_value.flat_map { |values| values }.to_set
end
```

### result

```rbs
class Object
  def remapped_entry_set: -> Set[[String, 1 | 2]]
  def flattened_value_set: -> Set[1 | 2 | 3]
end
```

## Set bracket constructor applies argument element type

### update

```ruby
def literal_set
  Set[1, "two", :three]
end

def splat_set
  values = [1, 2]
  Set[*values]
end
```

### result

```rbs
class Object
  def literal_set: -> Set[1 | "two" | :three]
  def splat_set: -> Set[1 | 2]
end
```

## Set operations keep static element types

### update

```ruby
def union_set
  Set[:read].union(Set[:write])
end

def pipe_set
  Set[1] | Set["two"]
end

def intersection_set
  Set[1, 2].intersection(Set[2, 3])
end

def ampersand_set
  Set[:a, :b] & Set[:b, :c]
end

def difference_set
  Set[1, 2].difference(Set[2])
end

def minus_set
  Set[:a, :b] - Set[:b]
end

def xor_set
  Set[:a, :b] ^ Set[:b, :c]
end
```

### result

```rbs
class Object
  def union_set: -> Set[:read | :write]
  def pipe_set: -> Set[1 | "two"]
  def intersection_set: -> Set[2]
  def ampersand_set: -> Set[:b]
  def difference_set: -> Set[1]
  def minus_set: -> Set[:a]
  def xor_set: -> Set[:a | :c]
end
```

## Set destructive operations update receiver

### update

```ruby
def merge_set
  values = Set[:read]
  result = values.merge(Set[:write])
  [result, values]
end

def subtract_set
  values = Set[:read, :write]
  result = values.subtract(Set[:write])
  [result, values]
end

def delete_set
  values = Set[:read, :write]
  result = values.delete(:write)
  [result, values]
end
```

### result

```rbs
class Object
  def merge_set: -> [Set[:read | :write], Set[:read | :write]]
  def subtract_set: -> [Set[:read], Set[:read]]
  def delete_set: -> [Set[:read], Set[:read]]
end
```

## Assign array to variable

### update

```ruby
def foo
  x = [1, 2]
  x
end
```

### result

```rbs
class Object
  def foo: -> [1, 2]
end
```

## Array with mixed types

### update

```ruby
def foo = [1, "hello"]
```

### result

```rbs
class Object
  def foo: -> [1, "hello"]
end
```

## Array#first and last keep tuple edges

### update

```ruby
def first_item
  ["one", "two", "three"].first
end

def last_item
  ["one", "two", "three"].last
end
```

### result

```rbs
class Object
  def first_item: -> "one"
  def last_item: -> "three"
end
```

## Array#first(n) and last(n) keep tuple slices

### update

```ruby
def first_items
  ["one", "two", "three"].first(2)
end

def last_items
  ["one", "two", "three"].last(2)
end
```

### result

```rbs
class Object
  def first_items: -> ["one", "two"]
  def last_items: -> ["two", "three"]
end
```

## Array#take and drop keep tuple slices

### update

```ruby
def take_items
  [1, 2, 3].take(2)
end

def drop_items
  [1, 2, 3].drop(1)
end
```

### result

```rbs
class Object
  def take_items: -> [1, 2]
  def drop_items: -> [2, 3]
end
```

## Array#sample keeps element precision

### update

```ruby
def sampled_state
  ["opened", "closed"].sample
end

def sampled_roles
  [:reader, :writer, :owner].sample(2)
end

def sampled_random_count
  [:reader, :writer, :owner].sample(rand(3))
end

def sampled_with_random
  [1, 2, 3].sample(random: Random.new(1))
end

def sampled_empty
  [].sample
end

def sampled_empty_many
  [].sample(2)
end
```

### result

```rbs
class Object
  def sampled_state: -> "closed" | "opened"
  def sampled_roles: -> Array[:owner | :reader | :writer]
  def sampled_random_count: -> Array[:owner | :reader | :writer]
  def sampled_with_random: -> 1 | 2 | 3
  def sampled_empty: -> nil
  def sampled_empty_many: -> [ ]
end
```

## Array index and values_at keep tuple shape

### update

```ruby
def negative_index_item
  [:small, :medium, :large][-1]
end

def selected_items
  ["one", "two", "three"].values_at(0, -1, 5)
end

def range_slice_items
  ["one", "two", "three"][1..-1]
end

def endless_slice_items
  ["one", "two", "three"][1..]
end

def beginless_slice_items
  ["one", "two", "three"][..1]
end

def exclusive_slice_items
  ["one", "two", "three"][0...2]
end

def negative_end_slice_items
  ["one", "two", "three"][0..-4]
end

def call_slice_items
  ["one", "two", "three"].slice(0, 2)
end

def empty_slice
  [1, 2].drop(5)
end

def missing_range_slice
  [1, 2][3..]
end
```

### result

```rbs
class Object
  def negative_index_item: -> :large
  def selected_items: -> ["one", "three", nil]
  def range_slice_items: -> ["two", "three"]
  def endless_slice_items: -> ["two", "three"]
  def beginless_slice_items: -> ["one", "two"]
  def exclusive_slice_items: -> ["one", "two"]
  def negative_end_slice_items: -> [ ]
  def call_slice_items: -> ["one", "two"]
  def empty_slice: -> [ ]
  def missing_range_slice: -> nil
end
```

## Array index accepts literal float positions

### update

```ruby
def float_index_item
  ["zero", "one", "two"][1.9]
end

def float_negative_index_item
  ["zero", "one", "two"][-1.1]
end

def float_slice_items
  ["zero", "one", "two", "three"][1.9, 2.1]
end

def float_values_at_items
  ["zero", "one", "two"].values_at(0.9, 2.1)
end
```

### result

```rbs
class Object
  def float_index_item: -> "one"
  def float_negative_index_item: -> "two"
  def float_slice_items: -> ["one", "two"]
  def float_values_at_items: -> ["zero", "two"]
end
```

## Array fetch_values keeps tuple shape

### update

```ruby
def fetched_items
  ["zero", "one", "two"].fetch_values(2, 0, -1)
end

def fetched_items_with_block
  ["zero", "one"].fetch_values(1, 4) { |index| index.to_s }
end

def fetched_items_with_splat
  indexes = [0, -1]
  ["zero", "one"].fetch_values(*indexes)
end

def empty_fetch_values = [1, 2].fetch_values
```

### result

```rbs
class Object
  def fetched_items: -> ["two", "zero", "two"]
  def fetched_items_with_block: -> ["one", String]
  def fetched_items_with_splat: -> ["zero", "one"]
  def empty_fetch_values: -> [ ]
end
```

## Array shift pop delete_at and delete keep return and updated shape

### update

```ruby
def shift_pop_flow
  parts = ["a", 1, :b, false]
  first = parts.shift
  last = parts.pop
  [first, last, parts]
end

def shift_count_flow
  parts = [:a, :b, :c]
  taken = parts.shift(2)
  [taken, parts]
end

def pop_count_flow
  parts = [1, 2, 3]
  taken = parts.pop(2)
  [taken, parts]
end

def delete_at_flow
  parts = ["a", 1, :b]
  removed = parts.delete_at(1)
  [removed, parts]
end

def delete_value_flow
  parts = [:a, :b, :a]
  removed = parts.delete(:a)
  [removed, parts]
end
```

### result

```rbs
class Object
  def shift_pop_flow: -> ["a", false, [1, :b]]
  def shift_count_flow: -> [[:a, :b], [:c]]
  def pop_count_flow: -> [[2, 3], [1]]
  def delete_at_flow: -> [1, ["a", :b]]
  def delete_value_flow: -> [:a?, [:b]]
end
```

## Array#push and concat return updated shape

### update

```ruby
def push_many_flow
  parts = [:a]
  changed = parts.push(:b, "c")
  [changed, parts]
end

def concat_many_flow
  parts = [:a]
  changed = parts.concat([:b], ["c"])
  [changed, parts]
end
```

### result

```rbs
class Object
  def push_many_flow: -> [[:a, :b, "c"], [:a, :b, "c"]]
  def concat_many_flow: -> [[:a, :b, "c"], [:a, :b, "c"]]
end
```

## Array#unshift and prepend return updated shape

### update

```ruby
def unshift_flow
  parts = [2, 3]
  changed = parts.unshift(1)
  [changed, parts]
end

def prepend_flow
  parts = ["tail"]
  parts.prepend("head", :marker)
end
```

### result

```rbs
class Object
  def unshift_flow: -> [[1, 2, 3], [1, 2, 3]]
  def prepend_flow: -> ["head", :marker, "tail"]
end
```

## Array#insert and replace return updated shape

### update

```ruby
def insert_flow
  parts = [:a, :d]
  changed = parts.insert(1, :b, :c)
  [changed, parts]
end

def replace_flow
  parts = [1, "old"]
  changed = parts.replace([:a, "new"])
  [changed, parts]
end

def fill_all_flow
  parts = [nil, nil]
  changed = parts.fill("x")
  [changed, parts]
end

def fill_range_flow
  parts = [:a, nil, nil, :d]
  changed = parts.fill("x", 1, 2)
  [changed, parts]
end

def fill_block_flow
  parts = [nil, nil]
  changed = parts.fill { |index| index }
  [changed, parts]
end
```

### result

```rbs
class Object
  def insert_flow: -> [[:a, :b, :c, :d], [:a, :b, :c, :d]]
  def replace_flow: -> [[:a, "new"], [:a, "new"]]
  def fill_all_flow: -> [["x", "x"], ["x", "x"]]
  def fill_range_flow: -> [[:a, "x", "x", :d], [:a, "x", "x", :d]]
  def fill_block_flow: -> [[0 | 1, 0 | 1], [0 | 1, 0 | 1]]
end
```

## Resolve generic type variables with zip

### update

```ruby
def zip_str_int
  ["a", "b"].zip([1, 2])
end

def zip_int_sym
  [1, 2, 3].zip([:x, :y, :z])
end
```

### result

```rbs
class Object
  def zip_str_int: -> Array[["a" | "b", (1 | 2)?]]
  def zip_int_sym: -> Array[[1 | 2 | 3, (:x | :y | :z)?]]
end
```

## Keep pair type through zip and cycle chain

### update

```ruby
def zip_with_repeated_label
  [1, 2].zip([:source].cycle)
end

def zip_cycle_to_hash
  [:a, :b].zip([true].cycle).to_h
end
```

### result

```rbs
class Object
  def zip_with_repeated_label: -> Array[[1 | 2, :source?]]
  def zip_cycle_to_hash: -> Hash[:a | :b, true | nil]
end
```

## transpose keeps column types

### update

```ruby
def transpose_pairs
  [[:project, 1], [:package, 2]].transpose
end

def split_columns
  names, counts = [["one", 1], ["two", 2]].transpose
  [names, counts]
end

def transpose_rows
  [[1, "a", :x], [2, "b", :y]].transpose
end
```

### result

```rbs
class Object
  def transpose_pairs: -> [Array[:package | :project], Array[1 | 2]]
  def split_columns: -> [Array["one" | "two"], Array[1 | 2]]
  def transpose_rows: -> [Array[1 | 2], Array["a" | "b"], Array[:x | :y]]
end
```

## Array pair to_h recovers key and value types

### update

```ruby
def pairs_to_hash
  [[:name, "a"], [:count, 1]].to_h
end

def mapped_pairs_to_hash
  ["a", "bb"].map { |value| [value, value.length] }.to_h
end

def rest_pair_block_to_hash
  [["a", 1, 2], ["b", 3, 4]].to_h do |key, *values|
    [key, values]
  end
end

def remap_entry_hash
  { code: 1, count: 2 }.to_h do |key, value|
    [key.to_s, value.to_s]
  end
end
```

### result

```rbs
class Object
  def pairs_to_hash: -> Hash[:count | :name, 1 | "a"]
  def mapped_pairs_to_hash: -> Hash["a" | "bb", 1 | 2]
  def rest_pair_block_to_hash: -> Hash["a" | "b", [1, 2] | [3, 4]]
  def remap_entry_hash: -> Hash[String, String]
end
```

## Hash[] recovers key and value types from array pairs

### update

```ruby
def pair_array_hash
  Hash[[[:name, "a"], [:count, 1]]]
end

def pair_args_hash
  Hash[[:name, "a"], [:count, 1]]
end

def product_hash
  Hash[[:ok, :skip].product([true])]
end

def product_splat_rows
  groups = [[true], [:new, :old]]
  [:name, :count].product(*groups)
end
```

### result

```rbs
class Object
  def pair_array_hash: -> Hash[:count | :name, 1 | "a"]
  def pair_args_hash: -> Hash[:count | :name, 1 | "a"]
  def product_hash: -> Hash[:ok | :skip, true]
  def product_splat_rows: -> Array[[:count | :name, true, :new | :old]]
end
```

## Hash[] recovers key and value types from splatted pairs

### update

```ruby
def flat_args_hash
  Hash[*[:name, "a", :count, 1]]
end

def local_flat_args_hash
  values = [:name, "a", :count, 1]
  Hash[*values]
end

def flattened_pairs_hash
  pairs = [[:name, "a"], [:count, 1]]
  Hash[*pairs.flatten]
end

def mapped_flatten_hash
  pairs = [[:name, "a"], [:count, 1]]
  Hash[*pairs.map { |key, value| [key, value] }.flatten]
end

def flat_map_hash
  pairs = [[:name, "a"], [:count, 1]]
  Hash[*pairs.flat_map { |key, value| [key, value] }]
end

def project_flat_hash
  Hash[*flat_pairs]
end

def flat_pairs = [:enabled, true, :archived, false]
```

### result

```rbs
class Object
  def flat_args_hash: -> Hash[:count | :name, 1 | "a"]
  def local_flat_args_hash: -> Hash[:count | :name, 1 | "a"]
  def flattened_pairs_hash: -> Hash[:count | :name, 1 | "a"]
  def mapped_flatten_hash: -> Hash[:count | :name, 1 | "a"]
  def flat_map_hash: -> Hash[:count | :name, 1 | "a"]
  def project_flat_hash: -> Hash[:archived | :enabled, bool]
  def flat_pairs: -> [:enabled, true, :archived, false]
end
```

## Array combination Enumerator keeps element array type

### update

```ruby
def pair_combinations
  [:first, :second, :third].combination(2).to_a
end

def pair_permutations
  [:first, :second, :third].permutation(2).to_a
end

def all_permutations
  [:first, :second].permutation.to_a
end
```

### result

```rbs
class Object
  def pair_combinations: -> Array[[:first | :second | :third, :first | :second | :third]]
  def pair_permutations: -> Array[[:first | :second | :third, :first | :second | :third]]
  def all_permutations: -> Array[[:first | :second, :first | :second]]
end
```

## Array combination Enumerator passes element array to block

### update

```ruby
def combination_names
  [:first, :second, :third].combination(2).map do |left, right|
    [left, right].join(":")
  end
end

def permutation_keys
  [:first, :second].permutation.map { |pair| pair[0] }
end
```

### result

```rbs
class Object
  def combination_names: -> Array[String]
  def permutation_keys: -> Array[:first | :second]
end
```

## Array#join keeps static string result

### update

```ruby
def join_path_parts
  ["root", "child", "leaf"].join("/")
end

def join_compact_parts
  ["root", nil, "leaf"].compact.join("/")
end

def join_symbol_parts
  [:read, :write].join(",")
end

def join_number_parts
  [1, 2, 3].join(".")
end

def join_nested_parts
  ["root", ["child", "leaf"]].join("/")
end

def join_default_separator
  ["a", "b"].join
end

def join_dynamic_parts(value)
  ["root", value].join("/")
end
```

### result

```rbs
class Object
  def join_path_parts: -> "root/child/leaf"
  def join_compact_parts: -> "root/leaf"
  def join_symbol_parts: -> "read,write"
  def join_number_parts: -> "1.2.3"
  def join_nested_parts: -> "root/child/leaf"
  def join_default_separator: -> "ab"
  def join_dynamic_parts: (untyped value) -> String
end
```

## Static collection cardinality

### update

```ruby
class Entry
  def name = "entry"
end

def tuple_size = ["name", "path"].size
def tuple_length = [1, 2, 3].length
def tuple_count = [:name, :path, :name].count(:name)
def compact_nil_count = [1, nil, 2].compact.count(nil)
def record_size = { name: "entry", count: 1 }.size
def compact_project_names = [Entry.new, nil].compact.map(&:name)
```

### result

```rbs
class Entry
  def name: -> "entry"
end

class Object
  def tuple_size: -> 2
  def tuple_length: -> 3
  def tuple_count: -> 2
  def compact_nil_count: -> 0
  def record_size: -> 2
  def compact_project_names: -> Array["entry"]
end
```

## Array set operations keep static element types

### update

```ruby
def union_keys
  [:local, :global].union([:env], [:temporary])
end

def pipe_keys
  [:local, :global] | [:env]
end

def intersection_keys
  [:local, :env, :global].intersection([:env, :temporary], [:env, :local])
end

def ampersand_keys
  [:local, :env] & [:env, :temporary]
end

def difference_keys
  [:create, :update, :delete].difference([:delete], [:archive])
end

def minus_keys
  [:create, :update, :delete] - [:delete]
end
```

### result

```rbs
class Object
  def union_keys: -> Array[:env | :global | :local | :temporary]
  def pipe_keys: -> Array[:env | :global | :local]
  def intersection_keys: -> Array[:env]
  def ampersand_keys: -> Array[:env]
  def difference_keys: -> Array[:create | :update]
  def minus_keys: -> Array[:create | :update]
end
```

## Array set operations work on project method return

### update

```ruby
class Source
  def first_keys = [:local, :global]
  def second_keys = [:global, :env]

  def merged_keys
    first_keys.union(second_keys)
  end

  def shared_keys
    first_keys.intersection(second_keys)
  end
end
```

### result

```rbs
class Source
  def first_keys: -> [:local, :global]
  def second_keys: -> [:global, :env]
  def merged_keys: -> Array[:env | :global | :local]
  def shared_keys: -> Array[:global]
end
```

## Array#uniq removes duplicate literals and keeps element type

### update

```ruby
def unique_keys
  [:local, :global, :local].uniq
end

def unique_names
  ["name", "path", "name"].uniq
end
```

### result

```rbs
class Object
  def unique_keys: -> Array[:global | :local]
  def unique_names: -> Array["name" | "path"]
end
```

## Destructive Array sort and uniq keep receiver element type

### update

```ruby
def unique_bang_keys
  values = [:local, :global, :local]
  result = values.uniq!
  [result, values]
end

def sort_by_bang_names
  values = ["long", "id"]
  result = values.sort_by! { |value| value.length }
  [result, values]
end

def reverse_bang_counts
  values = [1, 2, 3]
  result = values.reverse!
  [result, values]
end
```

### result

```rbs
class Object
  def unique_bang_keys: -> [Array[:global | :local]?, Array[:global | :local]]
  def sort_by_bang_names: -> [["long", "id"], Array["id" | "long"]]
  def reverse_bang_counts: -> [Array[1 | 2 | 3], Array[1 | 2 | 3]]
end
```

## `flatten` depth expands nested arrays step by step

### update

```ruby
def flatten_once
  [[1, [2]], [3, [4]]].flatten(1)
end

def flatten_twice
  [[1, [2]], [3, [4]]].flatten(2)
end

def flatten_negative
  [[1, [2]], [3]].flatten(-1)
end

def flatten_values_once
  { a: [[1], [2]], b: [[3]] }.values.flatten(1)
end

def flatten_compact_unique
  [1, [nil, 2], [2]].flatten.compact.uniq
end
```

### result

```rbs
class Object
  def flatten_once: -> Array[1 | 3 | [2] | [4]]
  def flatten_twice: -> Array[1 | 2 | 3 | 4]
  def flatten_negative: -> Array[1 | 2 | 3]
  def flatten_values_once: -> Array[[1] | [2] | [3]]
  def flatten_compact_unique: -> Array[1 | 2]
end
```

## `Hash#flatten` keeps static entry types

### update

```ruby
def flatten_record
  { name: "a", count: 1 }.flatten
end

def flatten_record_pairs
  { name: "a", count: 1 }.flatten(0)
end

def flatten_nested_values
  { name: ["a"], count: [1, [2]] }.flatten(2)
end

def flatten_all_values
  { name: ["a"], count: [1, [2]] }.flatten(-1)
end

def flatten_written_hash
  data = {}
  data[:name] = "a"
  data[:count] = 1
  data.flatten
end

def flatten_pair_keys
  { name: "a", count: 1 }.flatten(0).map(&:first)
end
```

### result

```rbs
class Object
  def flatten_record: -> Array[1 | "a" | :count | :name]
  def flatten_record_pairs: -> Array[[:count, 1] | [:name, "a"]]
  def flatten_nested_values: -> Array[1 | "a" | :count | :name | [2]]
  def flatten_all_values: -> Array[1 | 2 | "a" | :count | :name]
  def flatten_written_hash: -> Array[1 | "a" | :count | :name]
  def flatten_pair_keys: -> Array[:count | :name]
end
```

## Hash#compact removes nil values

### update

```ruby
def compact_record
  { name: "a", note: nil, active: false }.compact
end

def compact_record_with_nilable_value(flag)
  value = flag ? 1 : nil
  { value: value, count: 2 }.compact
end

def compact_hash_value_type
  values = {}
  values[:count] = 1
  values[:note] = nil
  values.compact
end

def compact_record_bang
  values = { name: "a", note: nil, active: false }
  result = values.compact!
  [result, values]
end
```

### result

```rbs
class Object
  def compact_record: -> { name: "a", active: false }
  def compact_record_with_nilable_value: (untyped flag) -> { ?value: 1, count: 2 }
  def compact_hash_value_type: -> Hash[:count | :note, 1]
  def compact_record_bang: -> [{ name: "a", active: false }?, { name: "a", active: false }]
end
```

## Destructive Array transform updates receiver element type

### update

```ruby
def map_bang_values
  values = ["1", "2"]
  result = values.map! { |value| value.to_i }
  [result, values]
end

def collect_bang_values
  values = ["a", "b"]
  result = values.collect!(&:to_sym)
  [result, values]
end
```

### result

```rbs
class Object
  def map_bang_values: -> [Array[Integer], Array[Integer]]
  def collect_bang_values: -> [Array[:a | :b], Array[:a | :b]]
end
```

## Destructive Array transforms destructure pair rows

### update

```ruby
def map_bang_pair_rows
  rows = [["left", 1], ["right", 2]]
  result = rows.map! do |name, count|
    [name.to_sym, count.to_s]
  end
  [result, rows]
end

def collect_bang_missing_rows
  rows = [[1], [2, "two"]]
  result = rows.collect! do |id, label|
    [id.to_s, label]
  end
  [result, rows]
end
```

### result

```rbs
class Object
  def map_bang_pair_rows: -> [Array[[:left | :right, String]], Array[[:left | :right, String]]]
  def collect_bang_missing_rows: -> [Array[[String, "two"?]], Array[[String, "two"?]]]
end
```

## Destructive Array filters narrow pair rows

### update

```ruby
def delete_if_pair_rows
  rows = [["one", 1], ["two", nil]]
  result = rows.delete_if do |_name, count|
    count.nil?
  end
  [result, rows]
end

def keep_if_pair_rows
  rows = [[1, "one"], [2, nil]]
  result = rows.keep_if do |_id, name|
    name
  end
  [result, rows]
end
```

### result

```rbs
class Object
  def delete_if_pair_rows: -> [Array[["one", 1]], Array[["one", 1]]]
  def keep_if_pair_rows: -> [Array[[1, "one"]], Array[[1, "one"]]]
end
```

## `flatten!` updates receiver element type

### update

```ruby
def flatten_bang_values
  values = [[1, [2]], [3]]
  result = values.flatten!(1)
  [result, values]
end

def flatten_bang_all
  values = [[1, [2]], [3]]
  result = values.flatten!
  [result, values]
end

def slice_bang_index
  values = ["a", 1, :b]
  result = values.slice!(1)
  [result, values]
end

def slice_bang_count
  values = ["a", 1, :b]
  result = values.slice!(1, 2)
  [result, values]
end

def slice_bang_range
  values = ["a", 1, :b, false]
  result = values.slice!(1...3)
  [result, values]
end

def slice_bang_missing
  values = ["a", 1]
  result = values.slice!(5)
  [result, values]
end
```

### result

```rbs
class Object
  def flatten_bang_values: -> [Array[1 | 3 | [2]]?, Array[1 | 3 | [2]]]
  def flatten_bang_all: -> [Array[1 | 2 | 3]?, Array[1 | 2 | 3]]
  def slice_bang_index: -> [1, ["a", :b]]
  def slice_bang_count: -> [[1, :b], ["a"]]
  def slice_bang_range: -> [[1, :b], ["a", false]]
  def slice_bang_missing: -> [nil, ["a", 1]]
end
```

## Destructive Array filter narrows receiver element type

### update

```ruby
class Entry
end

class Other
end

def delete_nil_values
  values = [1, nil, 2]
  result = values.delete_if(&:nil?)
  [result, values]
end

def keep_truthy_values
  values = [1, false, nil, 2]
  result = values.keep_if(&:itself)
  [result, values]
end

def select_entry_values
  values = [Entry.new, Other.new]
  result = values.select! { |value| value.is_a?(Entry) }
  [result, values]
end

def compact_values
  values = [1, nil, 2]
  result = values.compact!
  [result, values]
end
```

### result

```rbs
class Object
  def delete_nil_values: -> [Array[1 | 2], Array[1 | 2]]
  def keep_truthy_values: -> [Array[1 | 2], Array[1 | 2]]
  def select_entry_values: -> [Array[Entry]?, Array[Entry]]
  def compact_values: -> [Array[1 | 2]?, Array[1 | 2]]
end
```

## `[]=` expression returns assigned value

### update

```ruby
class MatrixLike
  def []=(*args, axis: nil)
    axis
  end
end

class A
  def plain
    matrix = MatrixLike.new
    matrix[5] = 8
  end

  def keyword
    matrix = MatrixLike.new
    matrix[5, axis: :y] = 8
  end
end
```

### result

```rbs
class A
  def plain: -> 8
  def keyword: -> 8
end

class MatrixLike
  def []=: (*Integer args, ?axis: Symbol?) -> Symbol?
end
```

## `[]=` expression with block arg returns assigned value

### update

```ruby
class MatrixLike
  def []=(*args, &block)
    yield if block
    :from_method
  end
end

class A
  def with_block
    block = -> { 1 }
    matrix = MatrixLike.new
    matrix[5, &block] = 8
  end
end
```

### result

```rbs
class A
  def with_block: -> 8
end

class MatrixLike
  def []=: (*Integer args, ?untyped &block) -> :from_method
end
```

## Hash#fetch and dig read exact record shape

### update

```ruby
def fetch_existing = { name: "Ada", count: 3 }.fetch(:name)

def fetch_default = { name: "Ada" }.fetch(:missing, "fallback")

def fetch_block = { name: "Ada" }.fetch(:missing) { |key| key.to_s }

def dig_record = { user: { name: "Ada", age: 3 } }.dig(:user, :name)

def dig_array = { flags: [:a, :b] }.dig(:flags, 0)
```

### result

```rbs
class Object
  def fetch_existing: -> "Ada"
  def fetch_default: -> "fallback"
  def fetch_block: -> String
  def dig_record: -> "Ada"
  def dig_array: -> :a
end
```

## Hash#slice except and values_at keep exact record shape

### update

```ruby
def slice_record = { name: "Ada", count: 3, enabled: true }.slice(:name, :count)

def except_record = { name: "Ada", count: 3, enabled: true }.except(:count)

def values_at_record = { name: "Ada", count: 3 }.values_at(:name, :missing)
```

### result

```rbs
class Object
  def slice_record: -> { name: "Ada", count: 3 }
  def except_record: -> { name: "Ada", enabled: true }
  def values_at_record: -> ["Ada", nil]
end
```

## Hash#delete updates exact record shape

### update

```ruby
def delete_record_key
  options = { name: "Ada", count: 3, enabled: true }
  name = options.delete(:name)
  [name, options]
end

def delete_missing_key
  options = { name: "Ada" }
  fallback = options.delete(:count) { |key| key.to_s }
  [fallback, options]
end

def delete_dynamic_key(key)
  options = { name: "Ada", count: 3 }
  value = options.delete(key)
  [value, options]
end
```

### result

```rbs
class Object
  def delete_record_key: -> ["Ada", { count: 3, enabled: true }]
  def delete_missing_key: -> [String, { name: "Ada" }]
  def delete_dynamic_key: (untyped key) -> [(3 | "Ada")?, { name: "Ada", count: 3 }]
end
```

## Hash#merge combines exact record shapes

### update

```ruby
def merge_record = { name: "Ada", count: 3 }.merge(name: "Grace", enabled: true)

def merge_many = { name: "Ada" }.merge({ count: 3 }, enabled: true)

def merge_conflict_block = { count: 1 }.merge(count: 2) { |key, old, new| old.to_s }
```

### result

```rbs
class Object
  def merge_record: -> { name: "Grace", count: 3, enabled: true }
  def merge_many: -> { name: "Ada", count: 3, enabled: true }
  def merge_conflict_block: -> { count: String }
end
```

## Hash destructive writers update receiver and return shape

### update

```ruby
class Item
  def initialize(name) = @name = name
  attr_reader :name
end

class A
  def store_record
    data = {}
    value = data.store(:item, Item.new("a"))
    [value.name, data[:item].name, data]
  end

  def merge_bang
    data = { name: "a" }
    result = data.merge!(count: 1)
    [result, data]
  end

  def merge_bang_block
    data = { count: 1 }
    result = data.merge!(count: 2) { |key, old, new| old.to_s }
    [result, data[:count]]
  end

  def update_record
    data = { name: "a", count: 1 }
    result = data.update(name: "b")
    [result, data[:name]]
  end

  def replace_record
    data = { name: "a", count: 1 }
    result = data.replace(enabled: true)
    [result, data[:enabled], data[:name]]
  end

  def nested_store
    data = { items: {} }
    data[:items].store(:first, Item.new("a"))
    data[:items][:first].name
  end
end
```

### result

```rbs
class A
  def store_record: -> ["a", "a", Hash[:item, Item]]
  def merge_bang: -> [{ name: "a", count: 1 }, { name: "a", count: 1 }]
  def merge_bang_block: -> [{ count: String }, String]
  def update_record: -> [{ name: "b", count: 1 }, "b"]
  def replace_record: -> [{ enabled: true }, true, nil]
  def nested_store: -> "a"
end

class Item
  def initialize: (String name) -> void
  def name: -> "a"
end
```

## Hash helpers expand statically known splat keys

### update

```ruby
SYMBOL_KEYS = [:name, :enabled]
STRING_KEYS = ["name", "count"]

def slice_with_symbol_key_splat
  data = { name: "Ada", count: 3, enabled: true }
  data.slice(*SYMBOL_KEYS)
end

def values_at_with_local_key_splat
  data = { name: "Ada", count: 3, enabled: true }
  keys = [:enabled, :missing]
  data.values_at(*keys)
end

def fetch_values_with_key_splat
  data = { name: "Ada", count: 3, enabled: true }
  keys = [:count, :name]
  data.fetch_values(*keys)
end

def fetch_values_with_block
  data = { name: "Ada" }
  data.fetch_values(:name, :count) { |key| key.to_s }
end

def fetch_values_string_key_block
  data = { "name" => "Ada" }
  keys = ["name", "count"]
  data.fetch_values(*keys) { |key| key.to_sym }
end

def empty_hash_fetch_values
  { name: "Ada" }.fetch_values
end

def except_with_string_key_splat
  data = { "name" => "Ada", "count" => 3, "enabled" => true }
  data.except(*STRING_KEYS)
end
```

### result

```rbs
SYMBOL_KEYS: [:name, :enabled]
STRING_KEYS: ["name", "count"]

class Object
  def slice_with_symbol_key_splat: -> { name: "Ada", enabled: true }
  def values_at_with_local_key_splat: -> [true, nil]
  def fetch_values_with_key_splat: -> [3, "Ada"]
  def fetch_values_with_block: -> ["Ada", String]
  def fetch_values_string_key_block: -> ["Ada", :count]
  def empty_hash_fetch_values: -> [ ]
  def except_with_string_key_splat: -> { "enabled" => true }
end
```

## assoc / rassoc recover static pairs

### update

```ruby
PAIRS = [["first", 1], ["second", 2]]
CHOICES = [["monday", 1], ["tuesday", 2]]

def assoc_pair = PAIRS.assoc("first")

def rassoc_pair = PAIRS.rassoc(2)

def missing_pair = PAIRS.assoc("missing")

def dynamic_pair(name)
  PAIRS.assoc(name)
end

def record_assoc_pair = { name: "Ada", count: 3 }.assoc(:name)

def record_rassoc_pair = { name: "Ada", count: 3 }.rassoc(3)

def record_missing_pair = { name: "Ada" }.rassoc(3)

def choice_name = CHOICES.rassoc(1)[0]
```

### result

```rbs
PAIRS: [["first", 1], ["second", 2]]
CHOICES: [["monday", 1], ["tuesday", 2]]

class Object
  def assoc_pair: -> ["first", 1]
  def rassoc_pair: -> ["second", 2]
  def missing_pair: -> nil
  def dynamic_pair: (untyped name) -> (["first", 1] | ["second", 2])?
  def record_assoc_pair: -> [:name, "Ada"]
  def record_rassoc_pair: -> [:count, 3]
  def record_missing_pair: -> nil
  def choice_name: -> "monday"
end
```

## Hash#key and index recover static keys

### update

```ruby
LEVELS = { low: 1, high: 2 }

class Lookup
  STATES = { ready: "r", done: "d" }

  def self.state_for(value) = STATES.key(value)
end

def record_key_lookup = { name: "Ada", count: 3 }.key(3)

def record_index_lookup = { queued: "q", done: "d" }.index("d")

def missing_key_lookup = { name: "Ada" }.key("Grace")

def dynamic_key_lookup(value) = { enabled: true, disabled: false }.key(value)

def constant_key_lookup = LEVELS.key(2)

def project_key_lookup = Lookup.state_for("d")
```

### result

```rbs
LEVELS: { low: 1, high: 2 }

class Lookup
  STATES: { ready: "r", done: "d" }

  def self.state_for: (String value) -> (:done | :ready)?
end

class Object
  def record_key_lookup: -> :count
  def record_index_lookup: -> :done
  def missing_key_lookup: -> nil
  def dynamic_key_lookup: (untyped value) -> (:disabled | :enabled)?
  def constant_key_lookup: -> :high
  def project_key_lookup: -> (:done | :ready)?
end
```

## Static membership predicates

### update

```ruby
KEYS = [:name, :count]
TABLE = { name: "Ada", count: 3 }
WORDS = ["one", "two"].map(&:upcase)

def tuple_includes_key = KEYS.include?(:name)

def tuple_missing_key = KEYS.member?(:missing)

def array_disjoint_member = WORDS.include?(:name)

def hash_has_key = TABLE.key?(:name)

def hash_has_missing_key = TABLE.has_key?(:missing)

def hash_includes_string_key = { "name" => "Ada" }.include?("name")

def hash_has_value = TABLE.value?("Ada")

def hash_has_missing_value = TABLE.has_value?(:missing)

def record_not_empty = TABLE.empty?

def tuple_empty = [].empty?
```

### result

```rbs
KEYS: [:name, :count]
TABLE: { name: "Ada", count: 3 }
WORDS: Array["ONE" | "TWO"]

class Object
  def tuple_includes_key: -> true
  def tuple_missing_key: -> false
  def array_disjoint_member: -> false
  def hash_has_key: -> true
  def hash_has_missing_key: -> false
  def hash_includes_string_key: -> true
  def hash_has_value: -> true
  def hash_has_missing_value: -> false
  def record_not_empty: -> false
  def tuple_empty: -> true
end
```

## Index into widened arrays is nilable but tuples stay exact

### update

```ruby
class IndexAccess
  def ints = [1, 2, 3].map { |x| x * 2 }

  def pair = [1, 'a']

  def widened_literal_index = ints[0]

  def widened_dynamic_index(i)
    j = i.to_i
    ints[j]
  end

  def widened_at = ints.at(0)

  def widened_fetch = ints.fetch(0)

  def tuple_first_elem = pair[0]

  def tuple_last_elem = pair[-1]

  def tuple_out_of_range = pair[5]

  def tuple_dynamic_index(i)
    j = i.to_i
    pair[j]
  end
end
```

### result

```rbs
class IndexAccess
  def ints: -> Array[Integer]
  def pair: -> [1, "a"]
  def widened_literal_index: -> Integer?
  def widened_dynamic_index: (untyped i) -> Integer?
  def widened_at: -> Integer?
  def widened_fetch: -> Integer
  def tuple_first_elem: -> 1
  def tuple_last_elem: -> "a"
  def tuple_out_of_range: -> nil
  def tuple_dynamic_index: (untyped i) -> (1 | "a")?
end
```

## Splat and double-splat expand inside literals

### update

```ruby
class Builder
  def numbers
    rest = [2, 3]
    [1, *rest, 4]
  end

  def merge_arrays
    [*[1, 2], *[3, 4]]
  end

  def options
    defaults = { timeout: 30 }
    { **defaults, retries: 3 }
  end

  def override
    { **{ level: :info }, level: :debug }
  end
end
```

### result

```rbs
class Builder
  def numbers: -> [1, 2, 3, 4]
  def merge_arrays: -> [1, 2, 3, 4]
  def options: -> { timeout: 30, retries: 3 }
  def override: -> { level: :debug }
end
```

## Indexed or-assign then push records the key

### update

```ruby
def build
  params = {}
  params[:f] ||= []
  params[:f] << :status
  params
end
```

### result

```rbs
class Object
  def build: -> Hash[:f, Array[:status]]
end
```

## deconstruct_keys then key? on a closed record

### update

```ruby
def has_a = { a: 1 }.deconstruct_keys(nil).key?(:a)
```

### result

```rbs
class Object
  def has_a: -> true
end
```

## Array compact drops nil

### update

```ruby
def foo = [1, nil, true].compact
```

### result

```rbs
class Object
  def foo: -> [1, true]
end
```

## invert swaps a two-entry record into a Hash

### update

```ruby
def flipped = { a: 1, b: 2 }.invert
```

### result

```rbs
class Object
  def flipped: -> Hash[1 | 2, :a | :b]
end
```

## Double append splat widens the array

### update

```ruby
class A
  def gen = [1]

  def check
    ary = []
    ary.append(*gen)
    ary.append(*gen)
  end
end
```

### result

```rbs
class A
  def gen: -> [1]
  def check: -> [1, 1]
end
```

## Hash splat of a method return merges keys

### update

```ruby
def foo = { **bar, b: 1 }

def bar = { a: 1 }
```

### result

```rbs
class Object
  def foo: -> { a: 1, b: 1 }
  def bar: -> { a: 1 }
end
```
