# Rails / Active Record / Class Attribute

## Generate class_attribute getter setter and predicate

### update

```ruby
class Base
  class_attribute :timeout
end
```

### result

```rbs
class Base
  def timeout: -> untyped
  def timeout=: (untyped timeout) -> untyped
  def timeout?: -> bool
  def self.timeout: -> untyped
  def self.timeout=: (untyped timeout) -> untyped
  def self.timeout?: -> bool
end
```

## Define multiple class_attribute names at once

### update

```ruby
class Config
  class_attribute :debug, :verbose
end
```

### result

```rbs
class Config
  def debug: -> untyped
  def debug=: (untyped debug) -> untyped
  def debug?: -> bool
  def self.debug: -> untyped
  def self.debug=: (untyped debug) -> untyped
  def self.debug?: -> bool
  def verbose: -> untyped
  def verbose=: (untyped verbose) -> untyped
  def verbose?: -> bool
  def self.verbose: -> untyped
  def self.verbose=: (untyped verbose) -> untyped
  def self.verbose?: -> bool
end
```

## Type accessor from default value

### update

```ruby
class A
  class_attribute :rate, default: 5
end
```

### result

```rbs
class A
  def rate: -> Integer
  def rate=: (Integer rate) -> Integer
  def rate?: -> bool
  def self.rate: -> Integer
  def self.rate=: (Integer rate) -> Integer
  def self.rate?: -> bool
end
```

## Suppress instance accessor when disabled

### update

```ruby
class B
  class_attribute :rate, instance_accessor: false
end
```

### result

```rbs
class B
  def self.rate: -> untyped
  def self.rate=: (untyped rate) -> untyped
  def self.rate?: -> bool
end
```

## Suppress only instance reader when disabled

### update

```ruby
class C
  class_attribute :rate, instance_reader: false
end
```

### result

```rbs
class C
  def rate=: (untyped rate) -> untyped
  def self.rate: -> untyped
  def self.rate=: (untyped rate) -> untyped
  def self.rate?: -> bool
end
```

## Suppress only instance writer when disabled

### update

```ruby
class D
  class_attribute :rate, instance_writer: false
end
```

### result

```rbs
class D
  def rate: -> untyped
  def rate?: -> bool
  def self.rate: -> untyped
  def self.rate=: (untyped rate) -> untyped
  def self.rate?: -> bool
end
```
