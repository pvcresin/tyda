# Ruby / Method / Blocks / Narrowing

## filter_map

### update

```ruby
def test_filter_map
  [1, nil, 2].filter_map { |value| value }
end

def test_filter_map_false
  [1, false, 2].filter_map { |value| value }
end
```

### result

```rbs
class Object
  def test_filter_map: -> Array[1 | 2]
  def test_filter_map_false: -> Array[1 | 2]
end
```

## to_h with block pair

### update

```ruby
def test_to_h_block
  [1, 2].to_h { |value| [value.to_s, value] }
end
```

### result

```rbs
class Object
  def test_to_h_block: -> Hash[String, 1 | 2]
end
```

## Array#zip block and pair chains

### update

```ruby
def zipped_pairs
  [1, 2].zip(["a", "b"])
end

def zipped_pairs_to_hash
  ["a", "b"].zip([1, 2]).to_h
end

def zipped_splat_rows
  rows = [["a", "b"], [1, 2], [:x, :y]]
  rows[0].zip(*rows[1..])
end

def zipped_splat_to_hash
  rows = [[:a, :b], [1, 2]]
  rows.first.zip(*rows.drop(1)).to_h
end

def collect_zipped_pairs
  result = []
  [1, 2].zip(["a", "b"]) do |number, label|
    result << [label, number]
  end
  result
end

def collect_zipped_heads
  result = []
  [1, 2].zip(["a", "b"]) do |pair|
    result << pair.first
  end
  result
end

def collect_zipped_splat
  rows = [[1, 2], ["a", "b"], [true, false]]
  result = []
  rows.first.zip(*rows.drop(1)) do |id, name, enabled|
    result << [name, id, enabled]
  end
  result
end
```

### result

```rbs
class Object
  def zipped_pairs: -> Array[[1 | 2, ("a" | "b")?]]
  def zipped_pairs_to_hash: -> Hash["a" | "b", (1 | 2)?]
  def zipped_splat_rows: -> Array[["a" | "b", (1 | 2)?, (:x | :y)?]]
  def zipped_splat_to_hash: -> Hash[:a | :b, (1 | 2)?]
  def collect_zipped_pairs: -> Array[[("a" | "b")?, 1 | 2]]
  def collect_zipped_heads: -> Array[1 | 2]
  def collect_zipped_splat: -> Array[[("a" | "b")?, 1 | 2, bool?]]
end
```

## partition splits element type into two arrays

### update

```ruby
def split_values
  [1, 2, 3].partition { |value| value > 1 }
end
```

### result

```rbs
class Object
  def split_values: -> [Array[1 | 2 | 3], Array[1 | 2 | 3]]
end
```

## grep preserves element type and block return type

### update

```ruby
def grep_values
  ["a", "bb", "ccc"].grep(/b/)
end

def grep_lengths
  ["a", "bb", "ccc"].grep(/b/) { |value| value.length }
end
```

### result

```rbs
class Object
  def grep_values: -> Array["a" | "bb" | "ccc"]
  def grep_lengths: -> Array[1 | 2 | 3]
end
```

## grep narrows nil and bool literal patterns

### update

```ruby
def grep_non_nil_values
  [1, nil, 2].grep_v(nil)
end

def grep_non_nil_values_by_class
  [1, nil, 2].grep_v(NilClass)
end

def grep_nil_values_by_class
  [1, nil, 2].grep(NilClass)
end

def grep_true_values
  [true, false, nil].grep(true)
end

def grep_not_false_values
  [true, false, nil].grep_v(false)
end
```

### result

```rbs
class Object
  def grep_non_nil_values: -> Array[1 | 2]
  def grep_non_nil_values_by_class: -> Array[1 | 2]
  def grep_nil_values_by_class: -> Array[nil]
  def grep_true_values: -> Array[true]
  def grep_not_false_values: -> Array[true | nil]
end
```

## grep narrows regexp-compatible values

### update

```ruby
def grep_text_values
  ["name", :token, 1, nil].grep(/n/)
end

def grep_text_lengths
  ["name", :token, 1].grep(/n/) { |value| value.to_s.length }
end

def grep_text_values_with_local
  pattern = /n/
  ["name", :token, 1].grep(pattern)
end

def grep_text_values_with_constructor
  pattern = Regexp.new("n")
  ["name", :token, 1].grep(pattern)
end

TEXT_PATTERN = /n/

module PatternSet
  VALUE = /n/
end

def grep_text_values_with_constant
  ["name", :token, 1].grep(TEXT_PATTERN)
end

def grep_text_values_with_nested_constant
  ["name", :token, 1].grep(PatternSet::VALUE)
end

def grep_text_values_with_compile
  ["name", :token, 1].grep(Regexp.compile("n"))
end

def grep_text_values_with_union
  ["name", :token, 1].grep(Regexp.union("n", /^to/))
end

def grep_v_text_values
  ["name", :token, 1].grep_v(/n/)
end
```

### result

```rbs
TEXT_PATTERN: Regexp

class Object
  def grep_text_values: -> Array["name" | :token]
  def grep_text_lengths: -> Array[Integer]
  def grep_text_values_with_local: -> Array["name" | :token]
  def grep_text_values_with_constructor: -> Array["name" | :token]
  def grep_text_values_with_constant: -> Array["name" | :token]
  def grep_text_values_with_nested_constant: -> Array["name" | :token]
  def grep_text_values_with_compile: -> Array["name" | :token]
  def grep_text_values_with_union: -> Array["name" | :token]
  def grep_v_text_values: -> Array[1 | "name" | :token]
end

module PatternSet
  VALUE: Regexp
end
```

## collection predicates narrow regexp-compatible values

### update

```ruby
def select_text_values
  ["name", :token, nil].select { |value| value =~ /n/ }
end

def find_text_value
  ["name", :token, nil].find { |value| /n/ === value }
end

def detect_text_value_with_match
  ["name", :token].detect { |value| value.match(/n/) }
end

def partition_text_values
  ["name", :token, nil].partition { |value| value.match?(/n/) }
end

def select_text_values_with_regexp_receiver
  ["name", :token, 1].select { |value| /n/ === value }
end

def select_text_values_with_constructor
  pattern = Regexp.new("n")
  ["name", :token, 1].select { |value| pattern === value }
end
```

### result

```rbs
class Object
  def select_text_values: -> Array["name" | :token]
  def find_text_value: -> ("name" | :token)?
  def detect_text_value_with_match: -> ("name" | :token)?
  def partition_text_values: -> [Array["name" | :token], Array[("name" | :token)?]]
  def select_text_values_with_regexp_receiver: -> Array["name" | :token]
  def select_text_values_with_constructor: -> Array["name" | :token]
end
```

## select and reject narrow nil predicate

### update

```ruby
def reject_nil_values
  [1, nil, 2].reject { |value| value.nil? }
end

def reject_nil_values_with_symbol_proc
  [1, nil, 2].reject(&:nil?)
end

def select_nil_values_with_symbol_proc
  [1, nil, 2].select(&:nil?)
end

def partition_nil_values_with_symbol_proc
  [1, nil, 2].partition(&:nil?)
end
```

### result

```rbs
class Object
  def reject_nil_values: -> Array[1 | 2]
  def reject_nil_values_with_symbol_proc: -> Array[1 | 2]
  def select_nil_values_with_symbol_proc: -> Array[nil]
  def partition_nil_values_with_symbol_proc: -> [Array[nil], Array[1 | 2]]
end
```

## filter and reject narrow truthy predicate

### update

```ruby
def filter_non_nil_values
  [1, nil, 2].filter { |value| !value.nil? }
end

def partition_non_nil_values
  [1, nil, 2].partition { |value| !value.nil? }
end

def filter_truthy_values
  [1, false, nil, 2].filter { |value| value }
end

def reject_truthy_values
  [1, false, nil, 2].reject { |value| value }
end

def partition_truthy_values
  [1, false, nil, 2].partition { |value| value }
end
```

### result

```rbs
class Object
  def filter_non_nil_values: -> Array[1 | 2]
  def partition_non_nil_values: -> [Array[1 | 2], Array[nil]]
  def filter_truthy_values: -> Array[1 | 2]
  def reject_truthy_values: -> Array[false | nil]
  def partition_truthy_values: -> [Array[1 | 2], Array[false | nil]]
end
```

## grep narrows class pattern

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

def grep_entries
  [Note.new, Flag.new, Other.new].grep(Entry)
end

def grep_non_entries
  [Note.new, Flag.new, Other.new].grep_v(Entry)
end
```

### result

```rbs
class Object
  def grep_entries: -> Array[Flag | Note]
  def grep_non_entries: -> Array[Other]
end
```

## grep block can call narrowed class method

### update

```ruby
class Entry
  def label = "entry"
end

class Note < Entry
  def label = "note"
end

class Flag < Entry
  def label = "flag"
end

class Other
end

def grep_entry_labels
  [Note.new, Flag.new, Other.new].grep(Entry) { |value| value.label }
end
```

### result

```rbs
class Entry
  def label: -> "entry"
end

class Flag < Entry
  def label: -> "flag"
end

class Note < Entry
  def label: -> "note"
end

class Object
  def grep_entry_labels: -> Array["flag" | "note"]
end
```

## select narrows class predicate

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

def select_notes
  [Note.new, Flag.new, Other.new].select { |value| value.is_a?(Note) }
end

def find_entry_values_with_numbered_param
  [Note.new, Flag.new, Other.new].find_all { _1.kind_of?(Entry) }
end

def select_non_entries
  [Note.new, Flag.new, Other.new].select { |value| !value.is_a?(Entry) }
end
```

### result

```rbs
class Object
  def select_notes: -> Array[Note]
  def find_entry_values_with_numbered_param: -> Array[Flag | Note]
  def select_non_entries: -> Array[Other]
end
```

## reject narrows class predicate

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

def reject_entries
  [Note.new, Flag.new, Other.new].reject { |value| value.is_a?(Entry) }
end

def reject_non_entries_with_numbered_param
  [Note.new, Flag.new, Other.new].reject { !_1.kind_of?(Entry) }
end
```

### result

```rbs
class Object
  def reject_entries: -> Array[Other]
  def reject_non_entries_with_numbered_param: -> Array[Flag | Note]
end
```

## partition narrows class predicate

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

def partition_entries
  [Note.new, Flag.new, Other.new].partition { |value| value.is_a?(Entry) }
end

def partition_non_entries_with_numbered_param
  [Note.new, Flag.new, Other.new].partition { !_1.kind_of?(Entry) }
end
```

### result

```rbs
class Object
  def partition_entries: -> [Array[Flag | Note], Array[Other]]
  def partition_non_entries_with_numbered_param: -> [Array[Other], Array[Flag | Note]]
end
```

## grep and filter narrow module predicate

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

def grep_taggable_values
  [Article.new, Event.new, Plain.new].grep(Taggable)
end

def filter_taggable_values
  [Article.new, Event.new, Plain.new].filter { |value| value.kind_of?(Taggable) }
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
  def grep_taggable_values: -> Array[Article | Event]
  def filter_taggable_values: -> Array[Article | Event]
end
```

## reject and partition narrow module predicate

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

def reject_taggable_values
  [Article.new, Event.new, Plain.new].reject { |value| value.kind_of?(Taggable) }
end

def partition_taggable_values
  [Article.new, Event.new, Plain.new].partition { |value| value.kind_of?(Taggable) }
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
  def reject_taggable_values: -> Array[Plain]
  def partition_taggable_values: -> [Array[Article | Event], Array[Plain]]
end
```

## select narrows respond_to? predicate

### update

```ruby
class NamedItem
  def name = "name"
end

class CountItem
  def count = 1
end

def select_named_items
  [NamedItem.new, CountItem.new].select { |item| item.respond_to?(:name) }.map(&:name)
end

def reject_named_items
  [NamedItem.new, CountItem.new].reject { |item| item.respond_to?("name") }.map(&:count)
end

def select_named_items_with_local
  method_name = :name
  [NamedItem.new, CountItem.new].select { |item| item.respond_to?(method_name) }.map(&:name)
end
```

### result

```rbs
class CountItem
  def count: -> 1
end

class NamedItem
  def name: -> "name"
end

class Object
  def select_named_items: -> Array["name"]
  def reject_named_items: -> Array[1]
  def select_named_items_with_local: -> Array["name"]
end
```

## find and partition narrow respond_to? predicate

### update

```ruby
module NamedValue
  def name = "name"
end

class NamedEntry
  include NamedValue
end

class CountEntry
  def count = 1
end

def find_named_entry
  [NamedEntry.new, CountEntry.new].find { |entry| entry.respond_to?(:name) }&.name
end

def partition_named_entries
  [NamedEntry.new, CountEntry.new].partition { |entry| entry.respond_to?(:name) }
end

def partition_count_entries
  [NamedEntry.new, CountEntry.new].partition { |entry| !entry.respond_to?(:count) }
end
```

### result

```rbs
class CountEntry
  def count: -> 1
end

class NamedEntry
  include NamedValue
end

module NamedValue
  def name: -> "name"
end

class Object
  def find_named_entry: -> "name"?
  def partition_named_entries: -> [Array[NamedEntry], Array[CountEntry]]
  def partition_count_entries: -> [Array[NamedEntry], Array[CountEntry]]
end
```

## select narrows project constant aliases

### update

```ruby
module Group
  class Base
  end

  class Item < Base
  end

  ItemAlias = Item
end

def select_project_items
  [Group::Item.new, Object.new].select { |value| value.is_a?(Group::ItemAlias) }
end

def grep_project_items
  [Group::Item.new, Object.new].grep(Group::ItemAlias)
end

def partition_project_items
  [Group::Item.new, Object.new].partition { |value| value.is_a?(Group::ItemAlias) }
end
```

### result

```rbs
module Group
  ItemAlias: singleton(Group::Item)
end

class Object
  def select_project_items: -> Array[Group::Item]
  def grep_project_items: -> Array[Group::Item]
  def partition_project_items: -> [Array[Group::Item], Array[Object]]
end
```
