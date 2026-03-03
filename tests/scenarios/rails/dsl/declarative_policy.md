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
