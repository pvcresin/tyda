# Rails / Active Support / Rails Singleton

## Rails.logger resolves declared BroadcastLogger

### update

```rbi
module ActiveSupport::LoggerSilence
  def silence(&block); end
end

class ActiveSupport::BroadcastLogger
  include ::ActiveSupport::LoggerSilence

  def info(*_arg0, **_arg1, &_arg2); end
end
```

```ruby
class Probe
  def logger = Rails.logger

  def silenced = Rails.logger.silence { 1 }
end
```

### result

```rbs
class Probe
  def logger: -> ActiveSupport::BroadcastLogger
  def silenced: -> untyped
end
```

## Resolve Rails application helpers

### update

```ruby
class Probe
  def app = Rails.application

  def config = Rails.configuration

  def cache = Rails.cache

  def logger = Rails.logger

  def root = Rails.root

  def custom_config = Rails.configuration.x

  def cache_value = Rails.cache.fetch("key").to_s

  def delete_cache = Rails.cache.delete("key")
end
```

### result

```rbs
class Probe
  def app: -> Rails::Application
  def config: -> Rails::Application::Configuration
  def cache: -> ActiveSupport::Cache::Store
  def logger: -> Logger
  def root: -> Pathname
  def custom_config: -> untyped
  def cache_value: -> untyped
  def delete_cache: -> bool
end
```

## Resolve environment predicates from environment files

### update

### file: config/environments/staging.rb

```ruby
Rails.application.configure do
end
```

```ruby
class Probe
  def staging = Rails.env.staging?
end
```

### result

```rbs
class Probe
  def staging: -> bool
end
```

## Resolve Rails.cache exist?

### update

```ruby
class Probe
  def cached = Rails.cache.exist?("key")
end
```

### result

```rbs
class Probe
  def cached: -> bool
end
```

## Rails.cache write returns nil when the cache store fails

### update

```ruby
class Probe
  def store = Rails.cache.write("key", 1)
end
```

### result

```rbs
class Probe
  def store: -> bool?
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
