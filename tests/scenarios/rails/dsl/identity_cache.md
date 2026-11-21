# Rails / DSL / Identity Cache

## cache_index joins fields and returns an array when not unique

```yaml
known_issue: true
```

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  include IdentityCache

  cache_index :first_name, :last_name
end
```

### result

```rbs
class User < ActiveRecord::Base
  include IdentityCache

  def self.fetch_by_first_name_and_last_name: (untyped first_name, untyped last_name) -> Array[User]
  def self.fetch_multi_by_first_name_and_last_name: (Array[untyped] index_values) -> Array[User]
end
```

## Unique cache_index returns a nilable model and adds a bang finder

```yaml
known_issue: true
```

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  include IdentityCache

  cache_index :email, unique: true
end
```

### result

```rbs
class User < ActiveRecord::Base
  include IdentityCache

  def self.fetch_by_email: (untyped email) -> User?
  def self.fetch_by_email!: (untyped email) -> User
  def self.fetch_multi_by_email: (Array[untyped] email) -> Array[User]
end
```

## cache_has_many defines an instance fetch method

```yaml
known_issue: true
```

### update

```ruby
class ActiveRecord::Base; end

class Order < ActiveRecord::Base; end

class User < ActiveRecord::Base
  include IdentityCache

  has_many :orders
  cache_has_many :orders, embed: true
end
```

### result

```rbs
class User < ActiveRecord::Base
  include IdentityCache

  def orders: -> ActiveRecord::Associations::CollectionProxy[Order]
  def order_ids: -> Array[Integer]
  def order_ids=: (Array[Integer] order_ids) -> Array[Integer]
  def orders=: (Array[Order] orders) -> ActiveRecord::Associations::CollectionProxy[Order]
  def fetch_orders: -> Array[Order]
end
```
