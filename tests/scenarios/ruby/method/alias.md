# Ruby / Method / Alias

## alias keyword

### update

```ruby
class A
  def hello = "hello"

  alias hi hello
end
```

### result

```rbs
class A
  def hello: -> "hello"
  alias hi hello
end
```

## alias_method

### update

```ruby
class A
  def add(x, y) = x

  alias_method :plus, :add
end
```

### result

```rbs
class A
  def add: (untyped x, untyped y) -> untyped
  alias plus add
end
```

## alias_method splatted name pair

### update

```ruby
class Entry
  PAIR = %i[target source]

  def source = "value"

  alias_method *PAIR

  def read = target
end
```

### result

```rbs
class Entry
  PAIR: [:target, :source]

  def source: -> "value"
  def read: -> "value"
  alias target source
end
```

## alias preserves parameters

### update

```ruby
class Item
  def pair(left, right) = [left, right]

  alias tuple pair

  def call_tuple = tuple("name", 1)
end
```

### result

```rbs
class Item
  def pair: (String left, Integer right) -> [String, Integer]
  def call_tuple: -> [String, Integer]
  alias tuple pair
end
```

## alias_method preserves keywords

### update

```ruby
class Table
  def update(key, value:, **options)
    [key, value, options]
  end

  alias_method :merge_row, :update

  def call_merge = merge_row(:name, value: "A", extra: 1)
end
```

### result

```rbs
class Table
  def update: (Symbol key, value: String, **Integer options) -> [Symbol, String, { extra: 1, value: "A" }]
  def call_merge: -> [Symbol, String, Hash[Symbol, untyped]]
  alias merge_row update
end
```

## alias preserves operator parameters

### update

```ruby
class Store
  def []=(key, value) = value

  alias write []=

  def call_write = write(:name, "A")
end
```

### result

```rbs
class Store
  def []=: (Symbol key, String value) -> String
  def call_write: -> String
  alias write []=
end
```

## alias_method inside visibility wrapper

### update

```ruby
class Policy
  def allowed? = true

  private alias_method(:permitted?, :allowed?)

  def check = permitted?
end
```

### result

```rbs
class Policy
  def allowed?: -> true
  def check: -> true
  alias permitted? allowed?
end
```

## alias_method through static send

### update

```ruby
class Entry
  def source(value:) = value

  target = :copy
  send(:alias_method, target, :source)

  def read_copy = copy(value: "value")
end
```

### result

```rbs
class Entry
  def source: (value: String) -> String
  def read_copy: -> String
  alias copy source
end
```

## alias_method through static receiver send

### update

```ruby
class Group
  class Item
    def name = "name"

    class << self
      def build = :item
    end
  end

  Item.send(:alias_method, :title, :name)
  Item.singleton_class.send(:alias_method, :create, :build)
end

def read_title = Group::Item.new.title
def read_create = Group::Item.create
```

### result

```rbs
class Group::Item
  def name: -> "name"
  def self.build: -> :item
  alias title name
  alias self.create self.build
end

class Object
  def read_title: -> "name"
  def read_create: -> :item
end
```

## alias resolves return type

### update

```ruby
class A
  def foo = "formatted"

  def bar = 42

  alias x foo
  alias y bar
end
```

### result

```rbs
class A
  def foo: -> "formatted"
  def bar: -> 42
  alias x foo
  alias y bar
end
```

## Resolve aliased name from call site

### update

```ruby
class AliasCall
  def source = "source"

  alias target source
  alias_method :also_target, :source

  def call_target = target
  def call_also_target = also_target
  def call_receiver = AliasCall.new.target
end
```

### result

```rbs
class AliasCall
  def source: -> "source"
  def call_target: -> "source"
  def call_also_target: -> "source"
  def call_receiver: -> "source"
  alias target source
  alias also_target source
end
```

## Alias singleton method

### update

```ruby
class A
  class << self
    def build = "instance"

    alias create build
  end
end
```

### result

```rbs
class A
  def self.build: -> "instance"
  alias self.create self.build
end
```

## Alias singleton method with parameters

### update

```ruby
class Builder
  class << self
    def build(name) = name

    alias create build
  end

  def call = Builder.create("box")
end
```

### result

```rbs
class Builder
  def self.build: (String name) -> String
  def call: -> String
  alias self.create self.build
end
```

## alias_method on a constant receiver lands on that class

### update

```ruby
class Bar
  def to_s = "b"
end

class Foo
  Bar.alias_method(:new_to_s, :to_s)
end

def f = Bar.new.new_to_s
```

### result

```rbs
class Bar
  def to_s: -> "b"
  alias new_to_s to_s
end

class Object
  def f: -> "b"
end
```
