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

## Preserve falsey alternatives through a deferred parameter return

```ruby
class DeferredParameterTruthiness
  def self.identity(value)
    value
  end

  def self.with_marker(value)
    identity(value) && :present
  end
end
```

### result

```rbs
class DeferredParameterTruthiness
  def self.identity: (untyped value) -> untyped
  def self.with_marker: (untyped value) -> (untyped | :present)
end
```

## Preserve falsey alternatives from a direct parameter

```ruby
class DeferredDirectParameterTruthiness
  def self.with_marker(value)
    value && :present
  end
end
```

### result

```rbs
class DeferredDirectParameterTruthiness
  def self.with_marker: (untyped value) -> (untyped | :present)
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
end
```

## Preserve nil from an omitted block in short-circuit results

```ruby
class DeferredOptionalBlock
  def self.with_default(value = nil, &block)
    value || block
  end

  def self.with_short_circuit_block(&block)
    block && :present
  end

  def self.with_guarded_block(value = nil, &block)
    if block
      value || block
    else
      :absent
    end
  end

  def self.with_guarded_short_circuit_block(&block)
    if block
      block && :present
    else
      :absent
    end
  end

  def self.with_reassigned_block(value = nil, &block)
    block = -> { :assigned }
    value || block
  end
end
```

### result

```rbs
class DeferredOptionalBlock
  def self.with_default: (?nil value, ?untyped &block) -> Proc?
  def self.with_short_circuit_block: (?untyped &block) -> :present?
  def self.with_guarded_block: (?nil value, ?untyped &block) -> (:absent | Proc)
  def self.with_guarded_short_circuit_block: (?untyped &block) -> (:absent | :present)
  def self.with_reassigned_block: (?nil value, ?untyped &block) -> Proc
end
```

## Resolve deferred projections before dispatching on a short-circuit result

```ruby
class DeferredShortCircuitReceiver
  def self.boolean(value)
    value ? true : false
  end

  def self.label_or_fallback(value)
    (maybe_label(value) || "fallback").upcase
  end

  def self.falsey_result(value)
    (boolean(value) && :present).to_s
  end

  def self.maybe_label(value)
    return nil unless value
    "ready"
  end
end
```

### result

```rbs
class DeferredShortCircuitReceiver
  def self.boolean: (untyped value) -> bool
  def self.label_or_fallback: (untyped value) -> String
  def self.falsey_result: (untyped value) -> String
  def self.maybe_label: (untyped value) -> "ready"?
end
```

## Preserve both predicate outcomes through conjunction

```ruby
class DeferredPredicateConjunction
  def canceling?(value)
    value ? true : false
  end

  def force_cancelable?(value)
    canceling?(value) && true
  end
end
```

### result

```rbs
class DeferredPredicateConjunction
  def canceling?: (untyped value) -> bool
  def force_cancelable?: (untyped value) -> bool
end
```

## Preserve both outcomes when coercing a deferred value to boolean

```ruby
class DeferredBooleanCoercion
  def initialize(value)
    @force = value || false
  end

  def raw_force
    @force
  end

  def negated_force?
    !@force
  end

  def force?
    !!@force
  end
end
```

### result

```rbs
class DeferredBooleanCoercion
  def initialize: (untyped value) -> void
  def raw_force: -> false | untyped
  def negated_force?: -> bool
  def force?: -> bool
end
```

## Negate a method result with a deferred truthiness alternative

```ruby
class DeferredPredicateNegation
  def initialize(value)
    @predicate = value || false
  end

  def predicate?
    @predicate
  end

  def negated_predicate?
    !predicate?
  end
end
```

### result

```rbs
class DeferredPredicateNegation
  def initialize: (untyped value) -> void
  def predicate?: -> false | untyped
  def negated_predicate?: -> bool
end
```

## Negate a predicate inferred through memoization

```ruby
class DeferredMemoizedPredicate
  def initialize(value)
    @value = value
  end

  def use_cte_for_search?
    strong_memoize(:use_cte_for_search) do
      next false unless value
      next false unless default_or_simple_sort?

      attempt_search_optimization? || attempt_search_optimization?
    end
  end

  def use_minimum_char_limit?
    !use_cte_for_search?
  end

  def default_or_simple_sort?
    true
  end

  def attempt_search_optimization?
    @value
  end
end
```

### result

```rbs
class DeferredMemoizedPredicate
  def initialize: (untyped value) -> void
  def use_cte_for_search?: -> false | untyped
  def use_minimum_char_limit?: -> bool
  def default_or_simple_sort?: -> true
  def attempt_search_optimization?: -> untyped
end
```

## Preserve the superclass result when a deferred predicate may be false

```ruby
module Ci
  module Metadatable
    def degenerated?
      value ? true : false
    end
  end

  class Processable < ::CommitStatus
    include Ci::Metadatable

    def archived?(...)
      degenerated? || super
    end
  end
end

class CommitStatus
  def pipeline
    Ci::Pipeline.new
  end

  def archived?(...)
    pipeline.archived?(...)
  end
end

module Ci
  class Pipeline
    def archived?(...)
      value ? true : false
    end
  end
end
```

### result

```rbs
module Ci::Metadatable
  def degenerated?: -> bool
end

class Ci::Pipeline
  def archived?: (*untyped, **untyped, ?untyped &block) -> bool
end

class Ci::Processable < CommitStatus
  include Ci::Metadatable

  def archived?: (*untyped, **untyped, ?untyped &block) -> bool
end

class CommitStatus
  def pipeline: -> Ci::Pipeline
  def archived?: (*untyped, **untyped, ?untyped &block) -> bool
end
```

## Resolve deferred singleton super through the superclass object

```ruby
class DeferredSingletonSuperChild < DeferredSingletonSuperParent
  def self.value
    super
  end
end

class DeferredSingletonSuperParent
  def self.value
    :class_value
  end

  def value
    "instance value"
  end
end
```

### result

```rbs
class DeferredSingletonSuperChild < DeferredSingletonSuperParent
  def self.value: -> :class_value
end

class DeferredSingletonSuperParent
  def self.value: -> :class_value
  def value: -> "instance value"
end
```

## Negate a resolved boolean predicate

```ruby
class DeferredResolvedPredicateNegation
  def symlink?
    value ? true : false
  end

  def file?
    !symlink?
  end
end
```

### result

```rbs
class DeferredResolvedPredicateNegation
  def symlink?: -> bool
  def file?: -> bool
end
```

## Preserve explicit cache keys through inherited short-circuit methods

### file: app/services/projects/open_issues_count_service.rb

```ruby
module Projects
  class OpenIssuesCountService < CountService
    PUBLIC_COUNT_KEY = "public_open_issues_count"
    TOTAL_COUNT_KEY = "total_open_issues_count"

    def cache_key_name
      public_only? ? PUBLIC_COUNT_KEY : TOTAL_COUNT_KEY
    end

    def public_only?
      true
    end

    def public_count_cache_key
      cache_key(PUBLIC_COUNT_KEY)
    end

    def total_count_cache_key
      cache_key(TOTAL_COUNT_KEY)
    end
  end
end
```

```ruby
module Projects
  class CountService
    VERSION = 1

    def initialize(project)
      @project = project
    end

    def cache_key(key = nil)
      cache_key = key || cache_key_name
      ["projects", "count_service", VERSION, @project.id, cache_key]
    end

    def cache_key_name
      raise NotImplementedError
    end
  end
end
```

### result

```rbs
class Projects::CountService
  VERSION: 1

  def initialize: (untyped project) -> void
  def cache_key: (?String? key) -> ["projects", "count_service", 1, untyped, String]
  def cache_key_name: -> bot
end
```
