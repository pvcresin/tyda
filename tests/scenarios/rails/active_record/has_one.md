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
