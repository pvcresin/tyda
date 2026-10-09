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

```yaml
known_issue: true
```

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

```yaml
known_issue: true
```

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
  def mapped: -> Array[[untyped, 0]]
  def values: -> [ ]
end
```

## Preserve block execution for an empty receiver and updated local

```yaml
known_issue: true
```

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
  def mapped: -> Array[[untyped, Integer]]
  def values: -> [ ]
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

```yaml
known_issue: true
```

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
  def kept_settings: -> untyped
end
```
