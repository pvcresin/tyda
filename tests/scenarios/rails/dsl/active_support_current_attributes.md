# Rails / DSL / Current Attributes

## Generate class and instance accessors

### update

```ruby
class Current < ActiveSupport::CurrentAttributes
  attribute :user, :account
end
```

### result

```rbs
class Current < ActiveSupport::CurrentAttributes
  def self.instance: -> Current
  def self.attributes: -> Hash[String, untyped]
  def self.reset: -> void
  def user: -> untyped
  def user=: (untyped user) -> untyped
  def self.user: -> untyped
  def self.user=: (untyped user) -> untyped
  def account: -> untyped
  def account=: (untyped account) -> untyped
  def self.account: -> untyped
  def self.account=: (untyped account) -> untyped
end
```
