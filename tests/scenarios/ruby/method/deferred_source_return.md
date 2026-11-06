# Ruby / Method / Deferred Source Return

## Resolve chained source method returns after their definitions arrive

### update

`app/models/controller.rb`

```ruby
class Controller
  def self.value
    Provider.value
  end
end
```

`app/models/provider.rb`

```ruby
class Provider
  def self.value
    DeferredTarget.value
  end
end
```

`app/models/deferred_target.rb`

```ruby
class DeferredTarget
  def self.value
    Inner.value
  end
end
```

`app/models/inner.rb`

```ruby
class Inner
  def self.value
    "known"
  end
end
```

```ruby
class Consumer
  def value
    Controller.value
  end
end
```

### result

```rbs
class Consumer
  def value: -> "known"
end
```

## Resolve an instance method return before dispatching a block call

```ruby
class DeferredCollectionReceiver
  def indexed_headers(default = nil)
    headers.map { |header| [header, 0] }
  end

  def headers
    ["id".to_s]
  end
end
```

### result

```rbs
class DeferredCollectionReceiver
  def indexed_headers: (?nil default) -> Array[[String, 0]]
  def headers: -> [String]
end
```

## Keep an empty array result when a block has no elements

```ruby
class EmptyDeferredCollection
  def mapped
    values.map { |value| [value, 0] }
  end

  def values
    []
  end
end
```

### result

```rbs
class EmptyDeferredCollection
  def mapped: -> [ ]
  def values: -> [ ]
end
```

## Preserve block execution for an empty receiver and updated local

```ruby
class EmptyDeferredCounter
  def mapped
    index = -1
    values.map { |value| [value, index += 1] }
  end

  def values
    []
  end
end
```

### result

```rbs
class EmptyDeferredCounter
  def mapped: -> [ ]
  def values: -> [ ]
end
```

## Empty array transformations keep an empty tuple

```ruby
class EmptyArrayTransformations
  def mapped = [].map { :mapped }
  def collected = [].collect { :collected }
  def mapped_in_place = [].map! { :mapped }
end
```

### result

```rbs
class EmptyArrayTransformations
  def mapped: -> [ ]
  def collected: -> [ ]
  def mapped_in_place: -> [ ]
end
```

## Do not assume an optional empty-array default stays empty

### update

```ruby
class OptionalArrayDefaultMap
  attr_reader :values

  def initialize(values: [])
    @values = values
  end

  def mapped = values.map { :mapped }
end
```

### result

```rbs
class OptionalArrayDefaultMap
  def values: -> [ ]
  def initialize: (?values: [ ]) -> void
  def mapped: -> Array[:mapped]
end
```

## Source-defined array transformations keep their return types

```ruby
class Array
  def map = :source_map
  def collect = :source_collect
end

class DeferredArrayTransformationOverride
  def mapped = values.map { load_raw(_1) }
  def collected = values.collect { load_raw(_1) }
  def values = []
  def load_raw(_value) = :deferred
end
```

### result

```rbs
class Array[E]
  def map: -> :source_map
  def collect: -> :source_collect
end

class DeferredArrayTransformationOverride
  def mapped: -> :source_map
  def collected: -> :source_collect
  def values: -> [ ]
  def load_raw: (nil _value) -> :deferred
end
```

## Keep memoized helper hash returns concrete

### update

```ruby
class MemoizedHashSource
  def clear_cache
    @source_requirements = nil
  end

  def source_requirements
    @source_requirements ||= find_source_requirements
  end

  def find_source_requirements
    { "ruby" => "3.0".split }
  end
end
```

### result

```rbs
class MemoizedHashSource
  def clear_cache: -> nil
  def source_requirements: -> { "ruby" => Array[String] }
  def find_source_requirements: -> { "ruby" => Array[String] }
end
```

## Resolve a hash source return before dispatching its block overload

```ruby
class DeferredHashReceiver
  def settings(key = nil)
    values = {}
    if key
      values[key] ||= {}
    else
      values
    end
  end

  def kept_settings
    settings.keep_if { |key, _value| key }
  end
end
```

### result

```rbs
class DeferredHashReceiver
  def settings: (?nil key) -> Hash[untyped, untyped]
  def kept_settings: -> Hash[untyped, untyped]
end
```

## Keep a block call unknown when part of its receiver is untyped

```rbs
class MixedHashReceiver
  def settings: () -> (nil | untyped | Hash[untyped, untyped])
end
```

```ruby
class MixedHashReceiver
  def kept_settings
    settings.keep_if { |key, _value| key }
  end
end
```

### result

```rbs
class MixedHashReceiver
  def kept_settings: -> untyped | Hash[untyped, untyped]
end
```

## Preserve literal record fields through direct source helpers

```ruby
class AlertPayloadBuilder
  def self.build(alert)
    attrs(alert)
  end

  def self.attrs(alert)
    { title: alert.title, count: alert.count }
  end
end

class AlertPayload
  def title
    "critical"
  end

  def count
    1
  end

  def payload
    AlertPayloadBuilder.build(self)
  end
end
```

### result

```rbs
class AlertPayload
  def title: -> "critical"
  def count: -> 1
  def payload: -> { title: "critical", count: 1 }
end

class AlertPayloadBuilder
  def self.build: (AlertPayload alert) -> { title: "critical", count: 1 }
  def self.attrs: (AlertPayload alert) -> { title: "critical", count: 1 }
end
```

## Resolve forward returns through nested rescue branches

```rbs
module Rails
  def self.root: -> Pathname
end
```

```ruby
class AppRevision
  def self.version
    @version ||= revision_or_fallback
  end

  def self.revision_or_fallback
    begin
      revision_file.read
    rescue Errno::ENOENT
      begin
        git_revision
      rescue Errno::ENOENT
        "UNKNOWN"
      end
    end.strip
  end

  def self.revision_file = Rails.root.join("REVISION")
  def self.git_revision = `git rev-parse HEAD`
end
```

### result

```rbs
class AppRevision
  def self.version: -> String
  def self.revision_or_fallback: -> String
  def self.revision_file: -> Pathname
  def self.git_revision: -> String
end
```

## A short circuit over a deferred subclass arm keeps an untyped right side

### update

```ruby
class DeferredListNode
  def initialize(head, tail)
    @head = head
    @tail = tail
  end

  attr_reader :head, :tail

  def empty? = false

  def same_end(other)
    current = self
    current = current.tail if other
    current.empty? && other.empty?
  end
end

class DeferredEmptyNode < DeferredListNode
  def initialize; end

  def empty? = true
end

DeferredListNode.new(1, DeferredEmptyNode.new)
```

### result

```rbs
class DeferredEmptyNode < DeferredListNode
  def initialize: -> void
  def empty?: -> true
end

class DeferredListNode
  def initialize: (Integer head, DeferredEmptyNode tail) -> void
  def head: -> Integer
  def tail: -> DeferredEmptyNode
  def empty?: -> false
  def same_end: (untyped other) -> (false | untyped)
end
```
