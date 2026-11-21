# Rails / DSL / CanCanCan

## Authorization predicates return bool in controllers and helpers

### update

```ruby
class PostsController < ActionController::Base
  def allowed = can?(:read, :post)

  def denied = cannot?(:read, :post)
end

module PostsHelper
  def editable = can?(:update, :post)
end
```

### result

```rbs
class PostsController < ActionController::Base
  def allowed: -> bool
  def denied: -> bool
end

module PostsHelper
  def editable: -> bool
end
```
