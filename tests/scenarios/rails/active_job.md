# Rails / Active Job

## Generate perform_now and perform_later on job classes

### update

```ruby
class CleanupJob < ApplicationJob
  #: (Integer user_id) -> Integer
  def perform(user_id) = user_id
end
```

### result

```rbs
class CleanupJob < ApplicationJob
  def self.perform_later: (Integer user_id) -> CleanupJob
  def self.perform_now: (Integer user_id) -> Integer
  def perform: (Integer user_id) -> Integer
end
```

## No-arg jobs keep no-arg perform helpers

### update

```ruby
class PurgeJob < ApplicationJob
  def perform = "done"
end
```

### result

```rbs
class PurgeJob < ApplicationJob
  def self.perform_later: -> PurgeJob
  def self.perform_now: -> "done"
  def perform: -> "done"
end
```

## queue_as discard_on and retry_on do not affect job inference

### update

```ruby
class NotifyJob < ApplicationJob
  queue_as :high_priority
  discard_on ActiveJob::DeserializationError
  retry_on Net::OpenTimeout, wait: 5, attempts: 3

  #: (String message) -> void
  def perform(message) = nil
end
```

### result

```rbs
class NotifyJob < ApplicationJob
  def self.perform_later: (String message) -> NotifyJob
  def self.perform_now: (String message) -> void
  def perform: (String message) -> void
end
```
