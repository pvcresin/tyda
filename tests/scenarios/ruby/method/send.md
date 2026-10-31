# Ruby / Method / Send

## Resolve symbol literal with send

### update

```ruby
class Caller
  def greet = "hello"

  def call_greet = send(:greet)
end
```

### result

```rbs
class Caller
  def greet: -> "hello"
  def call_greet: -> "hello"
end
```

## Resolve symbol literal with public_send

### update

```ruby
class Publisher
  def publish = true

  def do_publish = public_send(:publish)
end
```

### result

```rbs
class Publisher
  def publish: -> true
  def do_publish: -> true
end
```

## Resolve string literal with public_send

### update

```ruby
class StringPublisher
  def publish = "published"

  def do_publish = public_send("publish")
end
```

### result

```rbs
class StringPublisher
  def publish: -> "published"
  def do_publish: -> "published"
end
```

## send with receiver

### update

```ruby
class Wrapper
  def value = 42
end

class Client
  def get_value(w) = w.send(:value)
end
```

### result

```rbs
class Client
  def get_value: (untyped w) -> untyped
end

class Wrapper
  def value: -> 42
end
```

## Resolve send family with known receiver type

### update

```ruby
class Target
  def value = 42
  def label = "ok"
  def active? = true
end

class Client
  def from_send = Target.new.send(:value)
  def from_public_send = Target.new.public_send("label")
  def from_dunder_send = Target.new.__send__(:active?)
end
```

### result

```rbs
class Client
  def from_send: -> 42
  def from_public_send: -> "ok"
  def from_dunder_send: -> true
end

class Target
  def value: -> 42
  def label: -> "ok"
  def active?: -> true
end
```

## Dynamic arg is untyped

### update

```ruby
class Dynamic
  def dispatch(method_name) = send(method_name)
end
```

### result

```rbs
class Dynamic
  def dispatch: (untyped method_name) -> untyped
end
```

## Resolve symbol literal with __send__

### update

```ruby
class Sender
  def ping = "pong"

  def do_ping = __send__(:ping)
end
```

### result

```rbs
class Sender
  def ping: -> "pong"
  def do_ping: -> "pong"
end
```

## Resolve static method name variables

### update

```ruby
class Item
  NAME = "label"

  def label = "ok"
  def active? = true

  def via_local
    name = :label
    send(name)
  end

  def via_const = public_send(NAME)

  def via_union(flag)
    name = flag ? :label : :active?
    public_send(name)
  end
end
```

### result

```rbs
class Item
  NAME: "label"

  def label: -> "ok"
  def active?: -> true
  def via_local: -> "ok"
  def via_const: -> "ok"
  def via_union: (untyped flag) -> (true | "ok")
end
```

## Resolve static method objects from variables

### update

```ruby
class Tool
  NAME = :build

  def build(value) = value.to_s

  def via_local
    name = :build
    method(name).call(1)
  end

  def via_const = public_method(NAME).call(:ok)
end
```

### result

```rbs
class Tool
  NAME: :build

  def build: (untyped value) -> String
  def via_local: -> String
  def via_const: -> String
end
```

## Resolve static unbound method objects

### update

```ruby
class Item
  def echo(value) = value
  def label(value) = value.to_s

  def self.via_bare = instance_method(:echo).bind_call(new, "ok")
  def self.via_public = public_instance_method(:label).bind_call(new, :ok)
end

class Caller
  def via_receiver = Item.instance_method(:label).bind_call(Item.new, 1)

  def via_local
    method = Item.public_instance_method(:echo)
    method.bind_call(Item.new, :ready)
  end

  def via_bind = Item.instance_method(:echo).bind(Item.new).call(:bound)
end
```

### result

```rbs
class Caller
  def via_receiver: -> String
  def via_local: -> :ready
  def via_bind: -> :bound
end

class Item
  def echo: (untyped value) -> untyped
  def label: (untyped value) -> String
  def self.via_bare: -> "ok"
  def self.via_public: -> String
end
```

## Resolve cached unbound method objects

### update

```ruby
class Source
  def label(value) = value.to_s

  METHOD = instance_method(:label)

  def self.via_const = METHOD.bind_call(new, :ok)
  def via_bind = METHOD.bind(self).call("ok")
end
```

### result

```rbs
class Source
  METHOD: Proc

  def label: (untyped value) -> String
  def self.via_const: -> String
  def via_bind: -> String
end
```

## Resolve project unbound method objects

### update

`lib/source.rb`

```ruby
class Source
  def label(value) = value.to_s
end
```

```ruby
class Use
  def via_project = Source.instance_method(:label).bind_call(Source.new, :ready)
  def via_bind = Source.public_instance_method(:label).bind(Source.new).call(1)
end
```

### result

```rbs
class Use
  def via_project: -> String
  def via_bind: -> String
end
```

## Resolve unbound method self return

### update

```ruby
module Feature
  def self.apply(base) = Module.instance_method(:prepend_features).bind_call(self, base)

  def self.bound(base)
    method = Module.instance_method(:prepend_features).bind(self)
    method.call(base)
  end
end
```

### result

```rbs
module Feature
  def self.apply: (untyped base) -> singleton(Feature)
  def self.bound: (untyped base) -> singleton(Feature)
end
```

## Resolve static send arguments

### update

```ruby
class Relay
  def echo(value) = value
  def label(name:) = name

  def via_send = send(:echo, "ok")
  def via_keyword = public_send(:label, name: :ready)
end
```

### result

```rbs
class Relay
  def echo: (String value) -> String
  def label: (name: Symbol) -> Symbol
  def via_send: -> String
  def via_keyword: -> Symbol
end
```

## Resolve receiver send arguments

### update

```ruby
class Receiver
  def echo(value) = value
end

class Dispatcher
  def via_receiver = Receiver.new.public_send(:echo, 1)
end
```

### result

```rbs
class Dispatcher
  def via_receiver: -> Integer
end

class Receiver
  def echo: (Integer value) -> Integer
end
```

## Resolve __method__ as send target

### update

```ruby
class Target
  def value = "ok"
end

class Caller
  def value = Target.new.public_send(__method__)
end
```

### result

```rbs
class Caller
  def value: -> "ok"
end

class Target
  def value: -> "ok"
end
```

## Resolve interpolated static dispatch names

### update

```ruby
class Target
  def read_name = "name"
  def read_count = 1
  def write_name(value) = value

  def via_string(flag)
    field = flag ? "name" : "count"
    public_send("read_#{field}")
  end

  def via_symbol
    method(:"read_#{:name}").call
  end

  def via_argument
    field = "name"
    send("write_#{field}", :ready)
  end
end
```

### result

```rbs
class Target
  def read_name: -> "name"
  def read_count: -> 1
  def write_name: (Symbol value) -> Symbol
  def via_string: (untyped flag) -> (1 | "name")
  def via_symbol: -> "name"
  def via_argument: -> Symbol
end
```

## Resolve static names converted with to_sym

### update

```ruby
class Target
  def read_name = "name"
  def read_count = 1
  def write_name(value) = value

  def via_to_sym(flag)
    field = flag ? "name" : "count"
    public_send("read_#{field}".to_sym)
  end

  def via_intern
    field = "name"
    send("write_#{field}".intern, :ready)
  end

  def via_method = method("read_#{:name}".to_sym).call
end
```

### result

```rbs
class Target
  def read_name: -> "name"
  def read_count: -> 1
  def write_name: (Symbol value) -> Symbol
  def via_to_sym: (untyped flag) -> (1 | "name")
  def via_intern: -> Symbol
  def via_method: -> "name"
end
```
