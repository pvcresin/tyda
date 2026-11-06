# Rails / Active Record / Model

## Model with multiple DSLs

### update

```ruby
class Product
  belongs_to :category
  has_many :reviews
  has_one :detail

  scope :available, -> { where(available: true) }
  scope :featured, -> { where(featured: true) }

  enum status: { draft: 0, published: 1, archived: 2 }

  delegate :name, to: :category, prefix: true
end
```

### result

```rbs
class Product
  def category: -> Category
  def category=: (Category category) -> Category
  def build_category: (?Hash[String | Symbol, untyped] attributes) -> Category
  def create_category: (?Hash[String | Symbol, untyped] attributes) -> Category
  def create_category!: (?Hash[String | Symbol, untyped] attributes) -> Category
  def reviews: -> ActiveRecord::Associations::CollectionProxy[Review]
  def review_ids: -> Array[Integer]
  def review_ids=: (Array[Integer] review_ids) -> Array[Integer]
  def reviews=: (Array[Review] reviews) -> ActiveRecord::Associations::CollectionProxy[Review]
  def detail: -> Detail?
  def detail=: (Detail detail) -> Detail
  def build_detail: (?Hash[String | Symbol, untyped] attributes) -> Detail
  def create_detail: (?Hash[String | Symbol, untyped] attributes) -> Detail
  def create_detail!: (?Hash[String | Symbol, untyped] attributes) -> Detail
  def self.available: -> ActiveRecord::Relation[Product]
  def self.featured: -> ActiveRecord::Relation[Product]
  def status: -> String
  def self.statuses: -> Hash[String, untyped]
  def draft?: -> bool
  def draft!: -> bool
  def self.draft: -> ActiveRecord::Relation[Product]
  def published?: -> bool
  def published!: -> bool
  def self.published: -> ActiveRecord::Relation[Product]
  def archived?: -> bool
  def archived!: -> bool
  def self.archived: -> ActiveRecord::Relation[Product]
  def category_name: -> untyped
end
```

## Combine belongs_to and has_many

### update

```ruby
class Post
  belongs_to :author, class_name: "User"
  has_many :comments
  has_many :tags

  scope :recent, -> { order(created_at: :desc) }
end
```

### result

```rbs
class Post
  def author: -> User
  def author=: (User author) -> User
  def build_author: (?Hash[String | Symbol, untyped] attributes) -> User
  def create_author: (?Hash[String | Symbol, untyped] attributes) -> User
  def create_author!: (?Hash[String | Symbol, untyped] attributes) -> User
  def comments: -> ActiveRecord::Associations::CollectionProxy[Comment]
  def comment_ids: -> Array[Integer]
  def comment_ids=: (Array[Integer] comment_ids) -> Array[Integer]
  def comments=: (Array[Comment] comments) -> ActiveRecord::Associations::CollectionProxy[Comment]
  def tags: -> ActiveRecord::Associations::CollectionProxy[Tag]
  def tag_ids: -> Array[Integer]
  def tag_ids=: (Array[Integer] tag_ids) -> Array[Integer]
  def tags=: (Array[Tag] tags) -> ActiveRecord::Associations::CollectionProxy[Tag]
  def self.recent: -> ActiveRecord::Relation[Post]
end
```

## Keep plain methods with DSL methods

### update

```ruby
class Invoice
  belongs_to :customer
  has_many :line_items

  def total
    items = []
    items
  end

  def paid?
    items = []
    !items.empty?
  end
end
```

### result

```rbs
class Invoice
  def customer: -> Customer
  def customer=: (Customer customer) -> Customer
  def build_customer: (?Hash[String | Symbol, untyped] attributes) -> Customer
  def create_customer: (?Hash[String | Symbol, untyped] attributes) -> Customer
  def create_customer!: (?Hash[String | Symbol, untyped] attributes) -> Customer
  def line_items: -> ActiveRecord::Associations::CollectionProxy[LineItem]
  def line_item_ids: -> Array[Integer]
  def line_item_ids=: (Array[Integer] line_item_ids) -> Array[Integer]
  def line_items=: (Array[LineItem] line_items) -> ActiveRecord::Associations::CollectionProxy[LineItem]
  def total: -> [ ]
  def paid?: -> false
end
```

## Treat attribute and store_accessor as accessors

```yaml
rails_version: 5.2.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Profile < ApplicationRecord
  attribute :nickname, :string
  store_accessor :settings, :theme, :locale
end
```

### result

```rbs
class Profile < ApplicationRecord
  def nickname: -> String?
  def nickname=: (String? nickname) -> String?
  def nickname_changed?: -> bool
  def nickname_previously_changed?: -> bool
  def saved_change_to_nickname?: -> bool
  def will_save_change_to_nickname?: -> bool
  def nickname_change: -> [String?, String?]
  def nickname_was: -> String?
  def nickname_previously_was: -> String?
  def nickname_before_last_save: -> String?
  def nickname_in_database: -> String?
  def nickname_previous_change: -> Array[String?]?
  def nickname_change_to_be_saved: -> Array[String?]?
  def saved_change_to_nickname: -> Array[String?]?
  def nickname_will_change!: -> void
  def restore_nickname!: -> void
  def clear_nickname_change: -> void
  def theme: -> untyped
  def theme=: (untyped theme) -> untyped
  def locale: -> untyped
  def locale=: (untyped locale) -> untyped
end
```

## Untyped attribute on AR model uses schema column type

```yaml
rails_version: 5.2.0
```

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "profiles", force: :cascade do |t|
    t.string "title"
  end
end
```

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Profile < ApplicationRecord
  attribute :title
end

def result = Profile.new.title
```

### result

```rbs
class Object < BasicObject
  def result: -> String?
end
```

## Apply encrypts normalizes and generates_token_for

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class User < ApplicationRecord
  encrypts :email
  has_encrypted :ssn
  normalizes :email, with: -> value { value&.strip }
  generates_token_for :password_reset
end
```

### result

```rbs
class User < ApplicationRecord
  def email: -> untyped
  def email=: (untyped email) -> untyped
  def ssn: -> untyped
  def ssn=: (untyped ssn) -> untyped
  def self.normalize_value_for: (Symbol name, untyped value) -> untyped
  def generate_token_for: (Symbol purpose) -> String
  def self.find_by_token_for: (Symbol purpose, String token) -> User?
  def self.find_by_token_for!: (Symbol purpose, String token) -> User
end
```

## Apply delegated_type convenience methods

```yaml
rails_version: 6.1.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Entry < ApplicationRecord
  delegated_type :entryable, types: %w[ Message Comment ]
end
```

### result

```rbs
class Entry < ApplicationRecord
  def entryable: -> Comment | Message
  def entryable=: ((Comment | Message) entryable) -> (Comment | Message)
  def entryable_class: -> Class
  def entryable_name: -> String
  def message?: -> bool
  def message: -> Message?
  def message_id: -> Integer?
  def self.messages: -> ActiveRecord::Relation[Entry]
  def comment?: -> bool
  def comment: -> Comment?
  def comment_id: -> Integer?
  def self.comments: -> ActiveRecord::Relation[Entry]
end
```

## Apply Rails 6.0 bulk persistence methods

```yaml
rails_version: 6.0.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base
  connects_to database: { writing: :primary }
end

class User < ApplicationRecord
  def self.bulk_insert(rows) = insert_all(rows)

  def self.bulk_upsert(rows) = upsert_all(rows)
end
```

### result

```rbs
class User < ApplicationRecord
  def self.bulk_insert: (untyped rows) -> ActiveRecord::Result
  def self.bulk_upsert: (untyped rows) -> ActiveRecord::Result
end
```

## Rails 7.0 skips Rails 7.1 normalization and token helpers

```yaml
rails_version: 7.0.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class User < ApplicationRecord
  normalizes :email, with: -> value { value&.strip }
  generates_token_for :password_reset
end
```

### result

```rbs
```

## Resolve Rails 7.1 normalize_value_for through self.class

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class User < ApplicationRecord
  normalizes :email, with: -> value { value&.strip }

  def normalized_literal = self.class.normalize_value_for(:email, "demo@example.com")
end
```

### result

```rbs
class User < ApplicationRecord
  def email: -> untyped
  def email=: (untyped email) -> untyped
  def self.normalize_value_for: (Symbol name, untyped value) -> String
  def normalized_literal: -> "demo@example.com"
end
```

## Keep relation return for Rails 7.1 Arel chains

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Tag < ApplicationRecord
  class << self
    def matching_name(name) = where(arel_table[:name].lower.eq(arel_table.lower(name)))
  end
end
```

### result

```rbs
class Tag < ApplicationRecord
  def self.matching_name: (untyped name) -> ActiveRecord::Relation[Tag]
end
```

## Resolve update_attributes like update in Rails 6.0

```yaml
rails_version: 6.0.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end
class User < ApplicationRecord; end

#: (User user) -> bool
def persist(user) = user.update_attributes(name: "x")
```

### result

```rbs
class Object < BasicObject
  def persist: (User user) -> bool
end
```

## Keep update_attributes alias in Rails 5.2

```yaml
rails_version: 5.2.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end
class User < ApplicationRecord; end

#: (User user) -> bool
def persist(user) = user.update_attributes(name: "x")
```

### result

```rbs
class Object < BasicObject
  def persist: (User user) -> bool
end
```

## Apply ActiveRecord relation finder and builder class methods

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end

class A < ActiveRecord::Base
  def self.foo = find_or_initialize_by(key: "x")

  def self.bar = first

  def self.baz = [count, ids, pluck(:key), any?]

  def self.qux = where.not(key: nil).rewhere(key: "x").exists?

  def self.zap! = second!

  def self.each1 = find_each(batch_size: 10)
end
```

### result

```rbs
class A < ActiveRecord::Base
  def self.foo: -> A
  def self.bar: -> A?
  def self.baz: -> [Integer, Array[Integer], Array[untyped], bool]
  def self.qux: -> bool
  def self.zap!: -> A
  def self.each1: -> Enumerator[A]
end
```

## find returns the model type

`db/schema.rb`

```ruby
ActiveRecord::Schema.define(version: 2024_01_01) do
  create_table "posts" do |t|
    t.string "title"
    t.integer "author_id"
  end
end
```

```ruby
class Post < ApplicationRecord
end

class PostService
  def find_post(id) = Post.find(id)
  def find_and_title(id) = Post.find(id).title
end
```

```rbs
class Post < ApplicationRecord
end

class PostService
  def find_post: (untyped id) -> Post
  def find_and_title: (untyped id) -> String?
end
```

## find with many ids returns Array[Model]

`db/schema.rb`

```ruby
ActiveRecord::Schema.define(version: 2024_01_01) do
  create_table "posts" do |t|
    t.string "title"
  end
end
```

```ruby
class Post < ApplicationRecord
end

class PostService
  def find_array = Post.find([1, 2, 3])
  def find_many = Post.find(1, 2, 3)
  def find_splat_single = Post.find(*[1])
  def find_splat_many = Post.find(*[1, 2])
  def first_n = Post.first(5)
  def last_n = Post.last(3)
end
```

```rbs
class Post < ApplicationRecord
end

class PostService
  def find_array: -> Array[Post]
  def find_many: -> Array[Post]
  def find_splat_single: -> Post
  def find_splat_many: -> Array[Post]
  def first_n: -> Array[Post]
  def last_n: -> Array[Post]
end
```

## Associations inside with_options

### update

```ruby
class Order
  with_options dependent: :destroy do
    has_many :items
    has_one :invoice
  end
end
```

### result

```rbs
class Order
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[Item] items) -> ActiveRecord::Associations::CollectionProxy[Item]
  def invoice: -> Invoice?
  def invoice=: (Invoice invoice) -> Invoice
  def build_invoice: (?Hash[String | Symbol, untyped] attributes) -> Invoice
  def create_invoice: (?Hash[String | Symbol, untyped] attributes) -> Invoice
  def create_invoice!: (?Hash[String | Symbol, untyped] attributes) -> Invoice
end
```

## Nested associations inside with_options

### update

```ruby
class Project
  with_options dependent: :destroy do
    with_options inverse_of: :project do
      has_many :tasks
      has_one :owner, class_name: 'User'
    end
  end
end
```

### result

```rbs
class Project
  def tasks: -> ActiveRecord::Associations::CollectionProxy[Task]
  def task_ids: -> Array[Integer]
  def task_ids=: (Array[Integer] task_ids) -> Array[Integer]
  def tasks=: (Array[Task] tasks) -> ActiveRecord::Associations::CollectionProxy[Task]
  def owner: -> User?
  def owner=: (User owner) -> User
  def build_owner: (?Hash[String | Symbol, untyped] attributes) -> User
  def create_owner: (?Hash[String | Symbol, untyped] attributes) -> User
  def create_owner!: (?Hash[String | Symbol, untyped] attributes) -> User
end
```

## Direct Active Record class queries return Relation or Model

### update

```ruby
class Post < ApplicationRecord
  belongs_to :author, class_name: 'User'
end

class PostManager
  def recent_posts = Post.where(published: true)
  def find_post(id) = Post.find(id)
  def find_many_posts = Post.find(1, 2)
  def find_posts_by_ids = Post.find([1, 2])
  def first_post = Post.first
  def post_count = Post.count
  def any_posts? = Post.exists?
end
```

### result

```rbs
class Post < ApplicationRecord
  def author: -> User
  def author=: (User author) -> User
  def build_author: (?Hash[String | Symbol, untyped] attributes) -> User
  def create_author: (?Hash[String | Symbol, untyped] attributes) -> User
  def create_author!: (?Hash[String | Symbol, untyped] attributes) -> User
end

class PostManager
  def recent_posts: -> ActiveRecord::Relation[Post]
  def find_post: (untyped id) -> Post
  def find_many_posts: -> Array[Post]
  def find_posts_by_ids: -> Array[Post]
  def first_post: -> Post?
  def post_count: -> Integer
  def any_posts?: -> bool
end
```

## Scope methods chain on Relation

### update

```ruby
class Article < ApplicationRecord
  scope :published, -> { where(published: true) }
  scope :recent, -> { order(created_at: :desc) }
end

class ArticleFinder
  def published_recent = Article.published.recent
end
```

### result

```rbs
class Article < ApplicationRecord
  def self.published: -> ActiveRecord::Relation[Article]
  def self.recent: -> ActiveRecord::Relation[Article]
end

class ArticleFinder
  def published_recent: -> ActiveRecord::Relation[Article]
end
```

## with_options passes class_name and optional to belongs_to

### update

```ruby
class Appeal < ApplicationRecord
  with_options class_name: 'Account', optional: true do
    belongs_to :approved_by_account
    belongs_to :rejected_by_account
  end
end
```

### result

```rbs
class Appeal < ApplicationRecord
  def approved_by_account: -> Account?
  def approved_by_account=: (Account approved_by_account) -> Account
  def build_approved_by_account: (?Hash[String | Symbol, untyped] attributes) -> Account
  def create_approved_by_account: (?Hash[String | Symbol, untyped] attributes) -> Account
  def create_approved_by_account!: (?Hash[String | Symbol, untyped] attributes) -> Account
  def rejected_by_account: -> Account?
  def rejected_by_account=: (Account rejected_by_account) -> Account
  def build_rejected_by_account: (?Hash[String | Symbol, untyped] attributes) -> Account
  def create_rejected_by_account: (?Hash[String | Symbol, untyped] attributes) -> Account
  def create_rejected_by_account!: (?Hash[String | Symbol, untyped] attributes) -> Account
end
```

## Rails 6.0 skips newer relation methods

```yaml
rails_version: 6.0.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Post < ApplicationRecord
  def self.strict_test = strict_loading
  def self.with_test = with(foo: "bar")
  def self.in_order_test = in_order_of(:status, ["draft", "published"])
  def self.invert_test = invert_where
  def self.excluding_test = excluding(first)
  def self.with_recursive_test = with_recursive(foo: "bar")
  def self.regroup_test = regroup(:category)
end
```

### result

```rbs
class Post < ApplicationRecord
  def self.strict_test: -> untyped
  def self.with_test: -> untyped
  def self.in_order_test: -> untyped
  def self.invert_test: -> untyped
  def self.excluding_test: -> untyped
  def self.with_recursive_test: -> untyped
  def self.regroup_test: -> untyped
end
```

## Rails 6.1 supports strict_loading

```yaml
rails_version: 6.1.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Post < ApplicationRecord
  def self.strict_test = strict_loading(true)
  def self.with_test = with(foo: "bar")
end
```

### result

```rbs
class Post < ApplicationRecord
  def self.strict_test: -> ActiveRecord::Relation[Post]
  def self.with_test: -> untyped
end
```

## Rails 7.0 supports with in_order_of invert_where and excluding

```yaml
rails_version: 7.0.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Post < ApplicationRecord
  def self.with_test = with(foo: "bar")
  def self.in_order_test = in_order_of(:status, ["a", "b"])
  def self.invert_test = invert_where
  def self.excluding_test = excluding(first)
  def self.with_recursive_test = with_recursive(foo: "bar")
  def self.regroup_test = regroup(:category)
end
```

### result

```rbs
class Post < ApplicationRecord
  def self.with_test: -> ActiveRecord::Relation[Post]
  def self.in_order_test: -> ActiveRecord::Relation[Post]
  def self.invert_test: -> ActiveRecord::Relation[Post]
  def self.excluding_test: -> ActiveRecord::Relation[Post]
  def self.with_recursive_test: -> untyped
  def self.regroup_test: -> untyped
end
```

## Rails 7.1 supports with_recursive and regroup

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Post < ApplicationRecord
  def self.with_recursive_test = with_recursive(foo: "bar")
  def self.regroup_test = regroup(:category)
end
```

### result

```rbs
class Post < ApplicationRecord
  def self.with_recursive_test: -> ActiveRecord::Relation[Post]
  def self.regroup_test: -> ActiveRecord::Relation[Post]
end
```

## Grouped Active Record count returns grouped counts

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Post < ApplicationRecord
  def self.grouped_counts = group(:status).count
  def self.grouped_count_keys = group(:status).count.keys
end
```

### result

```rbs
class Post < ApplicationRecord
  def self.grouped_counts: -> Hash[untyped, Integer]
  def self.grouped_count_keys: -> Array[untyped]
end
```

## Serialized attributes use their declared type

### update

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "users", force: :cascade do |t|
    t.string "tags"
    t.string "position"
  end
end
```

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Position
  def line = 1
end

module SerializedAttributes
  extend ActiveSupport::Concern

  included do
    serialize :position, type: Position
  end
end

class User < ApplicationRecord
  serialize :tags, type: Array
  include SerializedAttributes

  def self.read_tags = new.tags
  def self.read_position = new.position
end
```

### result

```rbs
class Position
  def line: -> 1
end

module SerializedAttributes
  extend ActiveSupport::Concern
end

class User < ApplicationRecord
  include SerializedAttributes

  def tags: -> Array
  def tags=: (Array tags) -> Array
  def position: -> Position
  def position=: (Position position) -> Position
  def self.read_tags: -> Array
  def self.read_position: -> Position
end
```

## Preserve explicit methods for serialized attributes

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class SourceSerializedAttribute < ApplicationRecord
  serialize :payload, type: Array

  def payload = :source_payload
end

module RbsSerializedAttribute
  extend ActiveSupport::Concern

  included do
    serialize :payload, type: Array
  end
end

class RbsSerializedAttributeRecord < ApplicationRecord
  include RbsSerializedAttribute

  #: () -> Symbol
  def payload = :rbs_payload

  #: (String payload) -> Symbol
  def payload=(payload) = :rbs_payload
end
```

### result

```rbs
module RbsSerializedAttribute
  extend ActiveSupport::Concern
end

class RbsSerializedAttributeRecord < ApplicationRecord
  include RbsSerializedAttribute

  def payload: -> Symbol
  def payload=: (String payload) -> Symbol
end

class SourceSerializedAttribute < ApplicationRecord
  def payload=: (Array payload) -> Array
  def payload: -> :source_payload
end
```

## Serialized attributes without a declared type stay unknown

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

module UnspecifiedSerializedAttribute
  extend ActiveSupport::Concern

  included do
    serialize :payload
  end
end

class User < ApplicationRecord
  include UnspecifiedSerializedAttribute

  def self.read_payload = new.payload
end
```

### result

```rbs
module UnspecifiedSerializedAttribute
  extend ActiveSupport::Concern
end

class User < ApplicationRecord
  include UnspecifiedSerializedAttribute

  def payload: -> untyped
  def payload=: (untyped payload) -> untyped
  def self.read_payload: -> untyped
end
```

## Fully qualified serialized types apply from a concern

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "draft_notes", force: :cascade do |t|
    t.text "position"
    t.text "original_position"
    t.text "change_position"
  end
end
```

### file: lib/gitlab/diff/position.rb

```ruby
module Gitlab
  module Diff
    class Position
      def complete? = true
      def diff_file = "file"
    end
  end
end
```

### file: app/models/concerns/diff_positionable_note.rb

```ruby
module DiffPositionableNote
  extend ActiveSupport::Concern

  included do
    serialize :original_position, type: Gitlab::Diff::Position
    serialize :position, type: Gitlab::Diff::Position
    serialize :change_position, type: Gitlab::Diff::Position
  end
end
```

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class DraftNote < ApplicationRecord
  include DiffPositionableNote

  def self.position_complete = new.position&.complete?
  def self.original_position_diff_file = new.original_position&.diff_file
  def self.change_position_complete = new.change_position&.complete?
end
```

### result

```rbs
class DraftNote < ApplicationRecord
  include DiffPositionableNote

  def self.position_complete: -> true
  def self.original_position_diff_file: -> "file"
  def self.change_position_complete: -> true
end
```
