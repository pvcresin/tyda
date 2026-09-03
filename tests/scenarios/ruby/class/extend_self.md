# Ruby / Class / Extend Self

## `extend M` exposes M instance methods as singleton methods

### update

```ruby
module M
  def greet = :hello
end

class A
  extend M
end

def f = A.greet
```

### result

```rbs
class A
  extend M
end

module M
  def greet: -> :hello
end

class Object < BasicObject
  def f: -> :hello
end
```

## `extend self` exposes module methods as singleton methods

### update

```ruby
module M
  extend self

  def greet = :hi
end

def f = M.greet
```

### result

```rbs
module M
  extend M

  def greet: -> :hi
end

class Object < BasicObject
  def f: -> :hi
end
```

## `module_function define_method` registers instance and singleton methods

### update

```ruby
module Helper
  module_function define_method(:label) { "label" }
end

def f = Helper.label
```

### result

```rbs
module Helper
  def label: -> "label"
  def self.label: -> "label"
end

class Object < BasicObject
  def f: -> "label"
end
```

## `module_function define_method` syncs block params

### update

```ruby
module Helper
  module_function define_method(:wrap) { |value| [value] }
end

def f = Helper.wrap("x")
```

### result

```rbs
module Helper
  def wrap: (untyped value) -> [untyped]
  def self.wrap: (String value) -> [String]
end

class Object < BasicObject
  def f: -> [String]
end
```

## `module_function` method is callable as singleton method

### update

```ruby
module M
  module_function

  def greet = :hi
end

def f = M.greet
```

### result

```rbs
module M
  def greet: -> :hi
  def self.greet: -> :hi
end

class Object < BasicObject
  def f: -> :hi
end
```

## `module_function def` exposes singleton helper

### update

```ruby
module Helper
  module_function def status = :ok
end

def f = Helper.status
```

### result

```rbs
module Helper
  def status: -> :ok
  def self.status: -> :ok
end

class Object < BasicObject
  def f: -> :ok
end
```

## `module_function alias_method` exposes alias helper

### update

```ruby
module Helper
  def source = "source"

  module_function alias_method(:target, :source)
end

def f = Helper.target
```

### result

```rbs
module Helper
  def source: -> "source"
  def self.target: -> "source"
  alias target source
end

class Object < BasicObject
  def f: -> "source"
end
```
