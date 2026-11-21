# Known issues / Active Support / Extensions

## Float has no ordinalize

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Probe
  def ordinal = 1.5.ordinalize
end
```

### result

```rbs
class Probe
  def ordinal: -> untyped
end
```

## CurrentAttributes attributes use Symbol keys

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Current < ActiveSupport::CurrentAttributes
  attribute :user

  def snapshot = attributes
end
```

### result

```rbs
class Current < ActiveSupport::CurrentAttributes
  def self.instance: -> Current
  def self.attributes: -> Hash[Symbol, untyped]
  def self.reset: -> void
  def user: -> untyped
  def user=: (untyped user) -> untyped
  def self.user: -> untyped
  def self.user=: (untyped user) -> untyped
  def snapshot: -> Hash[Symbol, untyped]
end
```
