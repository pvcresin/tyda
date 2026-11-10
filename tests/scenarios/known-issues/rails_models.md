# Known Issues / Rails Models

## Relation to_a in a model with an association returns an array of the model

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Post < ApplicationRecord
  belongs_to :account

  def everything = Post.all.to_a
  def filtered = Post.where(id: 1).order(:id).limit(3).to_a
end
```

### result

```rbs
class Post < ApplicationRecord
  def account: -> Account
  def account=: (Account account) -> Account
  def build_account: (?Hash[String | Symbol, untyped] attributes) -> Account
  def create_account: (?Hash[String | Symbol, untyped] attributes) -> Account
  def create_account!: (?Hash[String | Symbol, untyped] attributes) -> Account
  def everything: -> Array[Post]
  def filtered: -> Array[Post]
end
```

## Association proxy to_a inside the owner returns an array of the target

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
class Account < ApplicationRecord
  has_many :posts

  def post_records = posts.to_a
end
```

### result

```rbs
class Account < ApplicationRecord
  def posts: -> ActiveRecord::Associations::CollectionProxy[Post]
  def post_ids: -> Array[Integer]
  def post_ids=: (Array[Integer] post_ids) -> Array[Integer]
  def posts=: (Array[Post] posts) -> ActiveRecord::Associations::CollectionProxy[Post]
  def post_records: -> Array[Post]
end
```

## Relation records returns an array of the model

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
class PostRecords
  def loaded = Post.where(id: 1).records
end
```

### result

```rbs
class PostRecords
  def loaded: -> Array[Post]
end
```

## Relation load returns the relation

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
class PostRecords
  def preloaded = Post.where(id: 1).load
end
```

### result

```rbs
class PostRecords
  def preloaded: -> ActiveRecord::Relation[Post]
end
```

## empty? on a relation and an association proxy returns bool

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/account.rb

```ruby
class Account < ApplicationRecord
  has_many :posts
end
```

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
end
```

```ruby
class PostChecks
  def no_posts = Post.where(id: 1).empty?
  def no_account_posts = Account.find(1).posts.empty?
end
```

### result

```rbs
class PostChecks
  def no_posts: -> bool
  def no_account_posts: -> bool
end
```

## Relation map passes the model to the block

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
  def titles = Post.where(id: 1).map { |post| post.title }
end
```

### result

```rbs
class PostTitles
  def titles: -> Array[String]
end
```

## pick returns the nilable column type

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "terms", force: :cascade do |t|
    t.date "starts_on", null: false
  end
end
```

### file: app/models/term.rb

```ruby
class Term < ApplicationRecord
end
```

```ruby
class TermDates
  def first_start = Term.order(:id).pick(:starts_on)
end
```

### result

```rbs
class TermDates
  def first_start: -> Date?
end
```

## maximum and minimum return the nilable column type

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "items", force: :cascade do |t|
    t.integer "position", null: false
  end
end
```

### file: app/models/item.rb

```ruby
class Item < ApplicationRecord
end
```

```ruby
class ItemPositions
  def last_position = Item.where(id: 1).maximum(:position)
  def first_position = Item.where(id: 1).minimum(:position)
end
```

### result

```rbs
class ItemPositions
  def last_position: -> Integer?
  def first_position: -> Integer?
end
```

## unscoped without a block returns a relation

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
  def every_post = Post.unscoped
end
```

### result

```rbs
class PostFinder
  def every_post: -> ActiveRecord::Relation[Post]
end
```

## first_or_create! and first_or_initialize return the model

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
  def first_or_new = Post.where(id: 1).first_or_initialize
  def first_or_saved = Post.where(id: 1).first_or_create!
end
```

### result

```rbs
class PostFinder
  def first_or_new: -> Post
  def first_or_saved: -> Post
end
```

## reload on an association proxy returns the proxy

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/account.rb

```ruby
class Account < ApplicationRecord
  has_many :posts
end
```

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
end
```

```ruby
class AccountPosts
  def fresh = Account.find(1).posts.reload
end
```

### result

```rbs
class AccountPosts
  def fresh: -> ActiveRecord::Associations::CollectionProxy[Post]
end
```

## Enum values generate negative scopes

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
  enum :status, { draft: 0, live: 1 }
end
```

```ruby
class PostFinder
  def not_drafts = Post.not_draft
  def not_live = Post.not_live
end
```

### result

```rbs
class PostFinder
  def not_drafts: -> ActiveRecord::Relation[Post]
  def not_live: -> ActiveRecord::Relation[Post]
end
```

## has_many in a namespaced model resolves the sibling model

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/admin/entry.rb

```ruby
module Admin
  class Entry < ApplicationRecord
  end
end
```

### file: app/models/admin/report.rb

```ruby
module Admin
  class Report < ApplicationRecord
    has_many :entries
  end
end
```

```ruby
class ReportEntries
  def first_entry = Admin::Report.find(1).entries.first
end
```

### result

```rbs
class ReportEntries
  def first_entry: -> Admin::Entry?
end
```

## Polymorphic belongs_to columns keep schema nullability

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "comments", force: :cascade do |t|
    t.string "commentable_type"
    t.bigint "commentable_id"
  end
end
```

```ruby
class Comment < ApplicationRecord
  belongs_to :commentable, polymorphic: true, optional: true
end
```

### result

```rbs
class Comment < ApplicationRecord
  def commentable: -> untyped
  def commentable=: (untyped commentable) -> untyped
  def commentable_type: -> String?
  def commentable_id: -> Integer?
end
```

## normalize_value_for returns the normalized value type

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
  normalizes :title, with: ->(title) { title.strip }
end
```

```ruby
class PostTitles
  def normalized = Post.normalize_value_for(:title, " a ")
end
```

### result

```rbs
class PostTitles
  def normalized: -> String
end
```
