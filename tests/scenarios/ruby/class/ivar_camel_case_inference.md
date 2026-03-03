# Ruby / Class / Ivar Camel Case Inference

## attr_reader with param name matching a class

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
  def account: -> Account
  def initialize: (Account account) -> void
end
```

## ivar return with CamelCase resolution

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
  def initialize: (User user) -> void
  def recipient: -> User
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
