# Ruby / Class / Module

## Method definition inside module

### update

```ruby
module Helper
  def greet = "hello"

  def count = 42
end
```

### result

```rbs
module Helper
  def greet: -> "hello"
  def count: -> 42
end
```

## Connect extend self helper to singleton call

### update

`lib/helper.rb`

```ruby
module Helper
  extend self

  def greet = "hello"
end
```

```ruby
class Use
  def call = Helper.greet
end
```

### result

```rbs
class Use
  def call: -> "hello"
end
```

## module_function splatted names expose helpers

### update

```ruby
module Helper
  NAMES = %i[label size]

  def label = "label"
  def size = 1

  module_function *NAMES
end

class Use
  def label = Helper.label
  def size = Helper.size
end
```

### result

```rbs
module Helper
  NAMES: [:label, :size]

  def label: -> "label"
  def size: -> 1
  def self.label: -> "label"
  def self.size: -> 1
end

class Use
  def label: -> "label"
  def size: -> 1
end
```

## module_function exposes later def as module helper

### update

```ruby
module Helper
  module_function

  def greet = "hello"
end

class Use
  def call = Helper.greet
end
```

### result

```rbs
module Helper
  def greet: -> "hello"
  def self.greet: -> "hello"
end

class Use
  def call: -> "hello"
end
```

## module_function name exposes existing method as module helper

### update

```ruby
module Helper
  def greet = "hello"
  module_function :greet
end

class Use
  def call = Helper.greet
end
```

### result

```rbs
module Helper
  def greet: -> "hello"
  def self.greet: -> "hello"
end

class Use
  def call: -> "hello"
end
```
