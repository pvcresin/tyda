# Ruby / Control / Source Predicate Guards

## Source predicate with nil exits narrows its local arguments

```yaml
known_issue: true
```

### update

```ruby
class SourcePredicateGuard
  #: () -> String?
  def maybe_string = nil

  def both_present?(left, right)
    return false unless left
    return nil unless right
    true
  end

  def guarded
    left = maybe_string
    right = maybe_string
    return :none unless both_present?(left, right)
    [left, right]
  end
end
```

### result

```rbs
class SourcePredicateGuard
  def maybe_string: -> String?
  def both_present?: (String? left, String? right) -> bool?
  def guarded: -> :none | [String, String]
end
```

## Source predicates that do not prove their argument keep nil

### update

```ruby
class SourcePredicateControls
  #: () -> String?
  def maybe_string = nil

  def always_true?(_value)
    true
  end

  def reassigned?(value)
    return false unless value
    value = nil
    true
  end

  def falls_through?(value)
    unless value
      false
    end
    true
  end

  def truthy_exit?(value)
    return :early unless value
    true
  end

  def present?(value)
    return false unless value
    true
  end

  def unconditional
    value = maybe_string
    return :none unless always_true?(value)
    value
  end

  def reassignment
    value = maybe_string
    return :none unless reassigned?(value)
    value
  end

  def fall_through
    value = maybe_string
    return :none unless falls_through?(value)
    value
  end

  def truthy_exit
    value = maybe_string
    return :none unless truthy_exit?(value)
    value
  end

  def or_branch
    value = maybe_string
    return :none unless present?(value) || true
    value
  end
end
```

### result

```rbs
class SourcePredicateControls
  def maybe_string: -> String?
  def always_true?: (String? _value) -> true
  def reassigned?: (String? value) -> bool
  def falls_through?: (String? value) -> true
  def truthy_exit?: (String? value) -> (true | :early)
  def present?: (String? value) -> bool
  def unconditional: -> (String | :none)?
  def reassignment: -> (String | :none)?
  def fall_through: -> (String | :none)?
  def truthy_exit: -> (String | :none)?
  def or_branch: -> (String | :none)?
end
```

## Overrides of a source predicate keep nil

### update

```ruby
class PredicateOverriddenBelow
  #: () -> String?
  def maybe_string = nil

  def present?(value)
    return false unless value
    true
  end

  def checked
    value = maybe_string
    return :none unless present?(value)
    value
  end
end

class PredicateOverrideChild < PredicateOverriddenBelow
  def present?(_value) = true
end

module PredicatePrependedOverride
  def present?(_value) = true
end

class PredicatePrepended
  prepend PredicatePrependedOverride

  #: () -> String?
  def maybe_string = nil

  def present?(value)
    return false unless value
    true
  end

  def checked
    value = maybe_string
    return :none unless present?(value)
    value
  end
end

class PredicateConditionallyDefined
  #: () -> String?
  def maybe_string = nil

  if ENV["ENABLE_PREDICATE"]
    def present?(value)
      return false unless value
      true
    end
  end

  def checked
    value = maybe_string
    return :none unless present?(value)
    value
  end
end
```

### result

```rbs
class PredicateConditionallyDefined
  def maybe_string: -> String?
  def present?: (String? value) -> bool
  def checked: -> (String | :none)?
end

class PredicateOverriddenBelow
  def maybe_string: -> String?
  def present?: (String? value) -> bool
  def checked: -> (String | :none)?
end

class PredicateOverrideChild < PredicateOverriddenBelow
  def present?: (untyped _value) -> true
end

class PredicatePrepended
  prepend PredicatePrependedOverride

  def maybe_string: -> String?
  def present?: (String? value) -> bool
  def checked: -> (String | :none)?
end

module PredicatePrependedOverride
  def present?: (untyped _value) -> true
end
```

## Unrelated mixins and singleton methods keep the source predicate effect

```yaml
known_issue: true
```

### update

```ruby
module PredicateUnrelatedMixin
  def unrelated = true
end

class PredicateWithUnrelatedNames
  include PredicateUnrelatedMixin

  #: () -> String?
  def maybe_string = nil

  def present?(value)
    return false unless value
    true
  end

  def self.present?(_value) = true

  def checked
    value = maybe_string
    return :none unless present?(value)
    value
  end
end
```

### result

```rbs
module PredicateUnrelatedMixin
  def unrelated: -> true
end

class PredicateWithUnrelatedNames
  include PredicateUnrelatedMixin

  def maybe_string: -> String?
  def present?: (String? value) -> bool
  def self.present?: (untyped _value) -> true
  def checked: -> (String | :none)
end
```

## Aliased source predicates keep nil

### update

```ruby
class PredicateWithAlias
  #: () -> String?
  def maybe_string = nil

  def present?(value)
    return false unless value
    true
  end
  alias aliased_present? present?

  def checked
    value = maybe_string
    return :none unless present?(value)
    value
  end
end
```

### result

```rbs
class PredicateWithAlias
  def maybe_string: -> String?
  def present?: (String? value) -> bool
  def checked: -> (String | :none)?
  alias aliased_present? present?
end
```

## Mixin overrides in another file keep nil

### update

### file: base.rb

```ruby
class PredicateCrossFileBase
  #: () -> String?
  def maybe_string = nil

  def present?(value)
    return false unless value
    true
  end

  def checked
    value = maybe_string
    return :none unless present?(value)
    value
  end
end
```

### file: child.rb

```ruby
module PredicateCrossFileOverride
  def present?(_value) = true
end

class PredicateCrossFileChild < PredicateCrossFileBase
  include PredicateCrossFileOverride
end
```

### result

```rbs
class PredicateCrossFileBase
  def maybe_string: -> String?
  def present?: (String? value) -> bool
  def checked: -> (String | :none)?
end

class PredicateCrossFileChild < PredicateCrossFileBase
  include PredicateCrossFileOverride
end

module PredicateCrossFileOverride
  def present?: (untyped _value) -> true
end
```

## Nil guards on a local narrow the guarded branch

### update

```ruby
class NullableLocalGuard
  def readable? = true
end

class NullableLocalGuardFlow
  #: () -> NullableLocalGuard?
  def maybe = nil

  def short_circuit
    resource = maybe
    resource && resource.readable?
  end

  def assigned_in_condition
    if (resource = maybe) && resource.readable?
      resource
    else
      :none
    end
  end

  def assigned_in_unless
    unless (resource = maybe) && resource.readable?
      :none
    else
      resource
    end
  end

  def unguarded = maybe.readable?
end
```

### result

```rbs
class NullableLocalGuard
  def readable?: -> true
end

class NullableLocalGuardFlow
  def maybe: -> NullableLocalGuard?
  def short_circuit: -> true | nil
  def assigned_in_condition: -> :none | NullableLocalGuard
  def assigned_in_unless: -> :none | NullableLocalGuard
  def unguarded: -> true
end
```

## A respond_to guard on the left of and covers the right

```ruby
class RespondGuardedNote
end

class RespondGuardedCaller
  def run(note)
    note.respond_to?(:zork) && note.zork
  end

  def call_run
    run(RespondGuardedNote.new)
  end
end
```

### result

```rbs
class RespondGuardedCaller
  def run: (RespondGuardedNote note) -> (false | untyped)
  def call_run: -> false | untyped
end
```
