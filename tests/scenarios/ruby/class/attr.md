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
  def x: -> Integer
  def y: -> String
end
```

## Initialize argument widening reaches ivar-derived methods

### update

```ruby
class User
  def initialize(name) = @name = name
  def ids = [1, 2, 3].map { |id| id * 2 }
  def summary = [@name, ids.first]
end

User.new("Alice")
```

### result

```rbs
class User
  def initialize: (String name) -> void
  def ids: -> Array[Integer]
  def summary: -> [String, Integer?]
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
  def name=: (String name) -> String
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
  def x: -> Integer
  def x=: (Integer x) -> Integer
  def y: -> Integer
  def y=: (Integer y) -> Integer
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
  def flag=: (bool flag) -> bool
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
  private def name: -> String
  private def token: -> Symbol
  private def token=: (Symbol token) -> Symbol
  def count=: (Integer count) -> Integer
  def label: -> String
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
  def self.test: -> String
end

class Repo
  def name: -> String
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
  def age=: (Integer age) -> Integer
  def token=: (String token) -> String
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
  def name: -> String
  def name=: (String name) -> String
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

  def set_count
    self.count = 2
  end

  def label = name
end

item = Item.new
item.set_count
item.label
```

### result

```rbs
class Item
  def initialize: -> void
  private def name: -> "item"
  private def count: -> 1 | 2
  private def count=: (Integer count) -> Integer
  def set_count: -> 2
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
  def spec: -> String
  def spec=: (String spec) -> String
  def source: -> Integer
  def source=: (Integer source) -> Integer
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
  def balance=: ((Integer | String) balance) -> (Integer | String)
  def initialize: -> void
end
```

## Local receiver setter refines a later getter

### update

```ruby
class DirectStore
  attr_accessor :value
  attr_accessor :other

  def touch; end

  def custom=(value)
    @custom = value.to_s
  end

  attr_reader :custom
end

def store_direct
  store = DirectStore.new
  store.value = 1
  store.value
end

def store_last_write_wins
  store = DirectStore.new
  store.value = 1
  store.value = :two
  store.value
end

def store_branch_joins_writes(flag)
  store = DirectStore.new
  if flag
    store.value = 1
  else
    store.value = :two
  end
  store.value
end

def store_loop_write_ends_fact(flag)
  store = DirectStore.new
  store.value = 1
  while flag
    store.value = :two
  end
  store.value
end

def store_string_keeps_class
  store = DirectStore.new
  store.value = "text"
  store.value
end

def store_call_ends_fact
  store = DirectStore.new
  store.value = 1
  store.touch
  store.value
end

def store_alias_ends_fact
  store = DirectStore.new
  other = store
  store.value = 1
  other.value = :two
  store.value
end

def store_parameter_keeps_no_fact(store)
  store.value = 1
  store.value
end

def store_custom_writer_keeps_no_fact
  store = DirectStore.new
  store.custom = 1
  store.custom
end

def store_array_keeps_no_fact
  store = DirectStore.new
  store.other = []
  store.other << 1
  store.other
end
```

### result

```rbs
class DirectStore
  def value: -> Integer | String | Symbol
  def value=: ((Integer | String | Symbol) value) -> (Integer | String | Symbol)
  def other: -> [ ]
  def other=: ([ ] other) -> [ ]
  def touch: -> nil
  def custom=: (Integer value) -> String
  def custom: -> String | untyped
end

class Object < BasicObject
  def store_direct: -> 1
  def store_last_write_wins: -> :two
  def store_branch_joins_writes: (untyped flag) -> (1 | :two)
  def store_loop_write_ends_fact: (untyped flag) -> untyped
  def store_string_keeps_class: -> String
  def store_call_ends_fact: -> untyped
  def store_alias_ends_fact: -> untyped
  def store_parameter_keeps_no_fact: (untyped store) -> untyped
  def store_custom_writer_keeps_no_fact: -> String | untyped
  def store_array_keeps_no_fact: -> untyped
end
```

## Attribute readers preserve known record shape around untyped fields

### update

```rbs
class OptionalCluster
  def group_type?: -> bool
end

class ClusterSource
  def self.fetch: -> (OptionalCluster | untyped)
end
```

```ruby
class AttributeRecordHolder
  attr_reader :options

  def initialize(options: {})
    @options = options
  end
end

def build_attribute_record_holder
  cluster = ClusterSource.fetch
  AttributeRecordHolder.new(options: { fixed: true, dynamic: cluster.group_type? })
end
```

### result

```rbs
class AttributeRecordHolder
  def options: -> Hash[untyped, untyped] | { fixed: bool, dynamic: bool | untyped }
  def initialize: (?options: (Hash[untyped, untyped] | { fixed: bool, dynamic: bool })) -> void
end

class Object < BasicObject
  def build_attribute_record_holder: -> AttributeRecordHolder
end
```

## Keep an unknown hash delete result from narrowing to nil

### update

```ruby
class ClusterParamProject
  def id = 1
end

class ClusterParamGroup
  def id = 1
end

class ClusterServiceParams
  attr_reader :params

  def initialize(params = {})
    @params = params.dup
  end

  def clusterable = @clusterable ||= params.delete(:clusterable)

  def clusterable_id
    case clusterable
    when ClusterParamProject
      clusterable.id
    when ClusterParamGroup
      clusterable.id
    end
  end
end
```

### result

```rbs
class ClusterParamGroup
  def id: -> 1
end

class ClusterParamProject
  def id: -> 1
end

class ClusterServiceParams
  def params: -> untyped
  def initialize: (?Hash[untyped, untyped] params) -> void
  def clusterable: -> untyped
  def clusterable_id: -> nil | untyped
end
```

## Class body locals stay visible to lambdas passed to writers

### update

```ruby
class ClassBodyLambdaCapture
  class << self
    attr_accessor :filter
  end

  pattern = /a/
  self.filter = lambda { |value| pattern.match?(value) }
  FILTER = lambda { |value| pattern.match?(value) }
end

top_level_pattern = /a/
TOP_LEVEL_SOURCES = [1].map { |_| top_level_pattern.source }
```

### result

```rbs
TOP_LEVEL_SOURCES: Array[String]

class ClassBodyLambdaCapture
  FILTER: Proc

  def self.filter: -> untyped
  def self.filter=: (Proc filter) -> Proc
end
```
