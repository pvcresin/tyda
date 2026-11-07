# Ruby / Class / Ivar Name No Class Guess

## A parameter named like a class stays unknown on attr_reader

```ruby
class Account
  def name = "test"
end

class Presenter
  attr_reader :account

  def initialize(account)
    @account = account
  end
end
```

### result

```rbs
class Account
  def name: -> "test"
end

class Presenter
  def account: -> untyped
  def initialize: (untyped account) -> void
end
```

## A parameter named like a class stays unknown on an ivar read

```ruby
class User
  attr_reader :name
  def initialize(name)
    @name = name
  end
end

class Mailer
  def initialize(user)
    @user = user
  end

  def recipient = @user
end
```

### result

```rbs
class Mailer
  def initialize: (untyped user) -> void
  def recipient: -> untyped
end

class User
  def name: -> untyped
  def initialize: (untyped name) -> void
end
```

## attr_reader with call site providing concrete type

```ruby
class Config
  def value = 42
end

class Service
  attr_reader :config

  def initialize(config)
    @config = config
  end
end

Service.new(Config.new)
```

### result

```rbs
class Config
  def value: -> 42
end

class Service
  def config: -> Config
  def initialize: (Config config) -> void
end
```
