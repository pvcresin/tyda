# Sorbet / Sig / RBI Input

## Use return value of method defined in RBI

```rbi
class Calculator
  sig { params(x: Integer, y: Integer).returns(Integer) }
  def add(x, y); end
end
```

```ruby
class App
  def compute
    calc = Calculator.new
    calc.add(1, 2)
  end
end
```

### result

```rbs
class App
  def compute: -> Integer
end
```

## Method call chain from RBI definition

```rbi
class StringHelper
  sig { params(s: String).returns(Integer) }
  def length_of(s); end

  sig { params(n: Integer).returns(String) }
  def number_to_s(n); end
end
```

```ruby
class App
  def process
    helper = StringHelper.new
    helper.number_to_s(42)
  end
end
```

### result

```rbs
class App
  def process: -> String
end
```

## Instance method chain from RBI definition

```rbi
class Helper
  sig { params(n: Integer).returns(String) }
  def format(n); end
end
```

```ruby
class App
  def display
    h = Helper.new
    h.format(42)
  end
end
```

### result

```rbs
class App
  def display: -> String
end
```

## void method from RBI definition

```rbi
class Logger
  sig { params(msg: String).void }
  def log(msg); end
end
```

```ruby
class App
  def setup
    logger = Logger.new
    logger.log("started")
  end
end
```

### result

```rbs
class App
  def setup: -> void
end
```

## T::Boolean type from RBI definition

```rbi
class Validator
  sig { params(value: String).returns(T::Boolean) }
  def valid?(value); end
end
```

```ruby
class App
  def check
    v = Validator.new
    v.valid?("test")
  end
end
```

### result

```rbs
class App
  def check: -> bool
end
```

## T.nilable type from RBI definition

```rbi
class Finder
  sig { params(id: Integer).returns(T.nilable(String)) }
  def find(id); end
end
```

```ruby
class App
  def lookup
    f = Finder.new
    f.find(42)
  end
end
```

### result

```rbs
class App
  def lookup: -> String?
end
```

## T.any type from RBI definition

```rbi
class Parser
  sig { params(input: String).returns(T.any(Integer, Float)) }
  def parse(input); end
end
```

```ruby
class App
  def read
    p = Parser.new
    p.parse("42")
  end
end
```

### result

```rbs
class App
  def read: -> Integer | Float
end
```

## T::Array type from RBI definition

```rbi
class Collector
  sig { returns(T::Array[String]) }
  def items; end
end
```

```ruby
class App
  def get_items
    c = Collector.new
    c.items
  end
end
```

### result

```rbs
class App
  def get_items: -> Array[String]
end
```

## T::Hash type from RBI definition

```rbi
class Config
  sig { returns(T::Hash[Symbol, String]) }
  def settings; end
end
```

```ruby
class App
  def load_config
    c = Config.new
    c.settings
  end
end
```

### result

```rbs
class App
  def load_config: -> Hash[Symbol, String]
end
```

## Multiple params from RBI definition

```rbi
class Formatter
  sig { params(name: String, age: Integer, active: T::Boolean).returns(String) }
  def format(name, age, active); end
end
```

```ruby
class App
  def display
    f = Formatter.new
    f.format("Alice", 30, true)
  end
end
```

### result

```rbs
class App
  def display: -> String
end
```

## Define inheritance in RBI

```rbi
class Animal
  sig { returns(String) }
  def speak; end
end

class Dog < Animal
  sig { returns(String) }
  def speak; end

  sig { returns(Integer) }
  def age; end
end
```

```ruby
class App
  def test
    d = Dog.new
    d.age
  end
end
```

### result

```rbs
class App
  def test: -> Integer
end
```

## T.class_of from RBI definition

```rbi
class Registry
  sig { params(klass: T.class_of(String)).returns(String) }
  def register(klass); end
end
```

```ruby
class App
  def setup
    r = Registry.new
    r.register(String)
  end
end
```

### result

```rbs
class App
  def setup: -> String
end
```

## Complex nested type from RBI definition

```rbi
class DataStore
  sig { returns(T::Hash[String, T::Array[Integer]]) }
  def data; end
end
```

```ruby
class App
  def load_data
    ds = DataStore.new
    ds.data
  end
end
```

### result

```rbs
class App
  def load_data: -> Hash[String, Array[Integer]]
end
```

## RBI sig do end form

```rbi
class Service
  sig do
    params(
      url: String,
      timeout: Integer
    )
    .returns(String)
  end
  def fetch(url, timeout); end
end
```

```ruby
class App
  def call_service
    s = Service.new
    s.fetch("http://example.com", 30)
  end
end
```

### result

```rbs
class App
  def call_service: -> String
end
```

## RBI override modifier

```rbi
class Base
  sig { returns(String) }
  def name; end
end

class Derived < Base
  sig { override.returns(String) }
  def name; end

  sig { returns(Integer) }
  def extra; end
end
```

```ruby
class App
  def test
    d = Derived.new
    d.extra
  end
end
```

### result

```rbs
class App
  def test: -> Integer
end
```

## RBI abstract modifier

```rbi
class Shape
  sig { abstract.returns(Float) }
  def area; end
end

class Circle < Shape
  sig { override.params(radius: Float).void }
  def initialize(radius); end

  sig { override.returns(Float) }
  def area; end
end
```

```ruby
class App
  def calc
    c = Circle.new(5.0)
    c.area
  end
end
```

### result

```rbs
class App
  def calc: -> Float
end
```

## Nested T.nilable and T::Array from RBI definition

```rbi
class Search
  sig { params(query: String).returns(T.nilable(T::Array[String])) }
  def search(query); end
end
```

```ruby
class App
  def find
    s = Search.new
    s.search("hello")
  end
end
```

### result

```rbs
class App
  def find: -> Array[String]?
end
```

## checked modifier from RBI definition

```rbi
class FastParser
  sig { checked(:never).params(data: String).returns(Integer) }
  def parse(data); end
end
```

```ruby
class App
  def quick_parse
    fp = FastParser.new
    fp.parse("42")
  end
end
```

### result

```rbs
class App
  def quick_parse: -> Integer
end
```

## RBI and code inference together

```rbi
class ExternalLib
  sig { params(x: Integer).returns(String) }
  def convert(x); end
end
```

```ruby
class MyApp
  def from_rbi
    lib = ExternalLib.new
    lib.convert(42)
  end

  def from_inference = "hello world"
end
```

### result

```rbs
class MyApp
  def from_rbi: -> String
  def from_inference: -> "hello world"
end
```

## Define module in RBI

```rbi
module Serializable
  sig { returns(String) }
  def to_json; end
end
```

```ruby
class User
  def serialize = to_json
end
```

### result

```rbs
class User
  def serialize: -> untyped
end
```

## Multiple classes from RBI definition

```rbi
class UserRepo
  sig { params(id: Integer).returns(T.nilable(String)) }
  def find(id); end
end

class OrderRepo
  sig { params(user_id: Integer).returns(T::Array[Integer]) }
  def orders_for(user_id); end
end
```

```ruby
class App
  def user_orders
    orders = OrderRepo.new
    orders.orders_for(1)
  end
end
```

### result

```rbs
class App
  def user_orders: -> Array[Integer]
end
```

## RBS comment wins over RBI definition

```rbi
class Typed
  sig { returns(String) }
  def value; end
end
```

```ruby
class Typed
  #: -> Integer
  def value = 42
end
```

### result

```rbs
class Typed
  def value: -> Integer
end
```

## RBI and inline sig together

```rbi
class External
  sig { params(x: Integer).returns(String) }
  def from_rbi(x); end
end
```

```ruby
class External
  sig { params(y: Float).returns(Integer) }
  def from_inline(y) = y.to_i
end
```

### result

```rbs
class External
  def from_inline: (Float y) -> Integer
end
```

## Use RBI type in method chain

```rbi
class Builder
  sig { params(key: String).returns(Builder) }
  def set(key); end

  sig { returns(String) }
  def build; end
end
```

```ruby
class App
  def create
    b = Builder.new
    b.set("name").build
  end
end
```

### result

```rbs
class App
  def create: -> String
end
```

## RBI T.any param with multiple types

```rbi
class Converter
  sig { params(value: T.any(String, Integer, Float, Symbol)).returns(String) }
  def to_s_safe(value); end
end
```

```ruby
class App
  def convert
    c = Converter.new
    c.to_s_safe(42)
  end
end
```

### result

```rbs
class App
  def convert: -> String
end
```

## RBI required keyword arg

```rbi
class Server
  sig { params(host: String, port: Integer).returns(String) }
  def start(host:, port:); end
end
```

```ruby
class App
  def run
    s = Server.new
    s.start(host: "0.0.0.0", port: 8080)
  end
end
```

### result

```rbs
class App
  def run: -> String
end
```

## RBI optional keyword arg

```rbi
class Client
  sig { params(url: String, timeout: Integer).returns(String) }
  def fetch(url:, timeout: 30); end
end
```

```ruby
class App
  def call
    c = Client.new
    c.fetch(url: "https://example.com")
  end
end
```

### result

```rbs
class App
  def call: -> String
end
```

## RBI optional arg

```rbi
class Calc
  sig { params(x: Integer, y: Integer).returns(Integer) }
  def add(x, y = 0); end
end
```

```ruby
class App
  def compute
    c = Calc.new
    c.add(10)
  end
end
```

### result

```rbs
class App
  def compute: -> Integer
end
```

## RBI rest arg

```rbi
class Logger
  sig { params(messages: String).void }
  def log(*messages); end
end
```

```ruby
class App
  def setup
    l = Logger.new
    l.log("start", "init")
  end
end
```

### result

```rbs
class App
  def setup: -> void
end
```

## RBI keyword rest arg

```rbi
class Config
  sig { params(opts: String).returns(T::Hash[Symbol, String]) }
  def set(**opts); end
end
```

```ruby
class App
  def configure
    c = Config.new
    c.set(host: "localhost", port: "3000")
  end
end
```

### result

```rbs
class App
  def configure: -> Hash[Symbol, String]
end
```

## RBI mixed positional + keyword + rest

```rbi
class Builder
  sig { params(name: String, args: Integer, debug: T::Boolean).returns(String) }
  def build(name, *args, debug: false); end
end
```

```ruby
class App
  def create
    b = Builder.new
    b.build("foo", 1, 2, 3, debug: true)
  end
end
```

### result

```rbs
class App
  def create: -> String
end
```

## RBI block arg

```rbi
class Runner
  sig { params(name: String, blk: T.proc.params(x: Integer).returns(String)).returns(T::Array[String]) }
  def execute(name, &blk); end
end
```

```ruby
class App
  def run
    r = Runner.new
    r.execute("task") { |x| x.to_s }
  end
end
```

### result

```rbs
class App
  def run: -> Array[String]
end
```

## RBI T.nilable keyword arg

```rbi
class User
  sig { params(name: String, email: T.nilable(String)).returns(String) }
  def create(name:, email: nil); end
end
```

```ruby
class App
  def register
    u = User.new
    u.create(name: "Alice")
  end
end
```

### result

```rbs
class App
  def register: -> String
end
```

## RBI T::Array rest arg return

```rbi
class Collector
  sig { params(items: Integer).returns(T::Array[Integer]) }
  def gather(*items); end
end
```

```ruby
class App
  def collect
    c = Collector.new
    c.gather(1, 2, 3)
  end
end
```

### result

```rbs
class App
  def collect: -> Array[Integer]
end
```

## RBI sig do end with keyword arg

```rbi
class Database
  sig do
    params(
      host: String,
      port: Integer,
      ssl: T::Boolean,
    )
    .returns(String)
  end
  def connect(host:, port:, ssl: false); end
end
```

```ruby
class App
  def setup
    db = Database.new
    db.connect(host: "db.local", port: 5432)
  end
end
```

### result

```rbs
class App
  def setup: -> String
end
```

## RBI many params

```rbi
class Form
  sig do
    params(
      name: String,
      age: Integer,
      email: String,
      active: T::Boolean,
      role: Symbol,
      notes: T.nilable(String),
    )
    .returns(String)
  end
  def submit(name, age, email, active, role, notes); end
end
```

```ruby
class App
  def create
    f = Form.new
    f.submit("Alice", 30, "a@b.com", true, :admin, nil)
  end
end
```

### result

```rbs
class App
  def create: -> String
end
```

## RBI override with keyword arg

```rbi
class Base
  sig { params(x: Integer).returns(String) }
  def format(x); end
end

class Child < Base
  sig { override.params(x: Integer, verbose: T::Boolean).returns(String) }
  def format(x, verbose: false); end
end
```

```ruby
class App
  def run
    c = Child.new
    c.format(42, verbose: true)
  end
end
```

### result

```rbs
class App
  def run: -> String
end
```

## RBI T.all intersection type

```rbi
class Validator
  sig { params(x: T.all(Comparable, Enumerable)).returns(T.all(Comparable, Enumerable)) }
  def validate(x); end
end
```

```ruby
class App
  def check
    v = Validator.new
    v.validate(42)
  end
end
```

### result

```rbs
class App
  def check: -> Comparable & Enumerable
end
```

## RBI T.noreturn bot type

```rbi
class ErrorHandler
  sig { params(msg: String).returns(T.noreturn) }
  def fatal(msg); end
end
```

```ruby
class App
  def crash
    e = ErrorHandler.new
    e.fatal("boom")
  end
end
```

### result

```rbs
class App
  def crash: -> bot
end
```

## RBI T.anything top type

```rbi
class Box
  sig { params(value: T.anything).returns(T.anything) }
  def store(value); end
end
```

```ruby
class App
  def put
    b = Box.new
    b.store("hello")
  end
end
```

### result

```rbs
class App
  def put: -> top
end
```

## RBI T.proc parameter

```rbi
class Scheduler
  sig { params(task: T.proc.params(x: Integer).returns(String)).returns(String) }
  def schedule(task); end
end
```

```ruby
class App
  def plan
    s = Scheduler.new
    s.schedule(->(x) { x.to_s })
  end
end
```

### result

```rbs
class App
  def plan: -> String
end
```

## RBI T.all and T.nilable compound type

```rbi
class Finder
  sig { params(key: String).returns(T.nilable(T.all(Readable, Writable))) }
  def find(key); end
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
