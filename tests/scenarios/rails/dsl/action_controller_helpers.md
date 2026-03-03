# Rails / DSL / Action Controller Helpers

## Resolve controller built-ins through unknown gem ancestors

### update

```ruby
class Settings::SessionsController < Devise::SessionsController
  def show = t('hello')

  def info = request

  def messages = flash

  def link = url_for(controller: "users", action: "show")
end
```

### result

```rbs
class Settings::SessionsController < Devise::SessionsController
  def show: -> String
  def info: -> ActionDispatch::Request
  def messages: -> ActionDispatch::Flash::FlashHash
  def link: -> String
end
```

## Resolve doorkeeper filters on oauth controllers

### update

```ruby
class OAuth::TokensController < Doorkeeper::TokensController
  def check = valid_doorkeeper_token?
end
```

### result

```rbs
class OAuth::TokensController < Doorkeeper::TokensController
  def check: -> bool
end
```

## Resolve translation helpers in view helper modules

### update

```ruby
module UsersHelper
  def user_label = t('users.label')
end
```

### result

```rbs
module UsersHelper
  def user_label: -> String
end
```

## Resolve view and route helpers in helper modules

### update

```ruby
module GroupsHelper
  def group_link(group) = link_to(group)

  def docs_url = help_page_path('user/groups')

  def allowed? = can?(:read_group)
end
```

### result

```rbs
module GroupsHelper
  def group_link: (untyped group) -> String
  def docs_url: -> String
  def allowed?: -> bool
end
```
