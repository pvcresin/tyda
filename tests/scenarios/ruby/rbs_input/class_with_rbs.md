# Ruby / RBS Input / Class With RBS

## RBS methods and inferred methods together

### update

```rbs
class User
  def name: -> String
end
```

```ruby
class User
  def name = "Alice"
  def age = 30
end
def test_rbs
  u = User.new
  u.name
end
def test_inferred
  u = User.new
  u.age
end
```

### result

```rbs
class Object < BasicObject
  def test_rbs: -> String
  def test_inferred: -> 30
end

class User
  def name: -> String
  def age: -> 30
end
```

## Chain using RBS return type

### update

```rbs
class Config
  def port: -> Integer
end
```

```ruby
class Config
  def port = 8080
  def host = "localhost"
end
def test_port
  c = Config.new
  c.port
end
def test_host
  c = Config.new
  c.host
end
def test_port_str
  c = Config.new
  c.port.to_s
end
```

### result

```rbs
class Config
  def port: -> Integer
  def host: -> "localhost"
end

class Object < BasicObject
  def test_port: -> Integer
  def test_host: -> "localhost"
  def test_port_str: -> String
end
```

## RBS annotations for multiple methods

### update

```rbs
class Formatter
  def format_name: (String first, String last) -> String
  def format_number: (Integer n) -> String
end
```

```ruby
class Formatter
  def format_name(first, last) = "#{first} #{last}"
  def format_number(n) = n.to_s
end
def test_name
  f = Formatter.new
  f.format_name("John", "Doe")
end
def test_number
  f = Formatter.new
  f.format_number(42)
end
```

### result

```rbs
class Formatter
  def format_name: (String first, String last) -> String
  def format_number: (Integer n) -> String
end

class Object < BasicObject
  def test_name: -> String
  def test_number: -> String
end
```

## Use RBS method return to infer another method

### update

```rbs
class DataSource
  def fetch_count: -> Integer
  def fetch_name: -> String
end
```

```ruby
class DataSource
  def fetch_count = 100
  def fetch_name = "test"
end
def test_count_float
  ds = DataSource.new
  ds.fetch_count.to_f
end
def test_name_length
  ds = DataSource.new
  ds.fetch_name.length
end
```

### result

```rbs
class DataSource
  def fetch_count: -> Integer
  def fetch_name: -> String
end

class Object < BasicObject
  def test_count_float: -> Float
  def test_name_length: -> Integer
end
```

## Resolve method on class without RBS annotation

### update

```ruby
class Calculator
  def add(a, b) = 42
  def name = "calc"
end
def test_add
  c = Calculator.new
  c.add(1, 2)
end
def test_name
  c = Calculator.new
  c.name
end
```

### result

```rbs
class Calculator
  def add: (Integer a, Integer b) -> 42
  def name: -> "calc"
end

class Object < BasicObject
  def test_add: -> 42
  def test_name: -> "calc"
end
```

## Multiple method calls on same instance

### update

```rbs
class Converter
  def to_string: (Integer n) -> String
  def to_int: (String s) -> Integer
end
```

```ruby
class Converter
  def to_string(n) = n.to_s
  def to_int(s) = s.to_i
end
def test
  c = Converter.new
  s = c.to_string(42)
  c.to_int(s)
end
```

### result

```rbs
class Converter
  def to_string: (Integer n) -> String
  def to_int: (String s) -> Integer
end

class Object < BasicObject
  def test: -> Integer
end
```

## Inline RBS comment wins over RBS file

### update

```rbs
class Api
  def status: -> Symbol
  def version: -> String
end
```

```ruby
class Api
  #: () -> :ok
  def status = :ok

  def version = "1.0"
end
```

### result

```rbs
class Api
  def status: -> :ok
  def version: -> String
end
```
