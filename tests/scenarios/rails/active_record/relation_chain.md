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

## Creating and strict finders return the model

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
end
```

```ruby
class PostFinder
  def found_or_created = Post.find_or_create_by(title: "a")
  def found_or_created! = Post.find_or_create_by!(title: "a")
  def first_strict = Post.where(id: 1).first!
  def last_strict = Post.where(id: 1).last!
  def taken = Post.take
  def taken! = Post.take!
  def only = Post.where(id: 1).sole
  def only_by = Post.find_sole_by(title: "a")
end
```

### result

```rbs
class PostFinder
  def found_or_created: -> Post
  def found_or_created!: -> Post
  def first_strict: -> Post
  def last_strict: -> Post
  def taken: -> Post?
  def taken!: -> Post
  def only: -> Post
  def only_by: -> Post
end
```

## Take leading and trailing records with a count as arrays

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
end
```

```ruby
class PostFinder
  def leading = Post.order(:id).first(3)
  def trailing = Post.order(:id).last(3)
  def single = Post.order(:id).last
end
```

### result

```rbs
class PostFinder
  def leading: -> Array[Post]
  def trailing: -> Array[Post]
  def single: -> Post?
end
```

## Keep the relation through combining and filtering builders

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/account.rb

```ruby
class Account < ApplicationRecord
end
```

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
  belongs_to :account
end
```

```ruby
class PostFinder
  def empty_set = Post.none
  def either = Post.where(id: 1).or(Post.where(id: 2))
  def merged = Post.joins(:account).merge(Account.where(name: "a"))
  def orphaned = Post.where.missing(:account)
  def attached = Post.where.associated(:account)
  def reversed = Post.order(:id).reverse_order
  def unfiltered = Post.where(id: 1).unscope(:where)
end
```

### result

```rbs
class PostFinder
  def empty_set: -> ActiveRecord::Relation[Post]
  def either: -> ActiveRecord::Relation[Post]
  def merged: -> ActiveRecord::Relation[Post]
  def orphaned: -> ActiveRecord::Relation[Post]
  def attached: -> ActiveRecord::Relation[Post]
  def reversed: -> ActiveRecord::Relation[Post]
  def unfiltered: -> ActiveRecord::Relation[Post]
end
```

## destroy_all on a relation returns the destroyed records

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
end
```

```ruby
class PostCleaner
  def purge = Post.where(id: 1).destroy_all
end
```

### result

```rbs
class PostCleaner
  def purge: -> Array[Post]
end
```

## Map a relation with a symbol proc to the column type

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "posts", force: :cascade do |t|
    t.string "title", null: false
  end
end
```

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
end
```

```ruby
class PostTitles
  def titles = Post.where(id: 1).map(&:title)
end
```

### result

```rbs
class PostTitles
  def titles: -> Array[String]
end
```

## Relation count predicates return bool

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
end
```

```ruby
class PostChecks
  def none = Post.where(id: 1).none?
  def many = Post.where(id: 1).many?
  def one = Post.where(id: 1).one?
  def by_id = Post.exists?(1)
end
```

### result

```rbs
class PostChecks
  def none: -> bool
  def many: -> bool
  def one: -> bool
  def by_id: -> bool
end
```
