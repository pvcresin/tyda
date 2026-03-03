# Ruby / Class / Template Method

## Parent calls subclass method in same file

### update

```ruby
class Base
  def run = name
end

class Child < Base
  def name = "world"
end
```

### result

```rbs
class Base
  def run: -> "world"
end

class Child < Base
  def name: -> "world"
end
```

## Parent calls subclass method in another file

### update

```ruby
class Base
  def greeting = "hello"
end

class Child < Base
  def name = "world"
end
```

```ruby
class Base
  def run = name
end
```

### result

```rbs
class Base
  def greeting: -> "hello"
  def run: -> "world"
end
```

## Union return values from multiple subclasses

### update

```ruby
class Animal
  def speak = sound
end

class Dog < Animal
  def sound = "woof"
end

class Cat < Animal
  def sound = "meow"
end
```

### result

```rbs
class Animal
  def speak: -> "meow" | "woof"
end

class Cat < Animal
  def sound: -> "meow"
end

class Dog < Animal
  def sound: -> "woof"
end
```

## Chain calls on subclass return value

### update

```ruby
class Account
  def user = "admin"
end

class BaseAction
  def process = target_account.user
end

class ModerationAction < BaseAction
  def target_account = Account.new
end
```

### result

```rbs
class Account
  def user: -> "admin"
end

class BaseAction
  def process: -> "admin"
end

class ModerationAction < BaseAction
  def target_account: -> Account
end
```

## Method call references class from another file

### update

```ruby
class User
  def email = "user@example.com"
end
```

```ruby
class Controller
  def show
    user = User.new
    user.email
  end
end
```

### result

```rbs
class Controller
  def show: -> "user@example.com"
end
```

## Inheritance chain across files

### update

```ruby
class ConcreteAction < BaseAction
  def prepare = "done"
end
```

```ruby
class BaseAction
  def execute = prepare
end
```

### result

```rbs
class BaseAction
  def execute: -> "done"
end
```
