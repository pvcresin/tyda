# Ruby / Class / Singleton Universal Tail

## Class#allocate on user class

### update

```ruby
class Foo
end

def make = Foo.allocate
```

### result

```rbs
class Object
  def make: -> untyped
end
```

## Instance attr_reader is not shadowed by Module#name

### update

```ruby
class Profile
  attr_reader :name
  def initialize
    @name = "Alice"
  end
end

def call = Profile.new.name
```

### result

```rbs
class Object
  def call: -> "Alice"
end

class Profile
  def name: -> "Alice"
  def initialize: -> void
end
```

## Resolve Module#define_method through singleton fallback

### update

```ruby
class Foo
end

def probe = Foo.define_method(:bar) { 1 }
```

### result

```rbs
class Object
  def probe: -> Symbol
end
```
