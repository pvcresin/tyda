# Ruby / Class / Attr

## Infer attr_reader type

### update

```ruby
class A
  def initialize(x, y)
    @x = x
    @y = y
  end

  attr_reader :x, :y
end

A.new(1, "hello")
```

### result

```rbs
class A
  def initialize: (Integer x, String y) -> void
  def x: -> 1
  def y: -> "hello"
end
```

## Infer attr_writer type

### update

```ruby
class A
  attr_writer :name

  def initialize(name)
    @name = name
  end
end

A.new("Alice")
```

### result

```rbs
class A
  def name=: (String name) -> "Alice"
  def initialize: (String name) -> void
end
```

## Infer attr_accessor type

### update

```ruby
class A
  attr_accessor :x, :y

  def initialize(x, y)
    @x = x
    @y = y
  end
end

A.new(1, 2)
```

### result

```rbs
class A
  def x: -> 1
  def x=: (Integer x) -> 1
  def y: -> 2
  def y=: (Integer y) -> 2
  def initialize: (Integer x, Integer y) -> void
end
```

## Infer splatted attr name lists

### update

```ruby
class Entry
  BASE = %i[name count]
  EXTRA = [:token, :ignored]

  attr_reader *(BASE + EXTRA - [:ignored])
  attr_accessor *%i[flag]

  def initialize
    @name = "entry"
    @count = 1
    @token = :token
    @flag = true
  end

  def snapshot = [name, count, token, flag]
end
```

### result

```rbs
class Entry
  BASE: [:name, :count]
  EXTRA: [:token, :ignored]

  def name: -> "entry"
  def count: -> 1
  def token: -> :token
  def flag: -> true
  def flag=: (bool flag) -> true
  def initialize: -> void
  def snapshot: -> ["entry", 1, :token, true]
end
```

## Collect attr_* inside visibility wrappers

### update

```ruby
class Entry
  def initialize(name, token, count)
    @name = name
    @token = token
    @count = count
  end

  private attr_reader :name
  protected attr_accessor :token
  public attr_writer :count

  def label = name
end

Entry.new("entry", :token, 3).label
```

### result

```rbs
class Entry
  def initialize: (String name, Symbol token, Integer count) -> void
  private def name: -> "entry"
  private def token: -> :token
  private def token=: (Symbol token) -> :token
  def count=: (Integer count) -> 3
  def label: -> "entry"
end
```

## Infer attr_reader ivar from same-class method call

### update

```ruby
class A
  attr_reader :x

  def initialize
    @x = build
  end

  def build = "hello"
end
```

### result

```rbs
class A
  def x: -> "hello"
  def initialize: -> void
  def build: -> "hello"
end
```

## Infer attr_reader ivar from private helper return

### update

```ruby
class A
  attr_reader :home

  def initialize(env)
    @home = normalize(env)
  end

  private

  def normalize(x)
    x.to_s
  end
end

A.new("/tmp")
```

### result

```rbs
class A
  def home: -> String
  def initialize: (String env) -> void
  private def normalize: (String x) -> String
end
```

## attr_reader calls through other classes return concrete type

### update

```ruby
class Repo
  attr_reader :name
  def initialize(name)
    @name = name
  end

  def self.make = new("hello")
end

class A
  def self.test
    r = Repo.make
    r.name
  end
end
```

### result

```rbs
class A
  def self.test: -> "hello"
end

class Repo
  def name: -> "hello"
  def initialize: (String name) -> void
  def self.make: -> Repo
end
```

## Resolve generated attr methods from call sites

### update

```ruby
class Profile
  attr_reader :name
  attr_accessor :age
  attr_writer :token

  def initialize
    @name = "Ada"
    @age = 20
  end
end

class ProfileUse
  def read_name = Profile.new.name
  def read_age = Profile.new.age

  def write_age
    profile = Profile.new
    profile.age = 21
  end

  def write_token
    profile = Profile.new
    profile.token = "secret"
  end
end
```

### result

```rbs
class Profile
  def name: -> "Ada"
  def age: -> Integer
  def age=: (Integer age) -> 20
  def token=: (String token) -> untyped
  def initialize: -> void
end

class ProfileUse
  def read_name: -> "Ada"
  def read_age: -> 20
  def write_age: -> 21
  def write_token: -> "secret"
end
```

## attr generates only readers

### update

```ruby
class Item
  attr :name

  def initialize
    @name = "item"
  end
end

Item.new.name
```

### result

```rbs
class Item
  def name: -> "item"
  def initialize: -> void
end
```

## attr true also generates writer

### update

```ruby
class Item
  attr :name, true

  def initialize
    @name = "item"
  end
end

item = Item.new
item.name = "next"
item.name
```

### result

```rbs
class Item
  def name: -> "item"
  def name=: (String name) -> "item"
  def initialize: -> void
end
```

## attr treats many names and string names as readers

### update

```ruby
class Item
  attr :name, "label"

  def initialize
    @name = :item
    @label = "label"
  end
end
```

### result

```rbs
class Item
  def name: -> :item
  def label: -> "label"
  def initialize: -> void
end
```

## Collect attr inside visibility wrappers

### update

```ruby
class Item
  def initialize
    @name = "item"
    @count = 1
  end

  private attr :name
  protected attr :count, true

  def label = name
end

item = Item.new
item.count = 2
item.label
```

### result

```rbs
class Item
  def initialize: -> void
  private def name: -> "item"
  private def count: -> 1
  private def count=: (Integer count) -> 1
  def label: -> "item"
end
```

## Resolve attr_reader through singleton method in another file

### update

```ruby
module Rake
  class Application
    attr_reader :original_dir
    def initialize
      @original_dir = Dir.pwd
    end
  end

  class << self
    def application
      @application ||= Rake::Application.new
    end

    def original_dir
      application.original_dir
    end
  end
end
```

### result

```rbs
module Rake
  def self.application: -> Rake::Application
  def self.original_dir: -> String
end

class Rake::Application
  def original_dir: -> String
  def initialize: -> void
end
```

## Nested `Const = Struct.new(...)` uses call-site attrs

### update

```ruby
class Holder
  Tuple = Struct.new(:spec, :source)

  def make
    Tuple.new("hello", 42)
  end
end
```

### result

```rbs
class Holder
  def make: -> Holder::Tuple
end

class Holder::Tuple
  def spec: -> "hello"
  def spec=: (String spec) -> "hello"
  def source: -> 42
  def source=: (Integer source) -> 42
  def initialize: (String spec, Integer source) -> void
  def self.members: -> Array[:source | :spec]
end
```

## attr_accessor getter reflects external setter writes

### update

```ruby
class Account
  attr_accessor :balance

  def initialize
    @balance = 0
  end
end

Account.new.balance = 100
Account.new.balance = "frozen"
```

### result

```rbs
class Account
  def balance: -> Integer | String
  def balance=: ((Integer | String) balance) -> 0
  def initialize: -> void
end
```
