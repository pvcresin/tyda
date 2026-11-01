# Rails / Active Record / Relation Chain

## Preserve collection proxy count methods

`app/models/post.rb`

```ruby
class ActiveRecord::Base; end

class Comment < ActiveRecord::Base; end

class Post < ActiveRecord::Base
  has_many :comments
end
```

### update

```ruby
class CommentCounter
  def count(post) = post.comments.count
  def size(post) = post.comments.size
  def length(post) = post.comments.length
end

counter = CommentCounter.new
counter.count(Post.new)
counter.size(Post.new)
counter.length(Post.new)
```

### result

```rbs
class CommentCounter
  def count: (Post post) -> Integer
  def size: (Post post) -> Integer
  def length: (Post post) -> Integer
end
```

## Preserve relation chain through a terminal pluck

`db/schema.rb`

```ruby
ActiveRecord::Schema.define(version: 2024_01_01) do
  create_table "posts" do |t|
    t.string "title"
    t.boolean "published"
    t.datetime "created_at"
  end
end
```

### update

```ruby
class ActiveRecord::Base; end

class Post < ActiveRecord::Base
  def self.titles = where(published: true).order(created_at: :desc).limit(3).pluck(:title)
end
```

### result

```rbs
class Post < ActiveRecord::Base
  def self.titles: -> Array[String?]
end
```

## Preserve association proxy through a filtered pluck

`db/schema.rb`

```ruby
ActiveRecord::Schema.define(version: 2024_01_01) do
  create_table "users" do |t|
    t.string "name"
  end

  create_table "posts" do |t|
    t.string "title"
    t.boolean "published"
    t.integer "user_id"
  end
end
```

### update

```ruby
class ActiveRecord::Base; end

class Post < ActiveRecord::Base; end

class User < ActiveRecord::Base
  has_many :posts

  def post_titles = posts.where(published: true).pluck(:title)
end
```

### result

```rbs
class User < ActiveRecord::Base
  def posts: -> ActiveRecord::Associations::CollectionProxy[Post]
  def post_ids: -> Array[Integer]
  def post_ids=: (Array[Integer] post_ids) -> Array[Integer]
  def posts=: (Array[Post] posts) -> ActiveRecord::Associations::CollectionProxy[Post]
  def post_titles: -> Array[String?]
end
```

## Destructure nullable multi-column pluck rows

`db/schema.rb`

```ruby
ActiveRecord::Schema.define(version: 2024_01_01) do
  create_table "statuses" do |t|
    t.integer "ordered_media_attachment_ids", array: true
  end
end
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Status < ApplicationRecord
  def self.with_discarded = all
end

class Report < ApplicationRecord
  def status_relation = Status.with_discarded
  def status_rows = Status.with_discarded.where(id: [1]).pluck(:id, :ordered_media_attachment_ids)

  def media_attachment_count
    total = 0
    Status.with_discarded.where(id: [1]).pluck(:id, :ordered_media_attachment_ids).each do |id, ordered_ids|
      total += ordered_ids ? ordered_ids.length : id
    end
    total
  end
end
```

### result

```rbs
class Report < ApplicationRecord
  def status_relation: -> ActiveRecord::Relation[Status]
  def status_rows: -> Array[[Integer, Array[Integer]?]]
  def media_attachment_count: -> Integer
end

class Status < ApplicationRecord
  def self.with_discarded: -> ActiveRecord::Relation[Status]
end
```

## Resolve a relation across model files

`db/schema.rb`

```ruby
ActiveRecord::Schema.define(version: 2024_01_01) do
  create_table "statuses" do |t|
    t.integer "ordered_media_attachment_ids", array: true
  end
end
```

`app/models/status.rb`

```ruby
class Status < ApplicationRecord
  def self.with_discarded = all
end
```

### update

```ruby
class Report < ApplicationRecord
  def statuses = Status.with_discarded.where(id: [1])
  def status_rows = statuses.pluck(:id, :ordered_media_attachment_ids)
end
```

### result

```rbs
class Report < ApplicationRecord
  def statuses: -> ActiveRecord::Relation[Status]
  def status_rows: -> Array[[Integer, Array[Integer]?]]
end
```
