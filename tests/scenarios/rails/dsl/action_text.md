# Rails / DSL / Action Text

## Generate rich text accessors

### update

```ruby
class Post
  has_rich_text :body
end
```

### result

```rbs
class Post
  def body: -> ActionText::RichText
  def body?: -> bool
  def body=: ((String | ActionText::Content | ActionText::RichText)? body) -> (String | ActionText::Content | ActionText::RichText)?
end
```

## Encrypted rich text reader returns EncryptedRichText

### update

```ruby
class Post
  has_rich_text :secret, encrypted: true
end
```

### result

```rbs
class Post
  def secret: -> ActionText::EncryptedRichText
  def secret?: -> bool
  def secret=: ((String | ActionText::Content | ActionText::RichText)? secret) -> (String | ActionText::Content | ActionText::RichText)?
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
