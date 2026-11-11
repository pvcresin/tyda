# Active Support / Extensions

## Resolve common primitive extensions

### update

```ruby
class Probe
  def bytes = 64.bytes
  def kilobyte = 1.kilobyte
  def kilobytes = 2.kilobytes
  def megabyte = 1.megabyte
  def megabytes = 2.megabytes
  def gigabyte = 1.gigabyte
  def gigabytes = 2.gigabytes

  def constantize = "String".constantize
  def safe_constantize = "String".safe_constantize
  def truncate = "a long title".truncate(4)
  def ends_with = "rails".ends_with?("ls")
  def starts_with = "rails".starts_with?("ra")
  def ordinalize = 4.ordinalize
  def first_character = "rails".first
  def last_character = "rails".last
  def humanize = "rails".humanize
  def html_safe = "rails".html_safe
  def minutes_from_now = 2.minutes.from_now
  def from_now = 2.days.from_now
end
```

### result

```rbs
class Probe
  def bytes: -> 64
  def kilobyte: -> Integer
  def kilobytes: -> Integer
  def megabyte: -> Integer
  def megabytes: -> Integer
  def gigabyte: -> Integer
  def gigabytes: -> Integer
  def constantize: -> singleton(String)
  def safe_constantize: -> singleton(String)
  def truncate: -> String
  def ends_with: -> bool
  def starts_with: -> bool
  def ordinalize: -> String
  def first_character: -> String
  def last_character: -> String
  def humanize: -> String
  def html_safe: -> String
  def minutes_from_now: -> Time
  def from_now: -> Time
end
```

## Serialize nil with Active Support JSON

### update

```ruby
class JsonExtension
  def nil_json = nil.to_json
  def value_json(value) = value.to_json
end
```

### result

```rbs
class JsonExtension
  def nil_json: -> String
  def value_json: (untyped value) -> String
end
```

## Resolve string inflection and filter extensions

### update

```ruby
class Inflections
  def squished = "a  b".squish
  def slug = "A b".parameterize
  def title = "a b".titleize
  def plural = "post".pluralize
  def singular = "posts".singularize
  def base_name = "A::B".demodulize
  def class_name = "post".classify
  def table_name = "Post".tableize
  def dashed = "a_b".dasherize
  def key_name = "Post".foreign_key
  def removed = "a-b".remove("-")
  def words = "a b c".truncate_words(2)
  def excluded = "abc".exclude?("d")
  def date = "2024-01-01".to_date
end
```

### result

```rbs
class Inflections
  def squished: -> String
  def slug: -> String
  def title: -> String
  def plural: -> String
  def singular: -> String
  def base_name: -> String
  def class_name: -> String
  def table_name: -> String
  def dashed: -> String
  def key_name: -> String
  def removed: -> String
  def words: -> String
  def excluded: -> bool
  def date: -> Date
end
```

## String to_date returns nil for a blank string

### update

```rbs
class DateInput
  def raw: -> String
end
```

```ruby
class DateInput
  def date = raw.to_date
end
```

### result

```rbs
class DateInput
  def date: -> Date?
end
```

## Array access extensions return the element or nil

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Picker
  # @rbs items: Array[Integer]
  def second(items) = items.second

  # @rbs items: Array[Integer]
  def third(items) = items.third
end
```

### result

```rbs
class Picker
  def second: (Array[Integer] items) -> Integer?
  def third: (Array[Integer] items) -> Integer?
end
```

## Array excluding including and in_groups keep the element type

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Picker
  # @rbs items: Array[Integer]
  def without_one(items) = items.excluding(1)

  # @rbs items: Array[Integer]
  def with_three(items) = items.including(3)

  # @rbs items: Array[Integer]
  def groups(items) = items.in_groups(2)
end
```

### result

```rbs
class Picker
  def without_one: (Array[Integer] items) -> Array[Integer]
  def with_three: (Array[Integer] items) -> Array[Integer]
  def groups: (Array[Integer] items) -> Array[Array[Integer?]]
end
```

## symbolize_keys and stringify_keys change the key type

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Keys
  # @rbs hash: Hash[String, Integer]
  def symbolized(hash) = hash.symbolize_keys

  # @rbs hash: Hash[Symbol, Integer]
  def stringified(hash) = hash.stringify_keys
end
```

### result

```rbs
class Keys
  def symbolized: (Hash[String, Integer] hash) -> Hash[Symbol, Integer]
  def stringified: (Hash[Symbol, Integer] hash) -> Hash[String, Integer]
end
```

## reverse_merge keeps keys from both record hashes

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Options
  def merged = { a: 1 }.reverse_merge(b: 2)
end
```

### result

```rbs
class Options
  def merged: -> { b: 2, a: 1 }
end
```

## with_indifferent_access returns HashWithIndifferentAccess

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Options
  # @rbs hash: Hash[Symbol, Integer]
  def indifferent(hash) = hash.with_indifferent_access
end
```

### result

```rbs
class Options
  def indifferent: (Hash[Symbol, Integer] hash) -> ActiveSupport::HashWithIndifferentAccess
end
```

## Resolve multiple_of? to_query and secure_compare

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Checks
  def even = 4.multiple_of?(2)

  def query = { page: 1 }.to_query

  def same = ActiveSupport::SecurityUtils.secure_compare("a", "b")
end
```

### result

```rbs
class Checks
  def even: -> bool
  def query: -> String
  def same: -> bool
end
```
