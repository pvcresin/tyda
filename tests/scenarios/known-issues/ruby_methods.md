# Known Issues / Ruby Methods

## An ivar set from a nil default parameter keeps nil

```ruby
class NilDefault
  def initialize(value = nil)
    @value = value
  end

  def value = @value
end

NilDefault.new
NilDefault.new("a")
```

### result

```rbs
class NilDefault
  def initialize: (?String? value) -> void
  def value: -> String?
end
```

## A yield inside an each block passes the element type

```ruby
class NameList
  def each_name(names)
    names.each { |name| yield name }
  end
end

NameList.new.each_name(["a"]) { |name| name.size }
```

### result

```rbs
class NameList
  def each_name: (Array[String] names) { (String) -> Integer } -> Array[String]
end
```

## A block param yielded from a later singleton method keeps the owner class

```ruby
class LaterSetup
  def self.setup
    yield instance
  end

  def self.instance = new
end

class SetupUser
  def run = LaterSetup.setup { |setup| setup }
end
```

### result

```rbs
class LaterSetup
  def self.setup: [R] { (LaterSetup) -> R } -> R
  def self.instance: -> LaterSetup
end

class SetupUser
  def run: -> LaterSetup
end
```
