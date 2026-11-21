# Known issues / Action Text

## Rich text reader is never nil and has a predicate

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end

class Post < ActiveRecord::Base
  has_rich_text :body
end
```

### result

```rbs
class Post < ActiveRecord::Base
  def body: -> ActionText::RichText
  def body?: -> bool
  def body=: ((ActionText::Content | ActionText::RichText | String)? body) -> (ActionText::Content | ActionText::RichText | String)?
end
```

## has_many_rich_texts is not an Action Text method

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end

class Post < ActiveRecord::Base
  has_many_rich_texts :notes
end
```

### result

```rbs
```
