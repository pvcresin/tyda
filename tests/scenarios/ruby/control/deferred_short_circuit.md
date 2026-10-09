# Ruby / Control / Deferred Short-Circuit Values

## Resolve deferred method returns before truthiness filtering

```ruby
class DeferredShortCircuit
  def self.maybe_strings(flag)
    [flag.to_s] if flag
  end

  def self.with_default(flag)
    maybe_strings(flag) || []
  end
end
```

### result

```rbs
class DeferredShortCircuit
  def self.maybe_strings: (untyped flag) -> [String]?
  def self.with_default: (untyped flag) -> Array[String]
end
```

## Resolve deferred falsey returns before applying an `||` fallback

```yaml
known_issue: true
```

```ruby
class DeferredFalsyFallback
  def self.entries_or_empty(status)
    possible_entries(status) || []
  end

  def self.entries_and_marker(status)
    possible_entries(status) && :present
  end

  def self.possible_entries(status)
    return false if status == :blocked
    return nil if status == :missing
    ["ready"]
  end
end
```

### result

```rbs
class DeferredFalsyFallback
  def self.entries_or_empty: (untyped status) -> Array["ready"]
  def self.entries_and_marker: (untyped status) -> (false | :present)?
  def self.possible_entries: (untyped status) -> (false | ["ready"])?
end
```

## Keep nil from safe navigation when a deferred receiver is resolved

```yaml
known_issue: true
```

```ruby
class DeferredSafeNavigation
  def self.label
    value&.upcase
  end

  def self.nullable_label(status)
    nullable_value(status)&.upcase
  end

  def self.value
    "ready"
  end

  def self.nullable_value(status)
    return nil if status == :missing
    "ready"
  end
end
```

### result

```rbs
class DeferredSafeNavigation
  def self.label: -> String?
  def self.nullable_label: (untyped status) -> String?
  def self.value: -> "ready"
  def self.nullable_value: (untyped status) -> "ready"?
end
```

## Resolve deferred falsey returns with early returns and rescue

```yaml
known_issue: true
```

```ruby
class DeferredFalsyFallbackWithRescue
  def self.entries_or_empty(status)
    possible_entries(status) || []
  rescue
    []
  end

  def self.possible_entries(status)
    return false if status == :blocked
    return nil if status == :missing

    ["ready"]
  end
end
```

### result

```rbs
class DeferredFalsyFallbackWithRescue
  def self.entries_or_empty: (untyped status) -> Array["ready"]
  def self.possible_entries: (untyped status) -> (false | ["ready"])?
end
```

## Narrow container truthiness when its element type is unresolved

```yaml
known_issue: true
```

```ruby
class DeferredNestedElements
  def self.with_default(value)
    possibly_empty(value) || []
  end

  def self.possibly_empty(value)
    return false unless value

    [value]
  end
end
```

### result

```rbs
class DeferredNestedElements
  def self.with_default: (untyped value) -> Array[untyped]
  def self.possibly_empty: (untyped value) -> (false | [untyped])
end
```

## Preserve the value assigned through a dynamic hash key

```yaml
known_issue: true
```

```ruby
class DynamicHashOrAssignment
  def self.initialized_value
    values = {}
    key = [:status, 1]
    values[key] ||= []
    values[key]
  end

  def self.append
    values = {}
    key = [:status, 1]
    values[key] ||= []
    values[key] << "done"
    values[key]
  end

  def self.append_to_nonempty_hash(key)
    values = [1].group_by { |value| value }
    values[key] ||= []
    values[key] << 2
    values[key]
  end

  def self.append_to_untyped_hash(values, key)
    values[key] ||= []
    values[key] << "done"
  end

  def self.different_key_is_nilable
    values = {}
    key = [:status, 1]
    other_key = [:status, 2]
    values[key] ||= []
    values[other_key]
  end

  def self.mutated_key_is_nilable
    values = {}
    key = [:status, 1]
    values[key] ||= []
    key << :changed
    values[key]
  end
end
```

### result

```rbs
class DynamicHashOrAssignment
  def self.initialized_value: -> [ ]
  def self.append: -> Array["done"]
  def self.append_to_nonempty_hash: (untyped key) -> Array[1 | 2]
  def self.append_to_untyped_hash: (untyped values, untyped key) -> untyped
  def self.different_key_is_nilable: -> [ ]?
  def self.mutated_key_is_nilable: -> [ ]?
end
```
