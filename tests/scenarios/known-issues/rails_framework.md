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
