# Rails / Active Record / Mattr Accessor

## Generate mattr_accessor class readers and writers

### update

```ruby
class AppConfig
  mattr_accessor :site_name
end
```

### result

```rbs
class AppConfig
  def self.site_name: -> untyped
  def self.site_name=: (untyped site_name) -> untyped
end
```

## Generate only mattr_reader readers

### update

```ruby
class Settings
  mattr_reader :version
end
```

### result

```rbs
class Settings
  def self.version: -> untyped
end
```

## Generate only mattr_writer writers

### update

```ruby
class Logger
  mattr_writer :log_level
end
```

### result

```rbs
class Logger
  def self.log_level=: (untyped log_level) -> untyped
end
```

## cattr_accessor works like mattr_accessor

### update

```ruby
class Cache
  cattr_accessor :store
end
```

### result

```rbs
class Cache
  def self.store: -> untyped
  def self.store=: (untyped store) -> untyped
end
```

## cattr_reader / cattr_writer

### update

```ruby
class Db
  cattr_reader :pool_size
  cattr_writer :timeout
end
```

### result

```rbs
class Db
  def self.pool_size: -> untyped
  def self.timeout=: (untyped timeout) -> untyped
end
```
