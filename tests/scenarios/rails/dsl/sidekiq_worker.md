# Rails / DSL / Sidekiq Worker

## Generate perform class methods

### update

```ruby
class Job
  include Sidekiq::Worker
end
```

### result

```rbs
class Job
  include Sidekiq::Worker

  def self.perform_later: -> Job
  def self.perform_now: -> untyped
  def self.perform_async: -> String
  def self.perform_in: -> String
  def self.perform_at: -> String
  def self.perform_bulk: -> Array[String]
end
```

## Resolve worker runtime accessors

### update

```ruby
class CleanupWorker
  include Sidekiq::Worker

  def perform
    logger.info('start')
    jid
  end
end
```

### result

```rbs
class CleanupWorker
  include Sidekiq::Worker

  def self.perform_async: -> String
  def self.perform_in: -> String
  def self.perform_at: -> String
  def self.perform_bulk: -> Array[String]
  def perform: -> String?
end
```
