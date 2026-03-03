# Ruby / Method / Forwardable

## def_delegators

### update

```ruby
class A
  extend Forwardable

  def_delegators :@io, :puts, :print
end
```

### result

```rbs
class A
  extend Forwardable

  def puts: -> untyped
  def print: -> untyped
end
```

## def_delegator

### update

```ruby
class A
  extend Forwardable

  def_delegator :@output, :write
end
```

### result

```rbs
class A
  extend Forwardable

  def write: -> untyped
end
```

## Follow delegated helper method statically

### update

```ruby
class Backend
  def status = true
end

class A
  extend Forwardable

  def backend = Backend.new
  def_delegator :backend, :status
end
```

### result

```rbs
class A
  extend Forwardable

  def backend: -> Backend
  def status: -> true
end

class Backend
  def status: -> true
end
```

## def_delegator with alias

### update

```ruby
class A
  extend Forwardable

  def_delegator :@backend, :execute, :run
end
```

### result

```rbs
class A
  extend Forwardable

  def run: -> untyped
end
```

## def_delegators inside class << self becomes singleton method

### update

```ruby
module Edition
  def self.root = "root"
end

module Gitlab
  class << self
    extend Forwardable
    def_delegators :Edition, :root
  end
end
```

### result

```rbs
module Edition
  def self.root: -> "root"
end

module Gitlab
  extend Forwardable

  def self.root: -> "root"
end
```

## def_instance_delegators

### update

```ruby
class A
  extend Forwardable

  def_instance_delegators :@io, :puts, :print
end
```

### result

```rbs
class A
  extend Forwardable

  def puts: -> untyped
  def print: -> untyped
end
```

## def_instance_delegator with alias

### update

```ruby
class A
  extend Forwardable

  def_instance_delegator :@backend, :execute, :run
end
```

### result

```rbs
class A
  extend Forwardable

  def run: -> untyped
end
```

## Combine multiple def_delegator and def_delegators calls

### update

```ruby
class A
  extend Forwardable

  def_delegators :@client, :get, :post
  def_delegator :@logger, :info, :log

  def process = "done"
end
```

### result

```rbs
class A
  extend Forwardable

  def get: -> untyped
  def post: -> untyped
  def log: -> untyped
  def process: -> "done"
end
```
