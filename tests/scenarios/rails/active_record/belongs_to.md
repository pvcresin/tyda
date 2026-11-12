# Rails / Active Record / Belongs To

## Generate belongs_to association methods

### update

```ruby
class Comment
  belongs_to :post
end
```

### result

```rbs
class Comment
  def post: -> Post
  def post=: (Post post) -> Post
  def build_post: (?Hash[String | Symbol, untyped] attributes) -> Post
  def create_post: (?Hash[String | Symbol, untyped] attributes) -> Post
  def create_post!: (?Hash[String | Symbol, untyped] attributes) -> Post
end
```

## belongs_to optional true allows nil

### update

```ruby
class Order
  belongs_to :coupon, optional: true
end
```

### result

```rbs
class Order
  def coupon: -> Coupon?
  def coupon=: (Coupon coupon) -> Coupon
  def build_coupon: (?Hash[String | Symbol, untyped] attributes) -> Coupon
  def create_coupon: (?Hash[String | Symbol, untyped] attributes) -> Coupon
  def create_coupon!: (?Hash[String | Symbol, untyped] attributes) -> Coupon
end
```

## belongs_to class_name sets the class

### update

```ruby
class Comment
  belongs_to :author, class_name: "User"
end
```

### result

```rbs
class Comment
  def author: -> User
  def author=: (User author) -> User
  def build_author: (?Hash[String | Symbol, untyped] attributes) -> User
  def create_author: (?Hash[String | Symbol, untyped] attributes) -> User
  def create_author!: (?Hash[String | Symbol, untyped] attributes) -> User
end
```

## belongs_to polymorphic true returns untyped

### update

```ruby
class Comment
  belongs_to :commentable, polymorphic: true
end
```

### result

```rbs
class Comment
  def commentable: -> untyped
  def commentable=: (untyped commentable) -> untyped
  def commentable_type: -> String
  def commentable_id: -> Integer
end
```

## belongs_to setter assignment returns target type

### update

```ruby
class ApplicationRecord; end

class B
  class << self
    def foo(x)
      [B.new]
    end
  end
end

class A < ApplicationRecord
  belongs_to :b, optional: true

  def foo=(x)
    self.b = B.foo(x).first
  end
end
```

### result

```rbs
class A < ApplicationRecord
  def b: -> B?
  def b=: (B? b) -> B
  def build_b: (?Hash[String | Symbol, untyped] attributes) -> B
  def create_b: (?Hash[String | Symbol, untyped] attributes) -> B
  def create_b!: (?Hash[String | Symbol, untyped] attributes) -> B
  def foo=: (untyped x) -> B
end

class B
  def self.foo: (untyped x) -> [B]
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
