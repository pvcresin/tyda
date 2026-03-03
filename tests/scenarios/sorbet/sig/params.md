# Sorbet / Sig / Params

## Required keyword arg with sig

```ruby
class Foo
  sig { params(name: String, age: Integer).returns(String) }
  def greet(name:, age:) = "#{name}: #{age}"
end
```

### result

```rbs
class Foo
  def greet: (name: String, age: Integer) -> String
end
```

## Optional keyword arg with sig

```ruby
class Foo
  sig { params(host: String, port: Integer).void }
  def connect(host: "localhost", port: 3000)
  end
end
```

### result

```rbs
class Foo
  def connect: (?host: String, ?port: Integer) -> void
end
```

## Optional arg with sig

```ruby
class Foo
  sig { params(x: Integer, y: Integer).returns(Integer) }
  def add(x, y = 0) = x + y
end
```

### result

```rbs
class Foo
  def add: (Integer x, ?Integer y) -> Integer
end
```

## Rest arg with sig

```ruby
class Foo
  sig { params(args: Integer).returns(Integer) }
  def sum(*args) = 0
end
```

### result

```rbs
class Foo
  def sum: (*Integer args) -> Integer
end
```

## Keyword rest arg with sig

```ruby
class Foo
  sig { params(opts: String).returns(String) }
  def configure(**opts) = ""
end
```

### result

```rbs
class Foo
  def configure: (**String opts) -> String
end
```

## Block arg with sig

```ruby
class Foo
  sig { params(blk: T.proc.params(x: Integer).returns(String)).returns(String) }
  def with_block(&blk) = yield(1)
end
```

### result

```rbs
class Foo
  def with_block: (?Proc &blk) -> String
end
```

## Mixed positional optional and keyword args

```ruby
class Foo
  sig { params(name: String, age: Integer, active: T::Boolean).returns(String) }
  def register(name, age: 0, active: true) = name
end
```

### result

```rbs
class Foo
  def register: (String name, ?age: Integer, ?active: bool) -> String
end
```

## Mixed positional rest and keyword rest args

```ruby
class Foo
  sig { params(first: String, args: Integer, opts: String).void }
  def complex(first, *args, **opts)
  end
end
```

### result

```rbs
class Foo
  def complex: (String first, *Integer args, **String opts) -> void
end
```

## Rest keyword rest and block args together

```ruby
class Foo
  sig { params(args: Integer, kwargs: String, blk: T.proc.void).void }
  def forward(*args, **kwargs, &blk)
  end
end
```

### result

```rbs
class Foo
  def forward: (*Integer args, **String kwargs, ?Proc &blk) -> void
end
```

## T.nilable keyword arg

```ruby
class Foo
  sig { params(name: String, email: T.nilable(String)).returns(String) }
  def create(name:, email: nil) = name
end
```

### result

```rbs
class Foo
  def create: (name: String, ?email: String?) -> String
end
```

## T.any positional arg

```ruby
class Foo
  sig { params(value: T.any(String, Integer, Symbol)).returns(String) }
  def to_str(value) = value.to_s
end
```

### result

```rbs
class Foo
  def to_str: ((Integer | String | Symbol) value) -> String
end
```

## T::Array rest arg

```ruby
class Foo
  sig { params(items: String).returns(T::Array[String]) }
  def collect(*items) = items
end
```

### result

```rbs
class Foo
  def collect: (*String items) -> Array[String]
end
```

## sig do end with keyword arg

```ruby
class Foo
  sig do
    params(
      host: String,
      port: Integer,
      ssl: T::Boolean,
    )
    .returns(String)
  end
  def connect(host:, port:, ssl: false) = "#{host}:#{port}"
end
```

### result

```rbs
class Foo
  def connect: (host: String, port: Integer, ?ssl: bool) -> String
end
```

## sig do...end + rest + block

```ruby
class Foo
  sig do
    params(
      args: Integer,
      blk: T.proc.params(x: Integer).returns(String),
    )
    .returns(T::Array[String])
  end
  def map_all(*args, &blk) = []
end
```

### result

```rbs
class Foo
  def map_all: (*Integer args, ?Proc &blk) -> Array[String]
end
```

## Class method with keyword arg

```ruby
class Foo
  sig { params(name: String, force: T::Boolean).returns(Foo) }
  def self.create(name:, force: false) = new
end
```

### result

```rbs
class Foo
  def self.create: (name: String, ?force: bool) -> Foo
end
```

## Many params

```ruby
class Foo
  sig do
    params(
      a: String,
      b: Integer,
      c: Float,
      d: T::Boolean,
      e: Symbol,
      f: T.nilable(String),
    )
    .returns(String)
  end
  def many_params(a, b, c, d, e, f) = a
end
```

### result

```rbs
class Foo
  def many_params: (String a, Integer b, Float c, bool d, Symbol e, String? f) -> String
end
```

## override with keyword arg

```ruby
class Foo
  sig { override.params(name: String, age: Integer).returns(String) }
  def describe(name:, age:) = "#{name} (#{age})"
end
```

### result

```rbs
class Foo
  def describe: (name: String, age: Integer) -> String
end
```

## abstract with params

```ruby
class Foo
  sig { abstract.params(x: Integer, y: Integer).returns(Integer) }
  def compute(x, y); end
end
```

### result

```rbs
class Foo
  def compute: (Integer x, Integer y) -> Integer
end
```

## type_parameters with params

```ruby
class Foo
  sig { type_parameters(:T).params(items: T::Array[T.type_parameter(:T)], blk: T.proc.params(x: T.type_parameter(:T)).returns(String)).returns(T::Array[String]) }
  def transform(items, &blk) = []
end
```

### result

```rbs
class Foo
  def transform: (Array[T] items, ?Proc &blk) -> Array[String]
end
```
