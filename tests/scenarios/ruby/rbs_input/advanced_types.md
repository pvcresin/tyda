# Ruby / RBS Input / Advanced Types

## Intersection return type

```rbs
class Sorter
  def sorted_first: (Array[Comparable & Enumerable] items) -> (Comparable & Enumerable)
end
```

```ruby
class App
  def run
    s = Sorter.new
    s.sorted_first([1, 2, 3])
  end
end
```

### result

```rbs
class App
  def run: -> Comparable & Enumerable
end
```

## Intersection parameter type

```rbs
class Processor
  def process: (Readable & Writable io) -> String
end
```

```ruby
class App
  def handle
    p = Processor.new
    p.process(STDOUT)
  end
end
```

### result

```rbs
class App
  def handle: -> String
end
```

## Intersection type with multiple members

```rbs
class Checker
  def check: (Comparable & Enumerable & Hashable item) -> bool
end
```

```ruby
class App
  def verify
    c = Checker.new
    c.check(42)
  end
end
```

### result

```rbs
class App
  def verify: -> bool
end
```

## top return type

```rbs
class Container
  def get: () -> top
end
```

```ruby
class App
  def fetch
    c = Container.new
    c.get
  end
end
```

### result

```rbs
class App
  def fetch: -> top
end
```

## top parameter type

```rbs
class Logger
  def log: (top message) -> void
end
```

```ruby
class App
  def write
    l = Logger.new
    l.log("hello")
  end
end
```

### result

```rbs
class App
  def write: -> void
end
```

## bot return type

```rbs
class Halter
  def halt: () -> bot
end
```

```ruby
class App
  def stop
    h = Halter.new
    h.halt
  end
end
```

### result

```rbs
class App
  def stop: -> bot
end
```

## bot parameter with top return

```rbs
class Absurd
  def impossible: (bot x) -> top
end
```

```ruby
class App
  def call
    a = Absurd.new
    a.impossible(nil)
  end
end
```

### result

```rbs
class App
  def call: -> top
end
```

## Proc parameter type

```rbs
class Executor
  def run_block: (^(Integer) -> String callback) -> String
end
```

```ruby
class App
  def call
    e = Executor.new
    e.run_block(->(x) { x.to_s })
  end
end
```

### result

```rbs
class App
  def call: -> String
end
```

## Proc parameter type checks arity

```rbs
class ProcArityExecutor
  def run_block: (^(Integer) -> String callback) -> String
               | (untyped callback) -> Integer
end
```

```ruby
class App
  def proc_arity
    e = ProcArityExecutor.new
    e.run_block(-> { "x" })
  end
end
```

### result

```rbs
class App
  def proc_arity: -> Integer
end
```

## Proc parameter type accepts untyped parameters

```rbs
class ProcUntypedExecutor
  def run_block: (^(?) -> String callback) -> String
               | (untyped callback) -> Integer
end
```

```ruby
class App
  def proc_untyped
    e = ProcUntypedExecutor.new
    e.run_block(->(value) { value.to_s })
  end
end
```

### result

```rbs
class App
  def proc_untyped: -> String
end
```

## Proc parameter type checks return type

```rbs
class ProcReturnExecutor
  def run_block: (^(Integer) -> String callback) -> String
               | (untyped callback) -> Integer
end
```

```ruby
class App
  def proc_return
    e = ProcReturnExecutor.new
    e.run_block(->(x) { 1 })
  end
end
```

### result

```rbs
class App
  def proc_return: -> Integer
end
```

## Proc parameter resolves generic return type

```rbs
class ProcGenericExecutor
  def run_block: [T] (^(Integer) -> T callback) -> T
end
```

```ruby
class App
  def proc_generic
    e = ProcGenericExecutor.new
    e.run_block(->(x) { "ok" })
  end
end
```

### result

```rbs
class App
  def proc_generic: -> "ok"
end
```

## Proc type with no args and void return

```rbs
class Runner
  def execute: (^() -> void task) -> void
end
```

```ruby
class App
  def start
    r = Runner.new
    r.execute(-> { puts "done" })
  end
end
```

### result

```rbs
class App
  def start: -> void
end
```

## Proc return type

```rbs
class Factory
  def create_handler: () -> ^(String) -> Integer
end
```

```ruby
class App
  def make
    f = Factory.new
    f.create_handler
  end
end
```

### result

```rbs
class App
  def make: -> Proc
end
```

## instance type in instance method

```rbs
class Builder
  def clone_self: () -> instance
end
```

```ruby
class App
  def duplicate
    b = Builder.new
    b.clone_self
  end
end
```

### result

```rbs
class App
  def duplicate: -> Builder
end
```

## instance type parameter in singleton method

```rbs
class InstanceAcceptor
  def self.accept: (instance item) -> String
                 | (untyped item) -> Integer
end
```

```ruby
class App
  def accept_instance
    InstanceAcceptor.accept(InstanceAcceptor.new)
  end
end
```

### result

```rbs
class App
  def accept_instance: -> String
end
```

## Method chain on instance type

```rbs
class Chainable
  def set_name: (String name) -> instance
  def set_age: (Integer age) -> instance
end
```

```ruby
class App
  def build
    c = Chainable.new
    c.set_name("Alice").set_age(30)
  end
end
```

### result

```rbs
class App
  def build: -> Chainable
end
```

## Union mixing top and bot

```rbs
class Flexible
  def anything_or_nothing: () -> (top | bot)
end
```

```ruby
class App
  def call
    f = Flexible.new
    f.anything_or_nothing
  end
end
```

### result

```rbs
class App
  def call: -> top
end
```

## intersection + nilable

```rbs
class Finder
  def find: (String key) -> (Readable & Writable)?
end
```

```ruby
class App
  def lookup
    f = Finder.new
    f.find("key")
  end
end
```

### result

```rbs
class App
  def lookup: -> (Readable & Writable)?
end
```
