# Rails / DSL / Active Record Persistence

## Resolve instance persistence methods

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  def persist = save

  def persist! = save!

  def rename(name) = update(name: name)

  def refresh = reload

  def remove = destroy

  def saved? = persisted?
end
```

### result

```rbs
class User < ActiveRecord::Base
  def persist: -> bool
  def persist!: -> bool
  def rename: (untyped name) -> bool
  def refresh: -> User
  def remove: -> User
  def saved?: -> bool
end
```

## Resolve legacy dynamic finders

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base; end

class UserLookup
  def by_email(email) = User.find_by_email(email)

  def by_email!(email) = User.find_by_email!(email)

  def all_admins = User.find_all_by_role('admin')
end
```

### result

```rbs
class UserLookup
  def by_email: (untyped email) -> User?
  def by_email!: (untyped email) -> User
  def all_admins: -> Array[User]
end
```

## Resolve destructive relation methods

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  def self.deactivate_all = where(active: true).update_all(active: false)

  def self.purge = where(active: false).delete_all
end
```

### result

```rbs
class User < ActiveRecord::Base
  def self.deactivate_all: -> Integer
  def self.purge: -> Integer
end
```

## Resolve batch enumerator and class metadata

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  def self.bulk_disable = in_batches.update_all(active: false)

  def self.table = table_name
end
```

### result

```rbs
class User < ActiveRecord::Base
  def self.bulk_disable: -> Integer
  def self.table: -> String
end
```
