# Known Issues / Rails Framework

## A controller action calls a concern method defined in another file

```yaml
rails_version: 7.1.0
```

### update

### file: app/controllers/concerns/paginated.rb

```ruby
module Paginated
  extend ActiveSupport::Concern

  def page_params = params.permit(:page)
end
```

```ruby
class PostsController < ActionController::Base
  include Paginated

  def index = page_params
end
```

### result

```rbs
class PostsController < ActionController::Base
  include Paginated

  def index: -> ActionController::Parameters
end
```

## deliver_now on a mailer defined in another file returns Mail::Message

```yaml
rails_version: 7.1.0
```

### update

### file: app/mailers/notice_mailer.rb

```ruby
class NoticeMailer < ApplicationMailer
  def ping(user) = mail
end
```

```ruby
class ItemsController < ActionController::Base
  def notice_now = NoticeMailer.ping(1).deliver_now
end
```

### result

```rbs
class ItemsController < ActionController::Base
  def notice_now: -> Mail::Message
end
```

## A parameterized mailer action returns a message delivery

```yaml
rails_version: 7.1.0
```

### update

```ruby
class InviteMailer < ApplicationMailer
  def invite = mail
end

class Inviter
  def delivery = InviteMailer.with(email: "a@example.com").invite
end
```

### result

```rbs
class InviteMailer < ApplicationMailer
  def invite: -> Mail::Message
  def self.invite: -> ActionMailer::MessageDelivery
end

class Inviter
  def delivery: -> ActionMailer::Parameterized::MessageDelivery
end
```

## Skip resource helpers excluded by only

```yaml
rails_version: 7.1.0
```

### update

`config/routes.rb`

```ruby
Rails.application.routes.draw do
  resources :posts, only: [:index, :show]
end
```

```ruby
class Links
  def edit_link(post) = edit_post_path(post)
end
```

### result

```rbs
class Links
  def edit_link: (untyped post) -> untyped
end
```

## Resolve shallow nested resource helpers

```yaml
rails_version: 7.1.0
```

### update

`config/routes.rb`

```ruby
Rails.application.routes.draw do
  resources :articles, shallow: true do
    resources :comments
  end
end
```

```ruby
class Links
  def comment(id) = comment_path(id)

  def edit_comment(id) = edit_comment_path(id)
end
```

### result

```rbs
class Links
  def comment: (untyped id) -> String
  def edit_comment: (untyped id) -> String
end
```

## Resolve helpers from routing concerns

```yaml
rails_version: 7.1.0
```

### update

`config/routes.rb`

```ruby
Rails.application.routes.draw do
  concern :commentable do
    resources :notes
  end
  resources :photos, concerns: :commentable
end
```

```ruby
class Links
  def notes(photo) = photo_notes_path(photo)
end
```

### result

```rbs
class Links
  def notes: (untyped photo) -> String
end
```

## Resolve resources helpers renamed with as

```yaml
rails_version: 7.1.0
```

### update

`config/routes.rb`

```ruby
Rails.application.routes.draw do
  resources :events, as: :happenings
end
```

```ruby
class Links
  def happenings = happenings_path

  def happening(event) = happening_path(event)
end
```

### result

```rbs
class Links
  def happenings: -> String
  def happening: (untyped event) -> String
end
```

## Resolve direct route helpers

```yaml
rails_version: 7.1.0
```

### update

`config/routes.rb`

```ruby
Rails.application.routes.draw do
  direct(:homepage) { "https://example.com" }
end
```

```ruby
class Links
  def home = homepage_url
end
```

### result

```rbs
class Links
  def home: -> String
end
```

## Resolve dom_id in helper modules

```yaml
rails_version: 7.1.0
```

### update

```ruby
module RowsHelper
  def row_id(record) = dom_id(record)
end
```

### result

```rbs
module RowsHelper
  def row_id: (untyped record) -> String
end
```
