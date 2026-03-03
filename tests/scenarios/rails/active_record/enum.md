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
  def status_active?: -> bool
  def status_active!: -> bool
  def self.status_active: -> ActiveRecord::Relation[User]
  def status_archived?: -> bool
  def status_archived!: -> bool
  def self.status_archived: -> ActiveRecord::Relation[User]
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
  def status_active?: -> bool
  def status_active!: -> bool
  def self.status_active: -> ActiveRecord::Relation[C]
  def status_archived?: -> bool
  def status_archived!: -> bool
  def self.status_archived: -> ActiveRecord::Relation[C]
  def admin_role?: -> bool
  def admin_role!: -> bool
  def self.admin_role: -> ActiveRecord::Relation[C]
  def guest_role?: -> bool
  def guest_role!: -> bool
  def self.guest_role: -> ActiveRecord::Relation[C]
end
```
