# Ruby / Class / Class Instance Variable

## Singleton method can read class body `@x`

### update

```ruby
class A
  @count = 0

  def self.count = @count
end

def f = A.count
```

### result

```rbs
class A
  def self.count: -> 0
end

class Object < BasicObject
  def f: -> 0
end
```

## Resolve later class body ivar from singleton method

### update

```ruby
class A
  def self.name = @name

  @name = "name"
end

def f = A.name
```

### result

```rbs
class A
  def self.name: -> "name"
end

class Object < BasicObject
  def f: -> "name"
end
```

## Singleton method can read module body `@x`

### update

```ruby
module M
  @config = { enabled: true }

  def self.config = @config
end

def f = M.config[:enabled]
```

### result

```rbs
module M
  def self.config: -> { enabled: true }
end

class Object < BasicObject
  def f: -> true
end
```

## Singleton attr_reader returns class body ivar

### update

```ruby
class A
  class << self
    attr_reader :label
  end

  @label = "label"
end

def f = A.label
```

### result

```rbs
class A
  def self.label: -> "label"
end

class Object < BasicObject
  def f: -> "label"
end
```

## Singleton attr_accessor returns class body ivar

### update

```ruby
class Setting
  @enabled = false
  @stream = $stderr

  class << self
    attr_accessor :enabled, :stream
  end
end

def enabled = Setting.enabled
def stream = Setting.stream
```

### result

```rbs
class Object < BasicObject
  def enabled: -> false
  def stream: -> IO
end

class Setting
  def self.enabled: -> false
  def self.enabled=: (bool enabled) -> false
  def self.stream: -> IO
  def self.stream=: (IO stream) -> IO
end
```

## Conditional ivar in singleton method resolves later class

### update

```ruby
class Store
  def self.config
    @config ||= Config.new
  end

  class Config
    def name = "config"
  end
end

def f = Store.config.name
```

### result

```rbs
class Object < BasicObject
  def f: -> "config"
end

class Store
  def self.config: -> Store::Config
end

class Store::Config
  def name: -> "config"
end
```

## Keep class << self ivar separate from class body ivar

### update

```ruby
class A
  class << self
    @v = 1

    def get = @v
  end
end

def f = A.get
```

### result

```rbs
class A
  def self.get: -> untyped
end

class Object < BasicObject
  def f: -> untyped
end
```

## Keep class body and instance method ivars separate

### update

```ruby
class A
  @v = "class"

  def initialize
    @v = "instance"
  end

  def get_instance = @v

  def self.get_class = @v
end
```

### result

```rbs
class A
  def initialize: -> void
  def get_instance: -> "instance"
  def self.get_class: -> "class"
end
```
