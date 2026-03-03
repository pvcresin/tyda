# Ruby / Class / Setter Type Sync

## attr_accessor setter param follows ivar type

```ruby
class Config
  def value = 42
end

class App
  attr_accessor :config

  def initialize(config)
    @config = config
  end
end

App.new(Config.new)
```

### result

```rbs
class App
  def config: -> Config
  def config=: (Config config) -> Config
  def initialize: (Config config) -> void
end

class Config
  def value: -> 42
end
```

## attr_writer setter param follows CamelCase inference

```ruby
class Account
  def name = "x"
end

class Service
  attr_writer :account

  def initialize(account)
    @account = account
  end
end
```

### result

```rbs
class Account
  def name: -> "x"
end

class Service
  def account=: (Account account) -> Account
  def initialize: (Account account) -> void
end
```

## Setter call expression returns assigned value

```ruby
class A
  def value=(x)
    :from_method
  end

  def assign
    self.value = 1
  end
end
```

### result

```rbs
class A
  def value=: (Integer x) -> :from_method
  def assign: -> 1
end
```
