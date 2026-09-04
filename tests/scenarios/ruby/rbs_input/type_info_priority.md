# Ruby / RBS Input / Type-info priority

## Stronger type information wins, and removal falls back

### update

```ruby
class Value
  def fetch = :inferred
end

def read_value = Value.new.fetch
```

### result

```rbs
class Object < BasicObject
  def read_value: -> :inferred
end

class Value
  def fetch: -> :inferred
end
```

### update

```rbi
class Value
  sig { returns(String) }
  def fetch; end
end
```

```ruby
class Value
  def fetch = :inferred
end

def read_value = Value.new.fetch
```

### result

```rbs
class Object < BasicObject
  def read_value: -> String
end

class Value
  def fetch: -> String
end
```

### update

```rbs
class Value
  def fetch: -> Integer
end
```

```rbi
class Value
  sig { returns(String) }
  def fetch; end
end
```

```ruby
class Value
  def fetch = :inferred
end

def read_value = Value.new.fetch
```

### result

```rbs
class Object < BasicObject
  def read_value: -> Integer
end

class Value
  def fetch: -> Integer
end
```

### update

```rbs
class Value
  def fetch: -> Integer
end
```

```rbi
class Value
  sig { returns(String) }
  def fetch; end
end
```

```ruby
class Value
  sig { returns(Symbol) }
  def fetch = :inferred
end

def read_value = Value.new.fetch
```

### result

```rbs
class Object < BasicObject
  def read_value: -> Symbol
end

class Value
  def fetch: -> Symbol
end
```

### update

```rbs
class Value
  def fetch: -> Integer
end
```

```rbi
class Value
  sig { returns(String) }
  def fetch; end
end
```

```ruby
class Value
  sig { returns(Symbol) }
  #: () -> bool
  def fetch = :inferred
end

def read_value = Value.new.fetch
```

### result

```rbs
class Object < BasicObject
  def read_value: -> bool
end

class Value
  def fetch: -> bool
end
```

## Removing explicit type information falls back in reverse order

### update

```rbs
class Value
  def fetch: -> Integer
end
```

```rbi
class Value
  sig { returns(String) }
  def fetch; end
end
```

```ruby
class Value
  sig { returns(Symbol) }
  #: () -> bool
  def fetch = :inferred
end

def read_value = Value.new.fetch
```

### result

```rbs
class Object < BasicObject
  def read_value: -> bool
end

class Value
  def fetch: -> bool
end
```

### update

```rbs
class Value
  def fetch: -> Integer
end
```

```rbi
class Value
  sig { returns(String) }
  def fetch; end
end
```

```ruby
class Value
  sig { returns(Symbol) }
  def fetch = :inferred
end

def read_value = Value.new.fetch
```

### result

```rbs
class Object < BasicObject
  def read_value: -> Symbol
end

class Value
  def fetch: -> Symbol
end
```

### update

```rbs
class Value
  def fetch: -> Integer
end
```

```rbi
class Value
  sig { returns(String) }
  def fetch; end
end
```

```ruby
class Value
  def fetch = :inferred
end

def read_value = Value.new.fetch
```

### result

```rbs
class Object < BasicObject
  def read_value: -> Integer
end

class Value
  def fetch: -> Integer
end
```

### update

```rbi
class Value
  sig { returns(String) }
  def fetch; end
end
```

```ruby
class Value
  def fetch = :inferred
end

def read_value = Value.new.fetch
```

### result

```rbs
class Object < BasicObject
  def read_value: -> String
end

class Value
  def fetch: -> String
end
```

### update

```ruby
class Value
  def fetch = :inferred
end

def read_value = Value.new.fetch
```

### result

```rbs
class Object < BasicObject
  def read_value: -> :inferred
end

class Value
  def fetch: -> :inferred
end
```
