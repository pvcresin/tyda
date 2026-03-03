# Ruby / RBS Comment / Attr Annotation

## Set attr_reader type with #:

### update

```ruby
class User
  #: String
  attr_reader :name

  def initialize(name)
    @name = name
  end
end
```

### result

```rbs
class User
  def name: -> String
  def initialize: (untyped name) -> void
end
```

## Set attr_accessor type with #:

### update

```ruby
class Config
  #: Integer
  attr_accessor :port

  def initialize
    @port = 8080
  end
end
```

### result

```rbs
class Config
  def port: -> Integer
  def port=: (Integer port) -> Integer
  def initialize: -> void
end
```

## Local variable type assertion

### update

`sorbet/config`

```ruby
.
```

```ruby
class Asserter
  def compute
    x = 42 #: Integer
    y = "hello" #: String
    x
  end
end
```

### result

```rbs
class Asserter
  def compute: -> Integer
end
```

## Ivar and constant assertions fall back to code inference

### update

`sorbet/config`

```ruby
.
```

```ruby
class Fallback
  def test
    val = [1, 2, 3] #: Array[Integer]
    val
  end
end
```

### result

```rbs
class Fallback
  def test: -> Array[Integer]
end
```
