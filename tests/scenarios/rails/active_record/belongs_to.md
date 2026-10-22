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
  def post: -> Post?
  def post=: (Post? post) -> Post?
  def build_post: -> Post
  def create_post: -> Post
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
  def coupon=: (Coupon? coupon) -> Coupon?
  def build_coupon: -> Coupon
  def create_coupon: -> Coupon
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
  def author: -> User?
  def author=: (User? author) -> User?
  def build_author: -> User
  def create_author: -> User
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
  def b=: (B? b) -> B?
  def build_b: -> B
  def create_b: -> B
  def foo=: (untyped x) -> B
end

class B
  def self.foo: (untyped x) -> [B]
end
```

## A NilClass case can clear a belongs_to association

### update

```ruby
class Post; end

class Comment
  belongs_to :post

  def clear_post(value)
    case value
    when NilClass
      self.post = nil
    end
  end

  def assign_post
    self.post = Post.new
  end

  def clear_post_via_send
    public_send(:post=, nil)
  end
end

class OptionalComment
  belongs_to :post, optional: true

  def clear_post
    self.post = nil
  end
end

class NilThenPostComment
  belongs_to :post

  def set_post_in_order
    self.post = nil
    self.post = Post.new
    nil
  end
end

class PostThenNilComment
  belongs_to :post

  def set_post_in_order
    self.post = Post.new
    self.post = nil
    nil
  end
end
```

### result

```rbs
class Comment
  def post: -> Post?
  def post=: (Post? post) -> Post?
  def build_post: -> Post
  def create_post: -> Post
  def clear_post: (untyped value) -> nil
  def assign_post: -> Post
  def clear_post_via_send: -> Post?
end

class NilThenPostComment
  def post: -> Post?
  def post=: (Post? post) -> Post?
  def build_post: -> Post
  def create_post: -> Post
  def set_post_in_order: -> nil
end

class OptionalComment
  def post: -> Post?
  def post=: (Post? post) -> Post?
  def build_post: -> Post
  def create_post: -> Post
  def clear_post: -> nil
end

class PostThenNilComment
  def post: -> Post?
  def post=: (Post? post) -> Post?
  def build_post: -> Post
  def create_post: -> Post
  def set_post_in_order: -> nil
end

```

## A required association reader can return nil after assignment

```ruby
class Post; end

class RequiredAssociationReaderAfterNilWrite
  belongs_to :post

  def clear_then_read
    self.post = nil
    self.post
  end
end
```

### result

```rbs
class RequiredAssociationReaderAfterNilWrite
  def post: -> Post?
  def post=: (Post? post) -> Post?
  def build_post: -> Post
  def create_post: -> Post
  def clear_then_read: -> Post?
end
```

## RBS writer parameter remains authoritative

### update

```rbs
class RbsComment
  def post: -> Post
  def post=: (Post post) -> Post?
end
```

```ruby
class Post; end

class RbsComment
  belongs_to :post

  def clear_post
    self.post = nil
  end
end
```

### result

```rbs
class RbsComment
  def post: -> Post
  def post=: (Post post) -> Post?
  def build_post: -> Post
  def create_post: -> Post
  def clear_post: -> nil
end
```
