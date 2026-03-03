# Ruby / Class / Mixin Callsite Inference

## Infer included module arg type from call sites

### update

```ruby
module Shared
  def foo(x) = x
end

class A
  include Shared

  def bar = foo("x")
end
```

### result

```rbs
class A
  include Shared

  def bar: -> String
end

module Shared
  def foo: (String x) -> String
end
```

## Follow nested includes from included module

### update

```ruby
module BaseShared
  def foo(x) = x
end

module Shared
  include BaseShared
end

class A
  include Shared

  def bar = foo("x")
end
```

### result

```rbs
class A
  include Shared

  def bar: -> String
end

module BaseShared
  def foo: (String x) -> String
end

module Shared
  include BaseShared
end
```

## Merge many class call sites and keyword args into mixin

### update

```ruby
module Configurable
  def configure(name:, enabled: false) = enabled ? name : nil
end

class UserSettings
  include Configurable

  def build
    configure(name: "profile", enabled: true)
    :done
  end
end

class AdminSettings
  include Configurable

  def build
    configure(name: "dashboard")
    :done
  end
end
```

### result

```rbs
class AdminSettings
  include Configurable

  def build: -> :done
end

module Configurable
  def configure: (name: String, ?enabled: bool) -> String?
end

class UserSettings
  include Configurable

  def build: -> :done
end
```

## Infer extended module class method args from call sites

### update

```ruby
module ClassHelpers
  def build_name(name) = name
end

class Report
  extend ClassHelpers

  def self.build = build_name("daily")
end
```

### result

```rbs
module ClassHelpers
  def build_name: (String name) -> String
end

class Report
  extend ClassHelpers

  def self.build: -> String
end
```

## Union optional positional default and call-site types

### update

```ruby
module Shared
  def foo(x = 1) = x
end

class A
  include Shared

  def bar = foo("x")
end
```

### result

```rbs
class A
  include Shared

  def bar: -> Integer | String
end

module Shared
  def foo: (?(Integer | String) x) -> (String | 1)
end
```
