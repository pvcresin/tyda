# Rails / DSL / Sidekiq Worker

## Generate Sidekiq perform class methods without Active Job helpers

```yaml
known_issue: true
```

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

  def self.perform_async: -> String?
  def self.perform_in: -> String?
  def self.perform_at: -> String?
  def self.perform_bulk: -> Array[String?]
  def self.perform_inline: -> true?
  def self.perform_sync: -> true?
end
```

## Resolve worker runtime accessors

```yaml
known_issue: true
```

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

  def self.perform_async: -> String?
  def self.perform_in: -> String?
  def self.perform_at: -> String?
  def self.perform_bulk: -> Array[String?]
  def self.perform_inline: -> true?
  def self.perform_sync: -> true?
  def perform: -> String?
end
```
