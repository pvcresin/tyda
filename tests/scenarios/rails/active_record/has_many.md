# Rails / Active Record / Has Many

## Generate has_many collection methods

### update

```ruby
class A
  has_many :items
end
```

### result

```rbs
class A
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[Item] items) -> ActiveRecord::Associations::CollectionProxy[Item]
end
```

## has_many class_name sets the class

### update

```ruby
class A
  has_many :items, class_name: "B"
end
```

### result

```rbs
class A
  def items: -> ActiveRecord::Associations::CollectionProxy[B]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[B] items) -> ActiveRecord::Associations::CollectionProxy[B]
end
```

## Inflect plural has_many names

### update

```ruby
class A
  has_many :items
end
```

### result

```rbs
class A
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[Item] items) -> ActiveRecord::Associations::CollectionProxy[Item]
end
```

## Resolve first and build from has_many proxy

### update

```ruby
class A < ApplicationRecord
  has_many :items

  def first_item = items.first

  def build_item = items.build
end
```

### result

```rbs
class A < ApplicationRecord
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[Item] items) -> ActiveRecord::Associations::CollectionProxy[Item]
  def first_item: -> Item?
  def build_item: -> Item
end
```

## Resolve has_many through and source targets

### update

```ruby
class A < ApplicationRecord
  has_many :items, through: :links, source: :item
end
```

### result

```rbs
class A < ApplicationRecord
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[Item] items) -> ActiveRecord::Associations::CollectionProxy[Item]
end
```

## Use has_many inverse_of for inverse target class

### update

```ruby
class A < ApplicationRecord
  belongs_to :item, class_name: "B", inverse_of: :items
end

class B < ApplicationRecord
  has_many :items, inverse_of: :item
end
```

### result

```rbs
class A < ApplicationRecord
  def item: -> B
  def item=: (B item) -> B
  def build_item: (?Hash[String | Symbol, untyped] attributes) -> B
  def create_item: (?Hash[String | Symbol, untyped] attributes) -> B
  def create_item!: (?Hash[String | Symbol, untyped] attributes) -> B
end

class B < ApplicationRecord
  def items: -> ActiveRecord::Associations::CollectionProxy[A]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[A] items) -> ActiveRecord::Associations::CollectionProxy[A]
end
```

## Apply custom inflections to has_many target inference

### update

`config/initializers/inflections.rb`

```ruby
ActiveSupport::Inflector.inflections(:en) do |inflect|
  inflect.irregular "person", "people"
end
```

```ruby
class ApplicationRecord; end

class Person < ApplicationRecord
end

class Team < ApplicationRecord
  has_many :people

  def first_person = people.first
end
```

### result

```rbs
class Team < ApplicationRecord
  def people: -> ActiveRecord::Associations::CollectionProxy[Person]
  def person_ids: -> Array[Integer]
  def person_ids=: (Array[Integer] person_ids) -> Array[Integer]
  def people=: (Array[Person] people) -> ActiveRecord::Associations::CollectionProxy[Person]
  def first_person: -> Person?
end
```

## Query a has_many proxy with finders and builders

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "posts", force: :cascade do |t|
    t.string "title", null: false
    t.bigint "account_id", null: false
  end
end
```

### file: app/models/account.rb

```ruby
class Account < ApplicationRecord
  has_many :posts
end
```

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
  belongs_to :account
end
```

```ruby
class AccountPosts
  def titles = Account.find(1).posts.where(id: 1).pluck(:title)
  def by_title = Account.find(1).posts.find_by(title: "a")
  def by_id = Account.find(1).posts.find(1)
  def created = Account.find(1).posts.create!(title: "a")
  def built = Account.find(1).posts.new
  def found_or_created = Account.find(1).posts.find_or_create_by(title: "a")
  def any = Account.find(1).posts.exists?
  def total = Account.find(1).posts.size
  def newest = Account.find(1).posts.order(:id).last
end
```

### result

```rbs
class AccountPosts
  def titles: -> Array[String]
  def by_title: -> Post?
  def by_id: -> Post
  def created: -> Post
  def built: -> Post
  def found_or_created: -> Post
  def any: -> bool
  def total: -> Integer
  def newest: -> Post?
end
```

## Scoped has_many with class_name uses the named class

### update

```ruby
class Account < ApplicationRecord
  has_many :drafts, -> { where(published_at: nil) }, class_name: "Post"
end
```

### result

```rbs
class Account < ApplicationRecord
  def drafts: -> ActiveRecord::Associations::CollectionProxy[Post]
  def draft_ids: -> Array[Integer]
  def draft_ids=: (Array[Integer] draft_ids) -> Array[Integer]
  def drafts=: (Array[Post] drafts) -> ActiveRecord::Associations::CollectionProxy[Post]
end
```

## Polymorphic has_many with as and class_name uses the named class

### update

```ruby
class Post < ApplicationRecord
  has_many :replies, as: :commentable, class_name: "Comment", dependent: :destroy
end
```

### result

```rbs
class Post < ApplicationRecord
  def replies: -> ActiveRecord::Associations::CollectionProxy[Comment]
  def reply_ids: -> Array[Integer]
  def reply_ids=: (Array[Integer] reply_ids) -> Array[Integer]
  def replies=: (Array[Comment] replies) -> ActiveRecord::Associations::CollectionProxy[Comment]
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
