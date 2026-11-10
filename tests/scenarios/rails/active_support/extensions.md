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

```yaml
known_issue: true
```

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
