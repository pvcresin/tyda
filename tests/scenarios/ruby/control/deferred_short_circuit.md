# Ruby / Control / Deferred Short-Circuit Values

## Preserve unknown values resolved from an RBS union

```yaml
known_issue: true
```

### update

```rbs
class DeferredUnknownSource
  def self.value: -> (false | untyped)
end
```

```ruby
class DeferredUnknownConsumer
  def self.classify
    if DeferredUnknownSource.value
      :truthy
    else
      :falsy
    end
  end
end
```

### result

```rbs
class DeferredUnknownConsumer
  def self.classify: -> (:falsy | :truthy)
end
```

## Resolve deferred method returns before truthiness filtering

```ruby
class DeferredShortCircuit
  def self.maybe_strings(flag)
    [String(flag)] if flag
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

## Preserve known boolean return values through deferred short-circuit calls

```yaml
known_issue: true
```

```ruby
class DeferredKnownBooleanShortCircuit
  def self.maybe_strings(flag)
    [flag.to_s] if flag
  end

  def self.with_default(flag)
    maybe_strings(flag) || []
  end

  def self.with_default_for_boolean = with_default(rand < 0.5)
end
```

### result

```rbs
class DeferredKnownBooleanShortCircuit
  def self.maybe_strings: (bool flag) -> ["true"]?
  def self.with_default: (bool flag) -> ([ ] | ["true"])
  def self.with_default_for_boolean: -> ([ ] | ["true"])
end
```

## Resolve deferred falsey returns before applying an `||` fallback

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
  def self.entries_or_empty: (untyped status) -> ([ ] | ["ready"])
  def self.entries_and_marker: (untyped status) -> (false | :present)?
  def self.possible_entries: (untyped status) -> (false | ["ready"])?
end
```

## Keep nil from safe navigation when a deferred receiver is resolved

```ruby
class DeferredSafeNavigation
  def self.label
    value&.upcase
  end

  def self.nullable_label(status)
    nullable_value(status)&.upcase
  end

  def self.false_receiver = false&.to_s

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
  def self.label: -> String
  def self.nullable_label: (untyped status) -> String?
  def self.false_receiver: -> "false"
  def self.value: -> "ready"
  def self.nullable_value: (untyped status) -> "ready"?
end
```

## Do not infer nil from an unresolved array element during safe navigation

```ruby
class DeferredSafeNavigationWithDeferredElement
  def self.values = [value]

  def self.value = "ready"

  def self.size = values&.size
end
```

### result

```rbs
class DeferredSafeNavigationWithDeferredElement
  def self.values: -> ["ready"]
  def self.value: -> "ready"
  def self.size: -> 1
end
```

## Resolve deferred falsey returns with early returns and rescue

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
  def self.entries_or_empty: (untyped status) -> ([ ] | ["ready"])
  def self.possible_entries: (untyped status) -> (false | ["ready"])?
end
```

## Narrow container truthiness when its element type is unresolved

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
  def self.with_default: (untyped value) -> ([ ] | [untyped])
  def self.possibly_empty: (untyped value) -> (false | [untyped])
end
```

## Preserve the value assigned through a dynamic hash key

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

  def self.different_key_returns_nil
    values = {}
    key = [:status, 1]
    other_key = [:status, 2]
    values[key] ||= []
    values[other_key]
  end

  def self.mutated_key_returns_nil
    values = {}
    key = [:status, 1]
    values[key] ||= []
    key << :changed
    values[key]
  end

  def self.plain_write(key)
    values = {}
    values[key] = 1
    values[key]
  end

  def self.reassigned_key_may_be_absent(key, other)
    values = {}
    values[key] = 1
    key = other
    values[key]
  end

  def self.deleted_key_may_be_absent(key)
    values = {}
    values[key] = 1
    values.delete(key)
    values[key]
  end

  def self.branch_write_may_be_absent(key, flag)
    values = {}
    values[key] = 1 if flag
    values[key]
  end

  def self.aliased_hash_may_lose_key(key)
    values = {}
    values[key] = 1
    other = values
    other.clear
    values[key]
  end

  def self.default_hash_key_may_be_defaulted
    values = Hash.new(0)
    values[:a] = 1
    values[:b]
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
  def self.different_key_returns_nil: -> nil
  def self.mutated_key_returns_nil: -> nil
  def self.plain_write: (untyped key) -> 1
  def self.reassigned_key_may_be_absent: (untyped key, untyped other) -> 1?
  def self.deleted_key_may_be_absent: (untyped key) -> 1?
  def self.branch_write_may_be_absent: (untyped key, untyped flag) -> untyped
  def self.aliased_hash_may_lose_key: (untyped key) -> 1?
  def self.default_hash_key_may_be_defaulted: -> (0 | 1)?
end
```
