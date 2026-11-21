# Known issues / Action Controller

## Translation helpers in controllers return untyped

```yaml
rails_version: "8.1"
```

### update

```ruby
class PostsController < ActionController::Base
  def title = t('posts.title')

  def label = translate('posts.label')
end
```

### result

```rbs
class PostsController < ActionController::Base
  def title: -> untyped
  def label: -> untyped
end
```

## Authorization predicates need cancancan in the bundle

```yaml
rails_version: "8.1"
```

### update

```ruby
class PostsController < ActionController::Base
  def allowed = can?(:read, :post)

  def denied = cannot?(:read, :post)
end
```

### result

```rbs
class PostsController < ActionController::Base
  def allowed: -> untyped
  def denied: -> untyped
end
```

## View helpers that return nil for nil input stay nilable

```yaml
rails_version: "8.1"
```

### update

```ruby
module PostsHelper
  def body(html) = sanitize(html)

  def views(count) = number_to_human(count)

  def size(bytes) = number_to_human_size(bytes)

  def total(count) = number_with_delimiter(count)
end
```

### result

```rbs
module PostsHelper
  def body: (untyped html) -> ActiveSupport::SafeBuffer?
  def views: (untyped count) -> String?
  def size: (untyped bytes) -> String?
  def total: (untyped count) -> String?
end
```

## Response charset is always a String

```yaml
rails_version: "8.1"
```

### update

```ruby
class PostsController < ActionController::Base
  def encoding = response.charset
end
```

### result

```rbs
class PostsController < ActionController::Base
  def encoding: -> String
end
```
