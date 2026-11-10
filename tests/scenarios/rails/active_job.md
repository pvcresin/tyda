# Rails / Active Job

## Generate perform_now and perform_later on job classes

```yaml
known_issue: true
```

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
  def self.perform_later: (Integer user_id) -> (CleanupJob | false)
  def self.perform_now: (Integer user_id) -> (Integer | false)
  def perform: (Integer user_id) -> Integer
end
```

## No-arg jobs keep no-arg perform helpers

```yaml
known_issue: true
```

### update

```ruby
class PurgeJob < ApplicationJob
  def perform = "done"
end
```

### result

```rbs
class PurgeJob < ApplicationJob
  def self.perform_later: -> (PurgeJob | false)
  def self.perform_now: -> ("done" | false)
  def perform: -> "done"
end
```

## queue_as discard_on and retry_on do not affect job inference

```yaml
known_issue: true
```

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
  def self.perform_later: (String message) -> (NotifyJob | false)
  def self.perform_now: (String message) -> void
  def perform: (String message) -> void
end
```

## Resolve perform_later for a job defined in another file

```yaml
known_issue: true
```

### update

### file: app/jobs/sync_job.rb

```ruby
class SyncJob < ApplicationJob
  #: (Integer id) -> String
  def perform(id) = id.to_s
end
```

```ruby
class ItemsController < ActionController::Base
  def enqueue = SyncJob.perform_later(1)
end
```

### result

```rbs
class ItemsController < ActionController::Base
  def enqueue: -> (SyncJob | false)
end
```

## Resolve perform_now for a job defined in another file

```yaml
known_issue: true
```

### update

### file: app/jobs/sync_job.rb

```ruby
class SyncJob < ApplicationJob
  #: (Integer id) -> String
  def perform(id) = id.to_s
end
```

```ruby
class ItemsController < ActionController::Base
  def run_now = SyncJob.perform_now(1)
end
```

### result

```rbs
class ItemsController < ActionController::Base
  def run_now: -> (String | false)
end
```
