# Ruby / Class / Nested

## Nested class

### update

```ruby
class Outer
  class Inner
    def foo = 42
  end
end
```

### result

```rbs
class Outer::Inner
  def foo: -> 42
end
```

## Nested module

### update

```ruby
module Helpers
  module Inner
    def foo = 42
  end
end
```

### result

```rbs
module Helpers::Inner
  def foo: -> 42
end
```

## Nested class and method inside class

### update

```ruby
class App
  def run = "running"

  class Config
    def debug? = true
  end
end
```

### result

```rbs
class App
  def run: -> "running"
end

class App::Config
  def debug?: -> true
end
```

## Use short class name inside same namespace

### update

```ruby
module Admin
  class Role
    def name = "admin"
  end

  class Action
    def role = Role.new
  end
end
```

### result

```rbs
class Admin::Action
  def role: -> Admin::Role
end

class Admin::Role
  def name: -> "admin"
end
```

## Reference outer class from namespace

### update

```ruby
class Logger
  def log(msg) = msg
end

module Services
  class Worker
    def logger = Logger.new
  end
end
```

### result

```rbs
class Logger
  def log: (untyped msg) -> untyped
end

class Services::Worker
  def logger: -> Logger
end
```

## Resolve constants from parent scope in nested namespace

### update

```ruby
module Platform
  class Config
    def name = "platform"
  end

  module Api
    class Client
      def config = Config.new
    end
  end
end
```

### result

```rbs
class Platform::Api::Client
  def config: -> Platform::Config
end

class Platform::Config
  def name: -> "platform"
end
```

## Compact nested class path under compact outer class

### update

```ruby
class Foo::Bar
  class Baz::Qux
    def self.v = 1
  end
end

def f = Foo::Bar::Baz::Qux.v
```

### result

```rbs
class Foo::Bar::Baz::Qux
  def self.v: -> 1
end

class Object < BasicObject
  def f: -> 1
end
```
