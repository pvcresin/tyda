# Ruby / Class / Ivar Accessor

## initialize with manual accessor

### update

```ruby
class Point
  def initialize(x, y)
    @x = x
    @y = y
  end

  def x = @x

  def y = @y
end
Point.new(1, 2)
```

### result

```rbs
class Point
  def initialize: (Integer x, Integer y) -> void
  def x: -> Integer
  def y: -> Integer
end
```

## Assign chained untyped call to ivar

### update

```ruby
class Config
  attr_reader :limit, :label
  def initialize(options = {})
    @limit = options[:limit].to_i
    @label = options[:label].to_s
  end
end
```

### result

```rbs
class Config
  def limit: -> untyped
  def label: -> untyped
  def initialize: (?Hash[untyped, untyped] options) -> void
end
```

## Hand-written getter resolves on an instance

### update

```ruby
class Counter
  def initialize(start)
    @count = start
  end

  def count = @count
end

class Client
  def current = Counter.new(10).count
  def next_value = Counter.new(10).count + 1
end
```

### result

```rbs
class Client
  def current: -> Integer
  def next_value: -> Integer
end

class Counter
  def initialize: (Integer start) -> void
  def count: -> Integer
end
```

## Parameters of other methods do not bind to initialize arguments

### update

```ruby
class IvarSetter
  attr_reader :count, :name, :names

  def initialize(count)
    @count = count
    @name = nil
  end

  def rename(name)
    @name = name
    @names = [name]
  end
end

IvarSetter.new(1)
```

### result

```rbs
class IvarSetter
  def count: -> Integer
  def name: -> untyped
  def names: -> [untyped]
  def initialize: (Integer count) -> void
  def rename: (untyped name) -> [untyped]
end
```

## attr_reader of a deferred ivar resolves through the initialize call site

```ruby
class Direct
  attr_reader :version
  def initialize(v)
    @version = v.to_s
  end
  def get = @version
end

class DirectClient
  def run = Direct.new("1.0").version
end
```

### result

```rbs
class Direct
  def version: -> String
  def initialize: (String v) -> void
  def get: -> String
end

class DirectClient
  def run: -> String
end
```

## attr_reader of a deferred ivar resolves through a forwarding factory

```ruby
class Forwarded
  attr_reader :version
  def initialize(v)
    @version = v.to_s
  end
  def self.create(v) = new(v)
  def get = @version
end

class ForwardedClient
  def run = Forwarded.create("1.0").version
end
```

### result

```rbs
class Forwarded
  def version: -> String
  def initialize: (String v) -> void
  def self.create: (String v) -> Forwarded
  def get: -> String
end

class ForwardedClient
  def run: -> String
end
```

## factory forwarding a later parameter binds the matching initialize parameter

```ruby
class Shifted
  attr_reader :label
  def initialize(v)
    @label = v.to_s
  end
  def self.create(ignored, v) = new(v)
end

class ShiftedClient
  def run = Shifted.create(1, :name).label
end
```

### result

```rbs
class Shifted
  def label: -> String
  def initialize: (Symbol v) -> void
  def self.create: (Integer ignored, Symbol v) -> Shifted
end

class ShiftedClient
  def run: -> String
end
```

## deferred ivar with an untyped receiver stays untyped

```ruby
class Unknown
  attr_reader :version
  def initialize(v)
    @version = v.to_s
  end
end
```

### result

```rbs
class Unknown
  def version: -> untyped
  def initialize: (untyped v) -> void
end
```

## deferred ivar picks the receiver overload from the argument

```ruby
class Overloaded
  attr_reader :sum
  def initialize(n)
    @sum = n + 1.5
  end
end

Overloaded.new(1)
```

### result

```rbs
class Overloaded
  def sum: -> Float
  def initialize: (Integer n) -> void
end
```
