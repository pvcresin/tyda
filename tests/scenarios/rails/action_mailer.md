# Rails / Action Mailer

## Infer mailer actions through class helpers

### update

```ruby
class UserMailer < ApplicationMailer
  def welcome = mail
end
```

### result

```rbs
class UserMailer < ApplicationMailer
  def welcome: -> Mail::Message
  def self.welcome: -> ActionMailer::MessageDelivery
end
```

## Class helpers keep instance action params

### update

```ruby
class User
end

class UserMailer < ApplicationMailer
  #: (User user) -> Mail::Message
  def welcome(user) = mail
end
```

### result

```rbs
class UserMailer < ApplicationMailer
  def welcome: (User user) -> Mail::Message
  def self.welcome: (User user) -> ActionMailer::MessageDelivery
end
```

## deliver_later! returns ActionMailer::MessageDelivery

### update

```ruby
class NotifyMailer < ApplicationMailer
  def alert(user) = mail
end

class NotifyService
  def notify(user) = NotifyMailer.alert(user).deliver_later!
end
```

### result

```rbs
class NotifyMailer < ApplicationMailer
  def alert: (untyped user) -> Mail::Message
  def self.alert: (untyped user) -> ActionMailer::MessageDelivery
end

class NotifyService
  def notify: (untyped user) -> ActionMailer::MessageDelivery
end
```

## Resolve mailers from namespace scope

### update

```ruby
class AlertMailer < ApplicationMailer
  def notify(user) = mail
end

module Admin
  class Notifier
    def send_alert(user) = AlertMailer.notify(user).deliver_later!
  end
end
```

### result

```rbs
class Admin::Notifier
  def send_alert: (untyped user) -> ActionMailer::MessageDelivery
end

class AlertMailer < ApplicationMailer
  def notify: (untyped user) -> Mail::Message
  def self.notify: (untyped user) -> ActionMailer::MessageDelivery
end
```

## deliver_now returns Mail::Message

### update

```ruby
class NoticeMailer < ApplicationMailer
  def ping(user) = mail
end

class Notifier
  def send_sync(user) = NoticeMailer.ping(user).deliver_now
end
```

### result

```rbs
class NoticeMailer < ApplicationMailer
  def ping: (untyped user) -> Mail::Message
  def self.ping: (untyped user) -> ActionMailer::MessageDelivery
end

class Notifier
  def send_sync: (untyped user) -> Mail::Message
end
```

## default DSL does not affect mailer inference

### update

```ruby
class WelcomeMailer < ApplicationMailer
  default from: "no-reply@example.com", reply_to: "support@example.com"

  def greet(user) = mail
end
```

### result

```rbs
class WelcomeMailer < ApplicationMailer
  def greet: (untyped user) -> Mail::Message
  def self.greet: (untyped user) -> ActionMailer::MessageDelivery
end
```

## Show mailer first-step method as synthetic

```yaml
include_synthetic_dsl_methods: true
```

### update

```ruby
class UserMailer < ApplicationMailer
  def welcome = mail
end
```

### result

```rbs
class UserMailer < ApplicationMailer
  def mail: -> Mail::Message
  def welcome: -> Mail::Message
  def self.welcome: -> ActionMailer::MessageDelivery
end
```
