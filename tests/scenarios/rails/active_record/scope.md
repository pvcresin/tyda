# Rails / Active Record / Scope

## Generate class methods from scope

### update

```ruby
class Post
  scope :published, -> { where(published: true) }
  scope :recent, -> { order(created_at: :desc) }
end
```

### result

```rbs
class Post
  def self.published: -> ActiveRecord::Relation[Post]
  def self.recent: -> ActiveRecord::Relation[Post]
end
```

## Define a single scope

### update

```ruby
class Article
  scope :draft, -> { where(status: "draft") }
end
```

### result

```rbs
class Article
  def self.draft: -> ActiveRecord::Relation[Article]
end
```

## Generate scope with args as class method

### update

```ruby
class Post
  scope :visible_to, ->(account) { where(account_id: account.id) }
end
```

### result

```rbs
class Post
  def self.visible_to: (untyped account) -> ActiveRecord::Relation[Post]
end
```

## Preserve optional and keyword scope parameters

### update

```ruby
class Post
  scope :recent, ->(limit = 100, since: nil, **options) do
    where(created_at: since).limit(limit)
  end
end
```

### result

```rbs
class Post
  def self.recent: (?Integer limit, ?since: nil, **untyped options) -> ActiveRecord::Relation[Post]
end
```

## Resolve chain and first from scope relation

### update

```ruby
class Post < ApplicationRecord
  scope :published, -> { where(published: true) }

  def self.first_published = published.first
end
```

### result

```rbs
class Post < ApplicationRecord
  def self.published: -> ActiveRecord::Relation[Post]
  def self.first_published: -> Post?
end
```

## Preserve scope parameters and defaults

### update

```ruby
class Project
  scope :recently_contributed_by, ->(user, since: nil) do
    where(user: user, created_at: since)
  end
end
```

### result

```rbs
class Project
  def self.recently_contributed_by: (untyped user, ?since: nil) -> ActiveRecord::Relation[Project]
end
```

## Limit async relation methods in Rails 7.0

```yaml
rails_version: 7.0.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Post < ApplicationRecord
  scope :published, -> { where(published: true) }

  def self.warm_published = published.load_async

  def self.count_published_async = published.async_count
end
```

### result

```rbs
class Post < ApplicationRecord
  def self.published: -> ActiveRecord::Relation[Post]
  def self.warm_published: -> ActiveRecord::Relation[Post]
  def self.count_published_async: -> untyped
end
```

## Resolve async relation methods in Rails 7.1

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Post < ApplicationRecord
  scope :published, -> { where(published: true) }

  def self.warm_published = published.load_async

  def self.count_published_async = published.async_count
end
```

### result

```rbs
class Post < ApplicationRecord
  def self.published: -> ActiveRecord::Relation[Post]
  def self.warm_published: -> ActiveRecord::Relation[Post]
  def self.count_published_async: -> ActiveRecord::Promise[Integer]
end
```
