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

## pluck on records loaded by a block filter reads the column type

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "emails", force: :cascade do |t|
    t.string "email", null: false
    t.boolean "confirmed", null: false
  end
end
```

### file: app/models/email.rb

```ruby
class Email < ApplicationRecord
end
```

```ruby
class EmailList
  def confirmed_addresses = Email.all.select(&:confirmed?).pluck(:email)
  def first_address = Email.all.select(&:confirmed?).pick(:email)
end
```

### result

```rbs
class EmailList
  def confirmed_addresses: -> Array[String]
  def first_address: -> String?
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

## Calculations on a grouped relation return a hash

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "items", force: :cascade do |t|
    t.integer "position", null: false
    t.string "kind", null: false
  end
end
```

### file: app/models/item.rb

```ruby
class Item < ApplicationRecord
end
```

```ruby
class ItemStats
  def last_positions = Item.where(id: 1).group(:kind).maximum(:position)
  def counts = Item.group(:kind).count
end
```

### result

```rbs
class ItemStats
  def last_positions: -> Hash[untyped, untyped]
  def counts: -> Hash[untyped, Integer]
end
```

## Query methods called on a subclass return the subclass

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "items", force: :cascade do |t|
    t.string "name"
    t.string "type"
  end
end
```

### file: app/models/item.rb

```ruby
class Item < ApplicationRecord
  scope :named, -> { where.not(name: nil) }
end
```

### file: app/models/special_item.rb

```ruby
class SpecialItem < Item
end
```

```ruby
class SpecialItemFinder
  def by_where = SpecialItem.where(name: "x").first
  def by_scope = SpecialItem.named.first
end
```

### result

```rbs
class SpecialItemFinder
  def by_where: -> SpecialItem?
  def by_scope: -> SpecialItem?
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

## delete and destroy follow the relation or association proxy contract

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
class PostRemoval
  def removed(post) = Account.find(1).posts.destroy(post)
  def deleted(post) = Account.find(1).posts.delete(post)
  def appended(post) = Account.find(1).posts << post
  def deleted_count = Post.where(id: 1).delete(1)
  def destroyed = Post.where(id: 1).destroy(1)
end
```

### result

```rbs
class PostRemoval
  def removed: (untyped post) -> Array[Post]
  def deleted: (untyped post) -> Array[Post]
  def appended: (untyped post) -> ActiveRecord::Associations::CollectionProxy[Post]
  def deleted_count: -> Integer
  def destroyed: -> Post
end
```
