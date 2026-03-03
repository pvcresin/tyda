# Rails / Helper Method

## Register helper_method without method definition

### update

```ruby
class ApplicationController
  helper_method :current_user
end
```

### result

```rbs
class ApplicationController
  def current_user: -> untyped
end
```

## Register multiple helper_method names

### update

```ruby
class BaseController
  helper_method :logged_in?, :admin?
end
```

### result

```rbs
class BaseController
  def logged_in?: -> untyped
  def admin?: -> untyped
end
```
