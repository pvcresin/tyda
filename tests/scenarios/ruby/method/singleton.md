# Ruby / Method / Singleton

## Basic singleton method

### update

```ruby
class A
  def self.bar = 42
end
```

### result

```rbs
class A
  def self.bar: -> 42
end
```

## Singleton and instance methods together

### update

```ruby
class A
  def self.create(name) = 42

  def instance_method = "hello"
end
```

### result

```rbs
class A
  def self.create: (untyped name) -> 42
  def instance_method: -> "hello"
end
```

## Multiple singleton methods

### update

```ruby
class A
  def self.version = "1.0"

  def self.debug? = false
end
```

### result

```rbs
class A
  def self.version: -> "1.0"
  def self.debug?: -> false
end
```

## Basic class << self

### update

```ruby
class A
  class << self
    def bar = 42

    def baz(name) = "hello"
  end
end
```

### result

```rbs
class A
  def self.bar: -> 42
  def self.baz: (untyped name) -> "hello"
end
```

## class << self with instance methods

### update

```ruby
class A
  class << self
    def call(arg) = "result"
  end

  def process = true
end
```

### result

```rbs
class A
  def self.call: (untyped arg) -> "result"
  def process: -> true
end
```

## Mix class << self and def self

### update

```ruby
class A
  def self.version = "1.0"

  class << self
    def start = true

    def stop = false
  end

  def self.reset = nil
end
```

### result

```rbs
class A
  def self.version: -> "1.0"
  def self.start: -> true
  def self.stop: -> false
  def self.reset: -> nil
end
```

## attr_reader and attr_accessor inside class << self

### update

```ruby
class A
  class << self
    attr_reader :instance
    attr_accessor :debug
  end
end
```

### result

```rbs
class A
  def self.instance: -> untyped
  def self.debug: -> untyped
  def self.debug=: (untyped debug) -> untyped
end
```

## RBS-commented method inside class << self

### update

```ruby
class A
  class << self
    #: (Integer) -> String
    def convert(n) = n.to_s
  end
end
```

### result

```rbs
class A
  def self.convert: (Integer n) -> String
end
```

## Use class << self call site to refine signature without external RBS

### update

```ruby
class A
  class << self
    def foo(x) = x

    def bar = foo("x")
  end
end
```

### result

```rbs
class A
  def self.foo: (String x) -> String
  def self.bar: -> String
end
```

## class << self in module

### update

```ruby
module Helpers
  class << self
    def format_name(first, last) = "#{first} #{last}"
  end
end
```

### result

```rbs
module Helpers
  def self.format_name: (untyped first, untyped last) -> String
end
```

## Create instance with self.new

### update

```ruby
class A
  def initialize(x)
    @x = x
  end

  def self.build = self.new(1)
end
```

### result

```rbs
class A
  def initialize: (Integer x) -> void
  def self.build: -> A
end
```

## Create instance with bare new inside singleton method

### update

```ruby
class A
  def initialize(x)
    @x = x
  end

  def self.null = new(0)
end
```

### result

```rbs
class A
  def initialize: (Integer x) -> void
  def self.null: -> A
end
```

## self.bare method call resolves class method

### update

```ruby
class A
  def self.helper = 42

  def self.entry = self.helper
end
```

### result

```rbs
class A
  def self.helper: -> 42
  def self.entry: -> 42
end
```
