# Ruby / Method / Blocks / Windows And Symbol Proc

## each_slice / each_cons preserve window element types

### update

```ruby
def slice_rows_to_hash
  [["a", 1], ["b", 2]].each_slice(2).map { |rows| rows.to_h }
end

def slice_positions_hash
  pairs = []
  [:a, 1, :b, 2].each_slice(2) do |key, value|
    pairs << [key, value]
  end
  pairs.to_h
end

def slice_missing_values
  pairs = []
  [:a, 1, :b].each_slice(2) do |key, value|
    pairs << [key, value]
  end
  pairs
end

def adjacent_windows
  windows = []
  [1, 2, 3].each_cons(2) do |window|
    windows << window
  end
  windows
end

def indexed_values_hash
  ["a", "b"].each.with_index.to_h
end
```

### result

```rbs
class Object
  def slice_rows_to_hash: -> Array[Hash["a" | "b", 1 | 2]]
  def slice_positions_hash: -> Hash[:a | :b, 1 | 2]
  def slice_missing_values: -> Array[[:a | :b, 1?]]
  def adjacent_windows: -> Array[Array[1 | 2 | 3]]
  def indexed_values_hash: -> Hash["a" | "b", Integer]
end
```

## Complex chain: select then map then reduce

### update

```ruby
def test_chain_select_map_reduce
  [1, 2, 3, 4, 5].select { |x| x > 2 }.map { |x| x.to_s }.reduce("") { |acc, s| acc + s }
end
```

### result

```rbs
class Object
  def test_chain_select_map_reduce: -> String
end
```

## Symbol-to-proc: map(&:to_s)

### update

```ruby
def test_symbol_map = [1, 2, 3].map(&:to_s)
```

### result

```rbs
class Object
  def test_symbol_map: -> Array["1" | "2" | "3"]
end
```

## Symbol-to-proc: select(&:empty?)

### update

```ruby
def test_symbol_select = ["a", "", "b"].select(&:empty?)
```

### result

```rbs
class Object
  def test_symbol_select: -> Array["" | "a" | "b"]
end
```

## Symbol-to-proc: reject(&:empty?)

### update

```ruby
def test_symbol_reject = ["hello", "", "world"].reject(&:empty?)
```

### result

```rbs
class Object
  def test_symbol_reject: -> Array["" | "hello" | "world"]
end
```

## Symbol-to-proc: filter_map and sum

### update

```ruby
class Item
  attr_reader :name, :payload

  def initialize(name, payload)
    @name = name
    @payload = payload
  end
end

def compact_values = [1, nil, false, 2].filter_map(&:itself)

def collect_payloads
  [Item.new("a", 1), Item.new("b", nil)].filter_map(&:payload)
end

def total_bytes = ["a", "bb"].sum(&:bytesize)

def group_items
  [Item.new("a", 1), Item.new("b", nil)].group_by(&:name)
end

def unique_items
  [Item.new("a", 1), Item.new("b", nil)].uniq(&:name)
end
```

### result

```rbs
class Item
  def name: -> "a" | "b"
  def payload: -> 1?
  def initialize: (String name, Integer? payload) -> void
end

class Object
  def compact_values: -> Array[1 | 2]
  def collect_payloads: -> Array[1]
  def total_bytes: -> Integer
  def group_items: -> Hash["a" | "b", Array[Item]]
  def unique_items: -> Array[Item]
end
```

## Symbol-to-proc: pair first/last

### update

```ruby
class Entry
  attr_reader :id

  def initialize(id)
    @id = id
  end
end

def pair_keys
  [["a", 1], ["b", 2]].map { |pair| pair.first }
end

def pair_values
  [["a", 1], ["b", 2]].map(&:last)
end

def slice_heads
  [1, 2, 3, 4].each_slice(2).map(&:first)
end

def first_lists
  { a: [1, 2], b: [3] }.transform_values(&:first)
end

def first_entries
  [Entry.new(1), Entry.new(2)].group_by(&:id).transform_values(&:first)
end
```

### result

```rbs
class Entry
  def id: -> 1 | 2
  def initialize: (Integer id) -> void
end

class Object
  def pair_keys: -> Array["a" | "b"]
  def pair_values: -> Array[1 | 2]
  def slice_heads: -> Array[1 | 3]
  def first_lists: -> { a: 1, b: 3 }
  def first_entries: -> Hash[1 | 2, Entry?]
end
```
