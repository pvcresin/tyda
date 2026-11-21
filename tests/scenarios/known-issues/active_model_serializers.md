# Known issues / Active Model Serializers

## Serializer associations do not define readers

```yaml
rails_version: "8.1"
```

### update

```ruby
class ActiveModel::Serializer
end

class PostSerializer < ActiveModel::Serializer
  belongs_to :author
  has_one :cover
  has_many :comments

  def kind = :post
end
```

### result

```rbs
class PostSerializer < ActiveModel::Serializer
  def kind: -> :post
end
```
