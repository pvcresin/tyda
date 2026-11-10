# Rails / Active Record / Enum

## Generate enum predicate bang and scope methods

### update

```ruby
class Order
  enum status: { pending: 0, shipped: 1, delivered: 2 }
end
```

### result

```rbs
class Order
  def status: -> String
  def self.statuses: -> Hash[String, untyped]
  def pending?: -> bool
  def pending!: -> bool
  def self.pending: -> ActiveRecord::Relation[Order]
  def shipped?: -> bool
  def shipped!: -> bool
  def self.shipped: -> ActiveRecord::Relation[Order]
  def delivered?: -> bool
  def delivered!: -> bool
  def self.delivered: -> ActiveRecord::Relation[Order]
end
```

## Expose enum values through the pluralized class method

### update

```ruby
class EnumLevels
  enum :level, { low: 0, high: 1 }

  def self.level_names = levels.keys
end
```

### result

```rbs
class EnumLevels
  def level: -> String
  def self.levels: -> Hash[String, untyped]
  def low?: -> bool
  def low!: -> bool
  def self.low: -> ActiveRecord::Relation[EnumLevels]
  def high?: -> bool
  def high!: -> bool
  def self.high: -> ActiveRecord::Relation[EnumLevels]
  def self.level_names: -> Array[String]
end
```

## Apply enum prefix and suffix options

### update

```ruby
class User
  enum :status, { active: 0, archived: 1 }, prefix: true
  enum :role, { admin: 0, guest: 1 }, suffix: true
end
```

### result

```rbs
class User
  def status: -> String
  def self.statuses: -> Hash[String, untyped]
  def status_active?: -> bool
  def status_active!: -> bool
  def self.status_active: -> ActiveRecord::Relation[User]
  def status_archived?: -> bool
  def status_archived!: -> bool
  def self.status_archived: -> ActiveRecord::Relation[User]
  def role: -> String
  def self.roles: -> Hash[String, untyped]
  def admin_role?: -> bool
  def admin_role!: -> bool
  def self.admin_role: -> ActiveRecord::Relation[User]
  def guest_role?: -> bool
  def guest_role!: -> bool
  def self.guest_role: -> ActiveRecord::Relation[User]
end
```

## Generate predicates from positional array values

### update

```ruby
class A
  enum :kind, [:pending, :done]
end
```

### result

```rbs
class A
  def kind: -> String
  def self.kinds: -> Hash[String, untyped]
  def pending?: -> bool
  def pending!: -> bool
  def self.pending: -> ActiveRecord::Relation[A]
  def done?: -> bool
  def done!: -> bool
  def self.done: -> ActiveRecord::Relation[A]
end
```

## Generate predicates from keyword array values

### update

```ruby
class B
  enum kind: [:pending, :done]
end
```

### result

```rbs
class B
  def kind: -> String
  def self.kinds: -> Hash[String, untyped]
  def pending?: -> bool
  def pending!: -> bool
  def self.pending: -> ActiveRecord::Relation[B]
  def done?: -> bool
  def done!: -> bool
  def self.done: -> ActiveRecord::Relation[B]
end
```

## Accept legacy _prefix and _suffix options

### update

```ruby
class C
  enum :status, { active: 0, archived: 1 }, _prefix: true
  enum :role, { admin: 0, guest: 1 }, _suffix: true
end
```

### result

```rbs
class C
  def status: -> String
  def self.statuses: -> Hash[String, untyped]
  def status_active?: -> bool
  def status_active!: -> bool
  def self.status_active: -> ActiveRecord::Relation[C]
  def status_archived?: -> bool
  def status_archived!: -> bool
  def self.status_archived: -> ActiveRecord::Relation[C]
  def role: -> String
  def self.roles: -> Hash[String, untyped]
  def admin_role?: -> bool
  def admin_role!: -> bool
  def self.admin_role: -> ActiveRecord::Relation[C]
  def guest_role?: -> bool
  def guest_role!: -> bool
  def self.guest_role: -> ActiveRecord::Relation[C]
end
```

## Return the string key from an enum reader

### update

```ruby
class EnumReader
  enum :status, { pending: 0, shipped: 1 }
  def status_value = status
  def status_symbol = status.to_sym
end
```

### result

```rbs
class EnumReader
  def status: -> String
  def self.statuses: -> Hash[String, untyped]
  def pending?: -> bool
  def pending!: -> bool
  def self.pending: -> ActiveRecord::Relation[EnumReader]
  def shipped?: -> bool
  def shipped!: -> bool
  def self.shipped: -> ActiveRecord::Relation[EnumReader]
  def status_value: -> String
  def status_symbol: -> Symbol
end
```

## Preserve a source reader defined after the enum

### update

```ruby
class EnumSourceReader
  enum :status, { pending: 0, shipped: 1 }
  #: () -> Symbol
  def status = :pending
  def status_copy = status
end
```

### result

```rbs
class EnumSourceReader
  def self.statuses: -> Hash[String, untyped]
  def pending?: -> bool
  def pending!: -> bool
  def self.pending: -> ActiveRecord::Relation[EnumSourceReader]
  def shipped?: -> bool
  def shipped!: -> bool
  def self.shipped: -> ActiveRecord::Relation[EnumSourceReader]
  def status: -> Symbol
  def status_copy: -> Symbol
end
```

## Preserve an RBS reader declaration

### update

```ruby
class EnumRbsReader
  enum :status, { pending: 0, shipped: 1 }
  def status_value = status
end
```

```rbs
class EnumRbsReader
  def status: -> Symbol
end
```

### result

```rbs
class EnumRbsReader
  def self.statuses: -> Hash[String, untyped]
  def pending?: -> bool
  def pending!: -> bool
  def self.pending: -> ActiveRecord::Relation[EnumRbsReader]
  def shipped?: -> bool
  def shipped!: -> bool
  def self.shipped: -> ActiveRecord::Relation[EnumRbsReader]
  def status_value: -> Symbol
end
```

## Keep enum readers as strings when mappings use constants

### update

```ruby
class ConstantEnumReader
  STATUSES = { pending: 0, shipped: 1 }.with_indifferent_access.freeze
  enum :status, STATUSES

  def status_symbol = status.to_sym
end
```

### result

```rbs
class ConstantEnumReader
  STATUSES: { pending: 0, shipped: 1 }

  def status: -> String
  def self.statuses: -> Hash[String, untyped]
  def pending?: -> bool
  def pending!: -> bool
  def self.pending: -> ActiveRecord::Relation[ConstantEnumReader]
  def shipped?: -> bool
  def shipped!: -> bool
  def self.shipped: -> ActiveRecord::Relation[ConstantEnumReader]
  def status_symbol: -> Symbol
end
```

## Keep enum methods from a constructed constant mapping

### update

```ruby
class HashWithIndifferentAccess; end

class ConstructedEnumReader
  ACTIONS = HashWithIndifferentAccess.new(pending: 0, shipped: 1).freeze
  enum :status, ACTIONS, suffix: true

  def status_symbol = status.to_sym
end
```

### result

```rbs
class ConstructedEnumReader
  ACTIONS: HashWithIndifferentAccess

  def status: -> String
  def self.statuses: -> Hash[String, untyped]
  def pending_status?: -> bool
  def pending_status!: -> bool
  def self.pending_status: -> ActiveRecord::Relation[ConstructedEnumReader]
  def shipped_status?: -> bool
  def shipped_status!: -> bool
  def self.shipped_status: -> ActiveRecord::Relation[ConstructedEnumReader]
  def status_symbol: -> Symbol
end
```

## Ignore namespaced HashWithIndifferentAccess lookalikes

### update

```ruby
module Alternative
  class HashWithIndifferentAccess; end
end

class NamespacedHashEnum
  ACTIONS = Alternative::HashWithIndifferentAccess.new(pending: 0)
  enum :status, ACTIONS
end
```

### result

```rbs
class NamespacedHashEnum
  ACTIONS: Alternative::HashWithIndifferentAccess

  def status: -> String
  def self.statuses: -> Hash[String, untyped]
end
```

## Ignore bare namespaced HashWithIndifferentAccess lookalikes

### update

```ruby
module Alternative
  class HashWithIndifferentAccess; end

  class BareHashEnum
    ACTIONS = HashWithIndifferentAccess.new(pending: 0)
    enum :status, ACTIONS
  end
end
```

### result

```rbs
class Alternative::BareHashEnum
  ACTIONS: Alternative::HashWithIndifferentAccess

  def status: -> String
  def self.statuses: -> Hash[String, untyped]
end
```

## Ignore inherited bare namespaced HashWithIndifferentAccess lookalikes

### update

```ruby
module Alternative
  class HashWithIndifferentAccess; end

  class Parent
    ACTIONS = HashWithIndifferentAccess.new(pending: 0)
  end
end

class InheritedBareHashEnum < Alternative::Parent
  enum :status, ACTIONS
end
```

### result

```rbs
class Alternative::Parent
  ACTIONS: Alternative::HashWithIndifferentAccess
end

class InheritedBareHashEnum < Alternative::Parent
  def status: -> String
  def self.statuses: -> Hash[String, untyped]
end
```

## Override the integer schema type for enum readers

`db/schema.rb`

```ruby
ActiveRecord::Schema.define(version: 2024_01_01) do
  create_table "enum_columns" do |t|
    t.integer "status"
  end
end
```

### update

```ruby
class ActiveRecord::Base; end

class EnumColumn < ActiveRecord::Base
  enum :status, { pending: 0, shipped: 1 }

  def status_symbol = status.to_sym
end
```

### result

```rbs
class EnumColumn < ActiveRecord::Base
  def status: -> String
  def self.statuses: -> Hash[String, untyped]
  def pending?: -> bool
  def pending!: -> bool
  def self.pending: -> ActiveRecord::Relation[EnumColumn]
  def shipped?: -> bool
  def shipped!: -> bool
  def self.shipped: -> ActiveRecord::Relation[EnumColumn]
  def status_symbol: -> Symbol
end
```

## Keep the enum reader type for parameter receivers

`db/schema.rb`

```ruby
ActiveRecord::Schema.define(version: 2024_01_01) do
  create_table "enum_receivers" do |t|
    t.integer "status"
    t.integer "count"
  end
end
```

### update

```ruby
class ActiveRecord::Base; end

class EnumReceiver < ActiveRecord::Base
  enum :status, { pending: 0, shipped: 1 }
end

class EnumReader
  def status_of(record) = record&.status || "initial"
  def count_of(record) = record.count

  def read = status_of(EnumReceiver.new)
  def read_count = count_of(EnumReceiver.new)
end
```

### result

```rbs
class EnumReader
  def status_of: (EnumReceiver record) -> String
  def count_of: (EnumReceiver record) -> Integer?
  def read: -> String
  def read_count: -> Integer?
end

class EnumReceiver < ActiveRecord::Base
  def status: -> String
  def self.statuses: -> Hash[String, untyped]
  def pending?: -> bool
  def pending!: -> bool
  def self.pending: -> ActiveRecord::Relation[EnumReceiver]
  def shipped?: -> bool
  def shipped!: -> bool
  def self.shipped: -> ActiveRecord::Relation[EnumReceiver]
end
```

## Apply a named enum prefix to string values

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Post < ApplicationRecord
  enum :kind, { note: "note", article: "article" }, prefix: :kind_of, validate: true, default: :note
end
```

### result

```rbs
class Post < ApplicationRecord
  def kind: -> String
  def self.kinds: -> Hash[String, untyped]
  def kind_of_note?: -> bool
  def kind_of_note!: -> bool
  def self.kind_of_note: -> ActiveRecord::Relation[Post]
  def kind_of_article?: -> bool
  def kind_of_article!: -> bool
  def self.kind_of_article: -> ActiveRecord::Relation[Post]
end
```
