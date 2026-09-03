# Ruby / Method / Blocks / Enumerable Core

## map with type transformation

### update

```ruby
def int_to_string = [1, 2, 3].map { |x| x.to_s }
```

### result

```rbs
class Object
  def int_to_string: -> Array[String]
end
```

## Map distributes literal operations across mixed tuple elements

### update

```ruby
def mixed_literal_map = [1, 2, "3"].map { |n| n * 2 }
```

### result

```rbs
class Object
  def mixed_literal_map: -> Array[Integer | String]
end
```

## select preserving element type

### update

```ruby
def filter_ints = [1, 2, 3].select { |x| x > 1 }
```

### result

```rbs
class Object
  def filter_ints: -> Array[1 | 2 | 3]
end
```

## Use map and select block signatures for side effects

### update

```ruby
def map_side_effect_values
  values = []
  [1, 2].map { |value| values << value }
  values
end

def numbered_map_side_effect_values
  values = []
  [1, 2].map { values << _1 }
  values
end

def select_side_effect_values
  values = []
  [1, 2].select { |value| values << value; true }
  values
end
```

### result

```rbs
class Object
  def map_side_effect_values: -> Array[1 | 2]
  def numbered_map_side_effect_values: -> Array[1 | 2]
  def select_side_effect_values: -> Array[1 | 2]
end
```

## Use Range and Set block signatures for side effects

### update

```ruby
def range_each_values
  values = []
  (1..3).each { |value| values << value }
  values
end

def range_reverse_values
  values = []
  (1..3).reverse_each { |value| values << value }
  values
end

def string_range_values
  values = []
  ("a".."c").each { |value| values << value }
  values
end

def set_each_values
  values = []
  Set[1, 2].each { |value| values << value }
  values
end

def set_with_object_map
  Set[:a, :b].each_with_object({}) { |key, map| map[key] = key.to_s }
end
```

### result

```rbs
class Object
  def range_each_values: -> Array[Integer]
  def range_reverse_values: -> Array[Integer]
  def string_range_values: -> Array[String]
  def set_each_values: -> Array[1 | 2]
  def set_with_object_map: -> Hash[:a | :b, String]
end
```

## Use Hash iterator block signatures for side effects

### update

```ruby

def hash_key_side_effect_values
  keys = []
  { name: "one", count: 2 }.each_key { |key| keys << key }
  keys
end

def hash_value_side_effect_values
  values = []
  { name: "one", count: 2 }.each_value { |value| values << value }
  values
end

def fetch_key_side_effect_values
  keys = []
  { name: "one" }.fetch(:count) { |key| keys << key }
  keys
end

def fetch_existing_key_side_effect_values
  keys = []
  { name: "one" }.fetch(:name) { |key| keys << key }
  keys
end

def fetch_values_key_side_effect_values
  keys = []
  { name: "one" }.fetch_values(:count) { |key| keys << key }
  keys
end

def fetch_values_existing_key_side_effect_values
  keys = []
  { name: "one" }.fetch_values(:name) { |key| keys << key }
  keys
end
```

### result

```rbs
class Object
  def hash_key_side_effect_values: -> Array[:count | :name]
  def hash_value_side_effect_values: -> Array[2 | "one"]
  def fetch_key_side_effect_values: -> Array[:count]
  def fetch_existing_key_side_effect_values: -> [ ]
  def fetch_values_key_side_effect_values: -> Array[:count]
  def fetch_values_existing_key_side_effect_values: -> [ ]
end
```

## each returning self

### update

```ruby
def each_returns_self = [1, 2, 3].each { |x| puts x }
```

### result

```rbs
class Object
  def each_returns_self: -> [1, 2, 3]
end
```

## find returning element | nil

### update

```ruby
def find_int = [1, 2, 3].find { |x| x > 1 }
```

### result

```rbs
class Object
  def find_int: -> (1 | 2 | 3)?
end
```

## find with ifnone proc

### update

```ruby
class F
  def find_ifnone
    [1, 2].find(-> { "x" }) { |n| n > 10 }
  end
end
```

### result

```rbs
class F
  def find_ifnone: -> 1 | 2 | "x"
end
```

## find and detect narrow with class predicate

### update

```ruby
class Entry
end

class Note < Entry
end

class Flag < Entry
end

class Other
end

def find_entry
  [Note.new, Other.new, Flag.new].find { |value| value.is_a?(Entry) }
end

def detect_non_entry
  [Note.new, Other.new, Flag.new].detect { |value| !value.kind_of?(Entry) }
end

def find_entry_or_default
  [Note.new, Other.new].find(-> { :none }) { |value| value.is_a?(Entry) }
end
```

### result

```rbs
class Object
  def find_entry: -> (Flag | Note)?
  def detect_non_entry: -> Other?
  def find_entry_or_default: -> :none | Note
end
```

## find narrows with module predicate

### update

```ruby
module Taggable
end

class Article
  include Taggable
end

class Event
  include Taggable
end

class Plain
end

def find_taggable_value
  [Article.new, Plain.new, Event.new].find { |value| value.kind_of?(Taggable) }
end
```

### result

```rbs
class Article
  include Taggable
end

class Event
  include Taggable
end

class Object
  def find_taggable_value: -> (Article | Event)?
end
```

## detect narrows with constant alias predicate

### update

```ruby
module Group
  class Base
  end

  class Item < Base
  end

  ItemAlias = Item
end

def detect_project_item
  [Group::Item.new, Object.new].detect { |value| value.is_a?(Group::ItemAlias) }
end
```

### result

```rbs
module Group
  ItemAlias: singleton(Group::Item)
end

class Object
  def detect_project_item: -> Group::Item?
end
```

## find and detect narrow with nil and truthy predicate

### update

```ruby
def find_non_nil_value
  [nil, "a", :b].find { |value| !value.nil? }
end

def detect_nil_value
  [1, nil, 2].detect(&:nil?)
end

def find_truthy_value
  [false, nil, "x"].find(&:itself)
end
```

### result

```rbs
class Object
  def find_non_nil_value: -> ("a" | :b)?
  def detect_nil_value: -> nil
  def find_truthy_value: -> "x"?
end
```

## Resolve block return for bsearch and find_index

### update

```ruby
def range_bsearch
  (0...10).bsearch { |index| index >= 3 }
end

def array_bsearch
  [1, 2, 3].bsearch { |value| value >= 2 }
end

def array_bsearch_index
  [1, 2, 3].bsearch_index { |value| value >= 2 }
end

def line_find_index
  ["one", "two", "three"].find_index { |line| line.length > 3 }
end

def entry_find_index
  { one: 1, two: 2 }.find_index { |key, value| key == :two && value > 1 }
end
```

### result

```rbs
class Object
  def range_bsearch: -> Integer?
  def array_bsearch: -> (1 | 2 | 3)?
  def array_bsearch_index: -> Integer?
  def line_find_index: -> Integer?
  def entry_find_index: -> Integer?
end
```

## Resolve window type for slice and chunk iterators

### update

```ruby

def slice_before_groups
  ["a", "bb", "ccc"].slice_before { |line| line.length > 1 }.to_a
end

def slice_after_heads
  ["a", "bb", "ccc"].slice_after { |line| line.length > 1 }.map(&:first)
end

def slice_when_groups
  [1, 2, 4, 5].slice_when { |left, right| right - left > 1 }.to_a
end

def chunk_while_groups
  [1, 2, 4, 5].chunk_while { |left, right| right - left == 1 }.to_a
end
```

### result

```rbs
class Object
  def slice_before_groups: -> Array[Array["a" | "bb" | "ccc"]]
  def slice_after_heads: -> Array[("a" | "bb" | "ccc")?]
  def slice_when_groups: -> Array[Array[1 | 2 | 4 | 5]]
  def chunk_while_groups: -> Array[Array[1 | 2 | 4 | 5]]
end
```

## reduce with initial value

### update

```ruby
def sum_reduce = [1, 2, 3].reduce(0) { |sum, x| sum + x }
```

### result

```rbs
class Object
  def sum_reduce: -> Integer
end
```

## Recover Array accumulator in inject from block return

### update

```ruby
def collect_values
  [1, 2, 3].inject([]) do |list, value|
    list << value.to_s
    list
  end
end
```

### result

```rbs
class Object
  def collect_values: -> Array[String]
end
```

## concat into Array accumulator in reduce

### update

```ruby
def collect_nested_values
  [[1, 2], [3]].reduce([]) do |list, values|
    list.concat(values)
    list
  end
end
```

### result

```rbs
class Object
  def collect_nested_values: -> Array[1 | 2 | 3]
end
```

## Write dynamic key to Hash accumulator in inject

### update

```ruby
def build_table
  ["a", "bb"].inject({}) do |table, name|
    table[name] = name.length
    table
  end
end
```

### result

```rbs
class Object
  def build_table: -> Hash["a" | "bb", 1 | 2]
end
```

## Update existing accumulator alias in inject

### update

```ruby
def collect_into_result
  result = []
  [1, 2, 3].inject(result) do |list, value|
    list << value.to_s
    list
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

## any?/all?/none?

### update

```ruby
def check_any = [1, 2, 3].any? { |x| x > 2 }

def check_all = [1, 2, 3].all? { |x| x > 0 }

def check_none = [1, 2, 3].none? { |x| x > 5 }
```

### result

```rbs
class Object
  def check_any: -> bool
  def check_all: -> bool
  def check_none: -> bool
end
```

## Method chaining: select then map

### update

```ruby
def chain_select_map = [1, 2, 3].select { |x| x > 1 }.map { |x| x.to_s }
```

### result

```rbs
class Object
  def chain_select_map: -> Array[String]
end
```

## tap preserves receiver type

### update

```ruby
def use_tap = "hello".tap { |s| puts s }
```

### result

```rbs
class Object
  def use_tap: -> "hello"
end
```

## tap propagates mutable receiver side effects

### update

```ruby
def build_hash_with_tap
  {}.tap do |hash|
    hash[:name] = "item"
    hash[:count] = 1
  end
end

def extend_record_with_tap
  { name: "item" }.tap do |hash|
    hash[:count] = 1
  end
end

def build_array_with_tap
  [].tap do |values|
    values << "item"
    values << 1
  end
end

def update_local_with_tap
  values = []
  values.tap { |list| list << :item }
  values
end
```

### result

```rbs
class Object
  def build_hash_with_tap: -> Hash[:count | :name, 1 | "item"]
  def extend_record_with_tap: -> { name: "item", count: 1 }
  def build_array_with_tap: -> Array[1 | "item"]
  def update_local_with_tap: -> Array[:item]
end
```

## then/yield_self transforms type

### update

```ruby
def use_then = "hello".then { |s| s.length }
```

### result

```rbs
class Object
  def use_then: -> 5
end
```

## count with block

### update

```ruby
def count_positives = [1, -2, 3].count { |x| x > 0 }
```

### result

```rbs
class Object
  def count_positives: -> Integer
end
```

## group_by

### update

```ruby
def group_by_parity = [1, 2, 3, 4].group_by { |x| x > 2 }
```

### result

```rbs
class Object
  def group_by_parity: -> Hash[bool, Array[1 | 2 | 3 | 4]]
end
```

## sort_by

### update

```ruby
def sort_desc = [3, 1, 2].sort_by { |x| x }
```

### result

```rbs
class Object
  def sort_desc: -> Array[1 | 2 | 3]
end
```

## sum without block

### update

```ruby
def sum_ints = [1, 2, 3].sum
```

### result

```rbs
class Object
  def sum_ints: -> Integer
end
```

## sum of float array is Float

### update

```ruby
def float_sum = [1.5, 2.5].sum
def mixed_sum = [1, 2.5].sum
def empty_sum = [].sum
```

### result

```rbs
class Object
  def float_sum: -> Float
  def mixed_sum: -> Float
  def empty_sum: -> Integer
end
```

## flat_map

### update

```ruby
def flat_map_example = [[1, 2], [3, 4]].flat_map { |a| a }
```

### result

```rbs
class Object
  def flat_map_example: -> Array[1 | 2 | 3 | 4]
end
```

## tuple union block destructuring

### update

```ruby
def map_entry_pairs
  [[1, "a"], [2, "b"]].map { |id, name| [name, id] }
end

def flat_map_entry_pairs
  [[1, "a"], [2, "b"]].flat_map { |id, name| [id, name] }
end

def missing_entry_tail
  [[1], [2, 3]].map { |id, value| [id, value] }
end
```

### result

```rbs
class Object
  def map_entry_pairs: -> Array[["a" | "b", 1 | 2]]
  def flat_map_entry_pairs: -> Array[1 | 2 | "a" | "b"]
  def missing_entry_tail: -> Array[[1 | 2, 3?]]
end
```

## min_by/max_by

### update

```ruby
def find_min_by = [3, 1, 2].min_by { |x| x }
```

### result

```rbs
class Object
  def find_min_by: -> (1 | 2 | 3)?
end
```

## reject

### update

```ruby
def reject_small = [1, 2, 3].reject { |x| x < 2 }
```

### result

```rbs
class Object
  def reject_small: -> [1, 2, 3]
end
```

## each_entry

### update

```ruby
def each_entry_rows
  rows = []
  { name: "one", count: 2 }.each_entry { |row| rows << row }
  rows
end

def each_entry_pairs
  { name: "one", count: 2 }.each_entry.map { |key, value| [key, value] }
end

def array_each_entry
  ["a", "bb"].each_entry.map(&:length)
end
```

### result

```rbs
class Object
  def each_entry_rows: -> Array[[:count | :name, 2 | "one"]]
  def each_entry_pairs: -> Array[[:count | :name, 2 | "one"]]
  def array_each_entry: -> Array[1 | 2]
end
```

## map over a literal tuple widens to Array of String

### update

```ruby
def stringified = [1, 2, 3].map { |n| n.to_s }
```

### result

```rbs
class Object
  def stringified: -> Array[String]
end
```

## numbered-parameter map widens integer increments

### update

```ruby
def shifted = [1, 2, 3].map { _1 + 1 }
```

### result

```rbs
class Object
  def shifted: -> Array[Integer]
end
```

## filter_map with even? keeps stringified survivors

### update

```ruby
def evens = [1, 2, 3].filter_map { |n| n.even? ? n.to_s : nil }
```

### result

```rbs
class Object
  def evens: -> Array[String]
end
```

## flat_map with branch-dependent sizes unions the elements

### update

```ruby
def flat_varied = [1, 2].flat_map { |n| n.even? ? [n, n] : [n] }
```

### result

```rbs
class Object
  def flat_varied: -> Array[1 | 2]
end
```

## flat_map of a scalar block widens to Array of String

### update

```ruby
def flat_strings = [1, 2, 3].flat_map { |n| n.to_s }
```

### result

```rbs
class Object
  def flat_strings: -> Array[String]
end
```

## Block accumulator from each push

### update

```ruby
def foo(arr)
  result = []
  arr.each { |x| result << x * 2 }
  result
end

foo([1, 2, 3])
```

### result

```rbs
class Object
  def foo: (Array[Integer] arr) -> Array[Integer]
end
```

## find on a literal tuple returns a nilable element union

### update

```ruby
def first_even = [1, 2, 3, 4].find { |n| n.even? }
```

### result

```rbs
class Object
  def first_even: -> (1 | 2 | 3 | 4)?
end
```

## find_index on a literal tuple returns a nilable Integer

### update

```ruby
def idx_first_even = [1, 2, 3, 4].find_index { |n| n.even? }
```

### result

```rbs
class Object
  def idx_first_even: -> Integer?
end
```

## Same block passed twice to each

### update

```ruby
def foo(&blk)
  obj = [1]
  obj.each(&blk)
  obj.each(&blk)
end

foo { |x| x }
```

### result

```rbs
class Object
  def foo: (?untyped &blk) -> [1]
end
```

## Block next excludes a dead until-true

### update

```ruby
def bar
  yield
  nil
end

def foo
  bar do
    next :a
    until true
      next :b
    end
    next :c
  end
end
```

### result

```rbs
class Object
  def bar: -> nil
  def foo: -> nil
end
```
