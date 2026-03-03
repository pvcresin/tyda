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
  def body: -> ActionText::RichText?
  def body=: (ActionText::RichText? body) -> ActionText::RichText?
end
```
