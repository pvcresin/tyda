# Rails / Active Support / Core Methods

## Narrow nilable local in present? then branch

### update

```ruby
class Probe
  #: (String | nil) -> String
  def normalize(x)
    if x.present?
      x.upcase
    else
      "missing"
    end
  end
end
```

### result

```rbs
class Probe
  def normalize: (String? x) -> String
end
```

## Keep different branch types after present?

### update

```ruby
class Probe
  #: (String | nil) -> (1 | Symbol)
  def convert(x)
    if x.present?
      x.to_sym
    else
      1
    end
  end
end
```

### result

```rbs
class Probe
  def convert: (String? x) -> (Symbol | 1)
end
```

## present? treats false as falsy branch

### update

```ruby
class Probe
  #: (String | bool | nil) -> (Symbol | 1)
  def convert(x)
    if x.present?
      x.to_sym
    else
      1
    end
  end
end
```

### result

```rbs
class Probe
  def convert: ((String | bool)? x) -> (Symbol | 1)
end
```

## Branch nil and false after present?

### update

```ruby
class Probe
  #: (String | bool | nil) -> (Symbol | 1 | 2)
  def convert(x)
    if x.present?
      x.to_sym
    elsif x.nil?
      1
    else
      2
    end
  end
end
```

### result

```rbs
class Probe
  def convert: ((String | bool)? x) -> (Symbol | 1 | 2)
end
```

## Handle falsy branch with unless present?

### update

```ruby
class Probe
  #: (String | nil) -> String
  def normalize(x)
    unless x.present?
      "missing"
    else
      x.upcase
    end
  end
end
```

### result

```rbs
class Probe
  def normalize: (String? x) -> String
end
```

## Keep both branch types with unless present?

### update

```ruby
class Probe
  #: (String | nil) -> (1 | Symbol)
  def convert(x)
    unless x.present?
      1
    else
      x.to_sym
    end
  end
end
```

### result

```rbs
class Probe
  def convert: (String? x) -> (Symbol | 1)
end
```

## Keep false in falsy branch with unless present?

### update

```ruby
class Probe
  #: (String | bool | nil) -> (Symbol | 1)
  def convert(x)
    unless x.present?
      1
    else
      x.to_sym
    end
  end
end
```

### result

```rbs
class Probe
  def convert: ((String | bool)? x) -> (Symbol | 1)
end
```

## Keep both branch types when branching on presence

### update

```ruby
class Probe
  #: (String | nil) -> (Symbol | 1)
  def convert(x)
    if x.presence
      x.to_sym
    else
      1
    end
  end
end
```

### result

```rbs
class Probe
  def convert: (String? x) -> (Symbol | 1)
end
```

## Keep both branch types with unless presence

### update

```ruby
class Probe
  #: (String | nil) -> (Symbol | 1)
  def convert(x)
    unless x.presence
      1
    else
      x.to_sym
    end
  end
end
```

### result

```rbs
class Probe
  def convert: (String? x) -> (Symbol | 1)
end
```

## Narrow nilable local in blank? else branch

### update

```ruby
class Probe
  #: (String | nil) -> String
  def normalize(x)
    if x.blank?
      "missing"
    else
      x.upcase
    end
  end
end
```

### result

```rbs
class Probe
  def normalize: (String? x) -> String
end
```

## Remove nil from local after blank? guard return

### update

```ruby
class Corporation
end

class Probe
  def pick(flag)
    corporation = flag ? Corporation.new : nil
    return Corporation.new if corporation.blank?
    corporation
  end
end
```

### result

```rbs
class Probe
  def pick: (untyped flag) -> Corporation
end
```

## presence_in returns receiver or nil

### update

```ruby
class Probe
  def choose(value = nil) = value.presence_in(["a", "b"])
end

Probe.new.choose("a")
```

### result

```rbs
class Probe
  def choose: (?String? value) -> String?
end
```

## try resolves symbol target methods

### update

```ruby
class User
  #: () -> String
  def name = "display"
end

class Probe
  def maybe_name(user = nil) = user.try(:name)
end

Probe.new.maybe_name(User.new)
```

### result

```rbs
class Probe
  def maybe_name: (?User? user) -> String?
end

class User
  def name: -> String
end
```

## Resolve duration instance methods

### update

```ruby
class Probe
  def duration = 5.minutes

  def duration_seconds = duration.to_i

  def duration_ago = duration.ago

  def duration_parts = duration.parts
end
```

### result

```rbs
class Probe
  def duration: -> ActiveSupport::Duration
  def duration_seconds: -> Integer
  def duration_ago: -> Time
  def duration_parts: -> Hash[Symbol, untyped]
end
```

## ActiveSupport index_by / index_with helpers

### update

```ruby
class Item
  attr_reader :id, :name

  def initialize(id, name)
    @id = id
    @name = name
  end
end

class Probe
  def entries = [Item.new(1, "book"), Item.new(2, "pen")]

  def index_by_block = entries.index_by { |item| item.id }

  def index_by_proc = entries.index_by(&:name)

  def index_with_block = [:a, :b].index_with { |key| key.to_s }

  def index_with_default = [:a, :b].index_with(false)
end
```

### result

```rbs
class Item
  def id: -> 1 | 2
  def name: -> "book" | "pen"
  def initialize: (Integer id, String name) -> void
end

class Probe
  def entries: -> [Item, Item]
  def index_by_block: -> Hash[1 | 2, Item]
  def index_by_proc: -> Hash["book" | "pen", Item]
  def index_with_block: -> Hash[:a | :b, String]
  def index_with_default: -> Hash[:a | :b, false]
end
```

## ActiveSupport Array.wrap and predicate helpers

### update

```ruby
class Probe
  def wrapped_nil = Array.wrap(nil)

  def wrapped_value = Array.wrap(:tag)

  def wrapped_array = Array.wrap([1, 2])

  def predicates(value) = [value.in?([1, 2]), [1, 2].exclude?(3), [1, 2].many?]

  def formatted(value) = value.to_fs(:db)
end
```

### result

```rbs
class Probe
  def wrapped_nil: -> [ ]
  def wrapped_value: -> Array[:tag]
  def wrapped_array: -> [1, 2]
  def predicates: (untyped value) -> [bool, bool, bool]
  def formatted: (untyped value) -> String
end
```

## ActiveSupport deep_dup / compact_blank helpers

### update

```ruby
class Probe
  def duplicate_record = { name: "Ada" }.deep_dup

  def compact_blank_array = [1, nil, false, "", "ok"].compact_blank

  def compact_blank_record = { name: "Ada", note: "", count: 1, active: false }.compact_blank

  def compact_blank_maybe_record(name = "Ada") = { name: name, count: 1 }.compact_blank
end
```

### result

```rbs
class Probe
  def duplicate_record: -> { name: "Ada" }
  def compact_blank_array: -> [1, "ok"]
  def compact_blank_record: -> { name: "Ada", count: 1 }
  def compact_blank_maybe_record: (?String name) -> { ?name: String, count: 1 }
end
```

## static constantize resolves project constants

### update

```ruby
module Group
  class Item
    def label = "item"
  end
end

class Probe
  def constantized_label = "Group::Item".constantize.new.label

  def safe_constantized_label = "Group::Item".safe_constantize&.new&.label

  def missing_constant = "Group::Missing".safe_constantize
end
```

### result

```rbs
class Group::Item
  def label: -> "item"
end

class Probe
  def constantized_label: -> "item"
  def safe_constantized_label: -> "item"
  def missing_constant: -> nil
end
```

## try! resolves symbol target methods

### update

```ruby
class User
  #: () -> String
  def name = "display"
end

class Probe
  def sure_name(user = nil) = user.try!(:name)
end

Probe.new.sure_name(User.new)
```

### result

```rbs
class Probe
  def sure_name: (?User? user) -> String?
end

class User
  def name: -> String
end
```

## Array to_sentence returns String

### update

```ruby
class Probe
  def joined = ["a", "b", "c"].to_sentence
end
```

### result

```rbs
class Probe
  def joined: -> String
end
```

## Array in_groups_of yields nilable groups

### update

```ruby
class Probe
  def grouped = [1, 2, 3].in_groups_of(2)
end
```

### result

```rbs
class Probe
  def grouped: -> Array[Array[Integer?]]
end
```
