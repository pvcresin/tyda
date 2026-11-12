# Known Issues / Rails Models

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
