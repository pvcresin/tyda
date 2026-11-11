# Rails / Active Record / Has One

## Generate has_one association methods

### update

```ruby
class A
  has_one :item
end
```

### result

```rbs
class A
  def item: -> Item?
  def item=: (Item item) -> Item
  def build_item: (?Hash[String | Symbol, untyped] attributes) -> Item
  def create_item: (?Hash[String | Symbol, untyped] attributes) -> Item
  def create_item!: (?Hash[String | Symbol, untyped] attributes) -> Item
end
```

## has_one class_name sets the class

### update

```ruby
class A
  has_one :item, class_name: "B"
end
```

### result

```rbs
class A
  def item: -> B?
  def item=: (B item) -> B
  def build_item: (?Hash[String | Symbol, untyped] attributes) -> B
  def create_item: (?Hash[String | Symbol, untyped] attributes) -> B
  def create_item!: (?Hash[String | Symbol, untyped] attributes) -> B
end
```

## has_one through resolves the source target

### update

### file: app/models/account.rb

```ruby
class Account < ApplicationRecord
  has_one :profile
end
```

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
  belongs_to :account
  has_one :profile, through: :account
end
```

```ruby
class PostProfile
  def profile_of = Post.find(1).profile
end
```

### result

```rbs
class PostProfile
  def profile_of: -> Profile?
end
```

## A namespaced model reads the table Rails computes for it

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "organizations", force: :cascade do |t|
    t.string "name"
  end
  create_table "organization_details", force: :cascade do |t|
    t.bigint "organization_id"
    t.text "description", null: false
  end
end
```

### file: app/models/organizations/organization_detail.rb

```ruby
module Organizations
  class OrganizationDetail < ApplicationRecord
    belongs_to :organization
  end
end
```

### file: app/models/organizations/organization.rb

```ruby
module Organizations
  class Organization < ApplicationRecord
    has_one :organization_detail
  end
end
```

```ruby
class OrganizationSummary
  def first_description = Organizations::Organization.first!.organization_detail&.description
end
```

### result

```rbs
class OrganizationSummary
  def first_description: -> String?
end
```
