# Ruby / Variable / Constant

## %w freeze constant emits literal tuple in RBS

### update

```ruby
class A
  CONST = %w(https).freeze
end
```

### result

```rbs
class A
  CONST: ["https"]
end
```

## %W freeze constant emits interpolated tuple shape in RBS

### update

```ruby
class A
  CONST = %W(https #{x}).freeze
end
```

### result

```rbs
class A
  CONST: ["https", String]
end
```

## Top-level constant

### update

```ruby
CONST = 42

def bar = CONST
```

### result

```rbs
CONST: 42

class Object
  def bar: -> 42
end
```

## Constant inside class

### update

```ruby
class A
  CONST = "1.0"

  def foo = CONST
end
```

### result

```rbs
class A
  CONST: "1.0"

  def foo: -> "1.0"
end
```

## Reference constant path

### update

```ruby
class A
  CONST = 3
end

def foo = A::CONST
```

### result

```rbs
class A
  CONST: 3
end

class Object
  def foo: -> 3
end
```

## Multiple constants

### update

```ruby
CONST1 = "x"
CONST2 = "y"
CONST3 = false

def info = CONST1
```

### result

```rbs
CONST1: "x"
CONST2: "y"
CONST3: false

class Object
  def info: -> "x"
end
```

## Return stdlib class constant as value

### update

```ruby
class Foo
  def bar = Time

  def baz
    klass = Time
    klass
  end
end
```

### result

```rbs
class Foo
  def bar: -> singleton(Time)
  def baz: -> singleton(Time)
end
```

## Built-in RUBY_* constants

### update

```ruby
class A
  def self.engine = RUBY_ENGINE
  def self.version = RUBY_VERSION
  def self.platform = RUBY_PLATFORM
  def self.patch = RUBY_PATCHLEVEL
  def self.release = RUBY_RELEASE_DATE
  def self.desc = RUBY_DESCRIPTION
end
```

### result

```rbs
class A
  def self.engine: -> String
  def self.version: -> String
  def self.platform: -> String
  def self.patch: -> Integer
  def self.release: -> String
  def self.desc: -> String
end
```

## Return value of ENV[key] and ENV.fetch

### update

```ruby
class A
  def self.home = ENV['HOME']
  def self.home_or_default = ENV.fetch('HOME', '/default')
end
```

### result

```rbs
class A
  def self.home: -> String?
  def self.home_or_default: -> String
end
```

## Treat RbConfig::CONFIG as Hash[String, String]

### update

```ruby
class A
  def self.host = RbConfig::CONFIG["host_os"]
  def self.all = RbConfig::CONFIG
end
```

### result

```rbs
class A
  def self.host: -> String?
  def self.all: -> Hash[String, String]
end
```

## Populate Hash constant in class body block

### update

```ruby
class Store
  TABLE = {}

  %w(alpha beta).each do |name|
    TABLE[name] = name.downcase.freeze
  end

  def self.table = TABLE
  def self.value = TABLE.fetch("alpha")
end
```

### result

```rbs
class Store
  TABLE: Hash["alpha" | "beta", String]

  def self.table: -> Hash["alpha" | "beta", String]
  def self.value: -> String
end
```

## Populate top-level Hash constant in block

### update

```ruby
TABLE = {}

%w(alpha beta).each do |name|
  TABLE[name] = name.length
end

def table = TABLE
```

### result

```rbs
TABLE: Hash["alpha" | "beta", 4 | 5]

class Object
  def table: -> Hash["alpha" | "beta", 4 | 5]
end
```

## Populate Hash constant through class body alias

### update

```ruby
class Store
  TABLE = {}
  table = TABLE

  %w(alpha beta).each do |name|
    table[name] = name.length
  end

  def self.table = TABLE
end
```

### result

```rbs
class Store
  TABLE: Hash["alpha" | "beta", 4 | 5]

  def self.table: -> Hash["alpha" | "beta", 4 | 5]
end
```

## stdlib constants like Math::PI and Float::INFINITY

### update

```ruby
class A
  def self.pi = Math::PI
  def self.inf = Float::INFINITY
  def self.sep = File::SEPARATOR
end
```

### result

```rbs
class A
  def self.pi: -> Float
  def self.inf: -> Float
  def self.sep: -> String
end
```

## Reuse module singleton ivar inside same method

### update

```ruby
module M
  def self.cached
    @count ||= 0
    @count + 1
  end
end
```

### result

```rbs
module M
  def self.cached: -> Integer
end
```
