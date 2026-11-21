# Known issues / Active Record Class Methods

## sole and find_sole_by are unresolved before Rails 7.0

```yaml
rails_version: 6.1.0
```

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  def self.only = sole

  def self.only_by(name) = find_sole_by(name: name)

  def self.only_active = where(active: true).sole
end
```

### result

```rbs
class User < ActiveRecord::Base
  def self.only: -> untyped
  def self.only_by: (untyped name) -> untyped
  def self.only_active: -> untyped
end
```

## sanitize_sql returns nil for blank conditions

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  def self.by_name(value) = sanitize_sql(["name = ?", value])

  def self.raw = sanitize_sql_for_conditions("name = 'a'")
end
```

### result

```rbs
class User < ActiveRecord::Base
  def self.by_name: (untyped value) -> String?
  def self.raw: -> String?
end
```

## primary_key can be composite or nil in Rails 7.1

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  def self.key = primary_key
end
```

### result

```rbs
class User < ActiveRecord::Base
  def self.key: -> (String | Array[String])?
end
```

## primary_key can be nil before Rails 7.1

```yaml
rails_version: 7.0.0
```

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  def self.key = primary_key
end
```

### result

```rbs
class User < ActiveRecord::Base
  def self.key: -> String?
end
```

## connects_to takes keywords and returns connection pools

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end

class ApplicationRecord < ActiveRecord::Base
  connects_to database: { writing: :primary }

  def self.setup = connects_to(database: { writing: :primary })

  def self.sharded = connects_to(shards: { default: { writing: :primary } })
end
```

### result

```rbs
class ApplicationRecord < ActiveRecord::Base
  def self.setup: -> Array[ActiveRecord::ConnectionAdapters::ConnectionPool]
  def self.sharded: -> Array[ActiveRecord::ConnectionAdapters::ConnectionPool]
end
```
