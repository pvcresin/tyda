# Rails / DSL / Active Storage

## Generate attachment accessors

### update

```ruby
class Post
  has_one_attached :avatar
  has_many_attached :photos
end
```

### result

```rbs
class Post
  def avatar: -> ActiveStorage::Attached::One
  def avatar=: (ActiveStorage::Attached::One avatar) -> ActiveStorage::Attached::One
  def photos: -> ActiveStorage::Attached::Many
  def photos=: (ActiveStorage::Attached::Many photos) -> ActiveStorage::Attached::Many
end
```
