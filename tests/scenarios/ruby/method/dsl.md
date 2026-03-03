# Ruby / Method / DSL

## Class-level DSL method

### update

```ruby
class Model
  def self.has_many(name) = name

  has_many :items
end
```

### result

```rbs
class Model
  def self.has_many: (Symbol name) -> Symbol
end
```

## Config DSL

### update

```ruby
class Config
  def self.setting(name) = name

  setting :debug
  setting :verbose
end
```

### result

```rbs
class Config
  def self.setting: (Symbol name) -> Symbol
end
```

## Register def in class_eval block as instance method

### update

```ruby
class Base
  class_eval do
    def dynamic
      "dynamic"
    end
  end

  def call_dynamic = dynamic
end
```

### result

```rbs
class Base
  def dynamic: -> "dynamic"
  def call_dynamic: -> "dynamic"
end
```

## Resolve later constant from def in class_eval block

### update

```ruby
class Box
  class_eval do
    def value
      VALUE
    end
  end

  VALUE = 1
end
```

### result

```rbs
class Box
  VALUE: 1

  def value: -> 1
end
```

## Resolve def in module_eval block from include target

### update

```ruby
module Helper
  module_eval do
    def label
      :ok
    end
  end
end

class Item
  include Helper

  def label_value = label
end
```

### result

```rbs
module Helper
  def label: -> :ok
end

class Item
  include Helper

  def label_value: -> :ok
end
```

## Register def in instance_eval block as singleton method

### update

```ruby
class Store
  instance_eval do
    def build
      :created
    end
  end
end

def create_store = Store.build
```

### result

```rbs
class Object
  def create_store: -> :created
end

class Store
  def self.build: -> :created
end
```

## instance_eval switches block self to the receiver

### update

```ruby
class Widget
  def size = 42
end

class Client
  def probe
    w = Widget.new
    w.instance_eval { size }
  end
end
```

### result

```rbs
class Client
  def probe: -> 42
end

class Widget
  def size: -> 42
end
```

## instance_exec switches self and binds block params

### update

```ruby
class Widget
  def size = 42
end

class Client
  def probe
    Widget.new.instance_exec(10) { |n| size + n }
  end
end
```

### result

```rbs
class Client
  def probe: -> Integer
end

class Widget
  def size: -> 42
end
```
