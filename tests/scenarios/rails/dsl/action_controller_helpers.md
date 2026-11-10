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

## Resolve tag builder methods in helper modules

### update

```ruby
module TagsHelper
  def scoped_tag(scope) = tag.samp(scope)

  def translated_tag = tag.span(t('users.label'))

  def direct_tag = tag('p', 'content')
end
```

### result

```rbs
module TagsHelper
  def scoped_tag: (untyped scope) -> String
  def translated_tag: -> String
  def direct_tag: -> String
end
```

## Keep deleted controller parameters unknown

### update

```ruby
class ParametersController < ActionController::Base
  def deleted_value = params.delete(:value)
end
```

### result

```rbs
class ParametersController < ActionController::Base
  def deleted_value: -> untyped
end
```

## Resolve text and number helpers in helper modules

### update

```ruby
module FormatHelper
  def short(text) = truncate(text, length: 10)

  def ago(time) = time_ago_in_words(time)

  def count = number_with_delimiter(1000)

  def posts_label = pluralize(2, "post")

  def joined = safe_join(["a", "b"])

  def box = content_tag(:div, "x")

  def logo = image_tag("logo.png")

  def body = simple_format("text")

  def clean = sanitize("<b>x</b>")
end
```

### result

```rbs
module FormatHelper
  def short: (untyped text) -> String
  def ago: (untyped time) -> String
  def count: -> String
  def posts_label: -> String
  def joined: -> String
  def box: -> String
  def logo: -> String
  def body: -> String
  def clean: -> String
end
```
