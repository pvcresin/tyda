# Rails / DSL / Kredis

## Kredis scalar attributes return Scalar without a setter

```yaml
known_issue: true
```

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  kredis_string :nickname
  kredis_integer :visits
  kredis_decimal :balance
  kredis_float :score
  kredis_boolean :onboarded
  kredis_datetime :last_seen_at
  kredis_json :settings
end
```

### result

```rbs
class User < ActiveRecord::Base
  def nickname: -> Kredis::Types::Scalar
  def visits: -> Kredis::Types::Scalar
  def balance: -> Kredis::Types::Scalar
  def score: -> Kredis::Types::Scalar
  def onboarded: -> Kredis::Types::Scalar
  def last_seen_at: -> Kredis::Types::Scalar
  def settings: -> Kredis::Types::Scalar
end
```

## kredis_slot returns Slots and kredis_flag adds a predicate

```yaml
known_issue: true
```

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  kredis_flag :special
  kredis_slot :attempts
end
```

### result

```rbs
class User < ActiveRecord::Base
  def special: -> Kredis::Types::Flag
  def special?: -> bool
  def attempts: -> Kredis::Types::Slots
end
```
