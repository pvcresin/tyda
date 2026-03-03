# Ruby / Class / Mixin Sibling Method

```ruby
class Request
  module Env
    def get_header(name) = @env[name]
  end
  module Helpers
    def path_info = get_header("PATH_INFO")
  end
  include Env
  include Helpers
end
```

## Resolve across two sibling mixins in same class

### update

```ruby
class Container
  module Reader
    def read_value = 42
  end
  module User
    def use_value = read_value
  end
  include Reader
  include User
end
```

### result

```rbs
class Container
  include Reader
  include User
end

module Container::Reader
  def read_value: -> 42
end

module Container::User
  def use_value: -> 42
end
```

## Sibling mixins with definition and call

### update

```ruby
class Request
  module Env
    def get_header(name)
      @env[name]
    end
  end
  module Helpers
    def path_info = get_header("PATH_INFO")
    def request_method = get_header("REQUEST_METHOD")
  end
  include Env
  include Helpers
end
```

### result

```rbs
class Request
  include Env
  include Helpers
end

module Request::Env
  def get_header: (untyped name) -> untyped
end

module Request::Helpers
  def path_info: -> untyped
  def request_method: -> untyped
end
```

## Resolve mixin short name to FQN in enclosing scope

### update

```ruby
class Request
  module Inner
    def hello = "hi"
  end
  include Inner

  def greet = hello
end
```

### result

```rbs
class Request
  include Inner

  def greet: -> "hi"
end

module Request::Inner
  def hello: -> "hi"
end
```

## Scoped resolution works in two-level nesting

### update

```ruby
module Outer
  class Request
    module Env
      def get_header = "x"
    end
    module Helpers
      def header = get_header
    end
    include Env
    include Helpers
  end
end
```

### result

```rbs
class Outer::Request
  include Env
  include Helpers
end

module Outer::Request::Env
  def get_header: -> "x"
end

module Outer::Request::Helpers
  def header: -> "x"
end
```

## Sibling mixin resolution does not affect module chain

### update

```ruby
module Reader
  def read_value = 42
end
module User
  def use_value = read_value
end
class Container
  include Reader
  include User
end
```

### result

```rbs
class Container
  include Reader
  include User
end

module Reader
  def read_value: -> 42
end

module User
  def use_value: -> 42
end
```
