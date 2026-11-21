# Rails / DSL / Declarative Policy

## Resolve policy predicates and rule chains

### update

```ruby
class DeclarativePolicy::Base; end

class BasePolicy < DeclarativePolicy::Base; end

class GroupPolicy < BasePolicy
  condition(:admin) { true }

  rule { admin }.enable :read_group

  def allowed? = can?(:read_group)
end
```

### result

```rbs
class GroupPolicy < BasePolicy
  def allowed?: -> bool
end
```

## Rule returns a PolicyDsl whose actions do not chain

```yaml
known_issue: true
```

### update

```ruby
class DeclarativePolicy::Base; end

class GroupPolicy < DeclarativePolicy::Base
  condition(:admin) { true }

  def self.admin_rule = self.rule { admin }

  def self.enable_read = self.rule { admin }.enable(:read_group)

  def rule_from_instance = GroupPolicy.rule { admin }
end
```

### result

```rbs
class GroupPolicy < DeclarativePolicy::Base
  def self.admin_rule: -> DeclarativePolicy::PolicyDsl
  def self.enable_read: -> untyped
  def rule_from_instance: -> DeclarativePolicy::PolicyDsl
end
```
