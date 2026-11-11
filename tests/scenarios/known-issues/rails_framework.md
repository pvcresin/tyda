# Known Issues / Rails Framework

## permit! and permitted? follow the strong parameters contract

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ItemsController < ActionController::Base
  def all_params = params.permit!

  def allowed = params.permitted?
end
```

### result

```rbs
class ItemsController < ActionController::Base
  def all_params: -> ActionController::Parameters
  def allowed: -> bool
end
```

## Resolve request port protocol and subdomain

```yaml
rails_version: 7.1.0
```

### update

```ruby
class PagesController < ActionController::Base
  def port = request.port

  def protocol = request.protocol

  def subdomain = request.subdomain
end
```

### result

```rbs
class PagesController < ActionController::Base
  def port: -> Integer
  def protocol: -> String
  def subdomain: -> String
end
```

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

## MessageDelivery#message returns Mail::Message

```yaml
rails_version: 7.1.0
```

### update

```ruby
class NoteMailer < ApplicationMailer
  def ping = mail
end

class Notifier
  def built = NoteMailer.ping.message
end
```

### result

```rbs
class NoteMailer < ApplicationMailer
  def ping: -> Mail::Message
  def self.ping: -> ActionMailer::MessageDelivery
end

class Notifier
  def built: -> Mail::Message
end
```

## Default environment predicates return bool without environment files

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Probe
  def production = Rails.env.production?

  def local = Rails.env.local?
end
```

### result

```rbs
class Probe
  def production: -> bool
  def local: -> bool
end
```

## Date.yesterday and Date.tomorrow return Date

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Calendar
  def yesterday = Date.yesterday

  def tomorrow = Date.tomorrow
end
```

### result

```rbs
class Calendar
  def yesterday: -> Date
  def tomorrow: -> Date
end
```

## Duration arithmetic with another duration returns a duration

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Timer
  def total = 1.day + 2.hours

  def remaining = 2.weeks - 1.day
end
```

### result

```rbs
class Timer
  def total: -> ActiveSupport::Duration
  def remaining: -> ActiveSupport::Duration
end
```

## Adding a duration to TimeWithZone returns TimeWithZone

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Clock
  def later = Time.current + 1.hour

  def zone_later = Time.zone.now + 1.hour
end
```

### result

```rbs
class Clock
  def later: -> ActiveSupport::TimeWithZone
  def zone_later: -> ActiveSupport::TimeWithZone
end
```

## TimeZone at and local return non-nil TimeWithZone

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Clock
  def epoch = Time.zone.at(0)

  def new_year = Time.zone.local(2024, 1, 1)
end
```

### result

```rbs
class Clock
  def epoch: -> ActiveSupport::TimeWithZone
  def new_year: -> ActiveSupport::TimeWithZone
end
```

## Array access extensions return the element or nil

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Picker
  # @rbs items: Array[Integer]
  def second(items) = items.second

  # @rbs items: Array[Integer]
  def third(items) = items.third
end
```

### result

```rbs
class Picker
  def second: (Array[Integer] items) -> Integer?
  def third: (Array[Integer] items) -> Integer?
end
```

## Array excluding including and in_groups keep the element type

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Picker
  # @rbs items: Array[Integer]
  def without_one(items) = items.excluding(1)

  # @rbs items: Array[Integer]
  def with_three(items) = items.including(3)

  # @rbs items: Array[Integer]
  def groups(items) = items.in_groups(2)
end
```

### result

```rbs
class Picker
  def without_one: (Array[Integer] items) -> Array[Integer]
  def with_three: (Array[Integer] items) -> Array[Integer]
  def groups: (Array[Integer] items) -> Array[Array[Integer?]]
end
```

## symbolize_keys and stringify_keys change the key type

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Keys
  # @rbs hash: Hash[String, Integer]
  def symbolized(hash) = hash.symbolize_keys

  # @rbs hash: Hash[Symbol, Integer]
  def stringified(hash) = hash.stringify_keys
end
```

### result

```rbs
class Keys
  def symbolized: (Hash[String, Integer] hash) -> Hash[Symbol, Integer]
  def stringified: (Hash[Symbol, Integer] hash) -> Hash[String, Integer]
end
```

## reverse_merge keeps keys from both record hashes

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Options
  def merged = { a: 1 }.reverse_merge(b: 2)
end
```

### result

```rbs
class Options
  def merged: -> { b: 2, a: 1 }
end
```

## with_indifferent_access returns HashWithIndifferentAccess

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Options
  # @rbs hash: Hash[Symbol, Integer]
  def indifferent(hash) = hash.with_indifferent_access
end
```

### result

```rbs
class Options
  def indifferent: (Hash[Symbol, Integer] hash) -> ActiveSupport::HashWithIndifferentAccess
end
```

## Resolve multiple_of? to_query and secure_compare

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Checks
  def even = 4.multiple_of?(2)

  def query = { page: 1 }.to_query

  def same = ActiveSupport::SecurityUtils.secure_compare("a", "b")
end
```

### result

```rbs
class Checks
  def even: -> bool
  def query: -> String
  def same: -> bool
end
```
