# Ruby / Method / Multiple Defs

## Multiple top-level methods

### update

```ruby
def foo = 1

def bar = "hello"
```

### result

```rbs
class Object < BasicObject
  def foo: -> 1
  def bar: -> "hello"
end
```

## A later def in the same body replaces the earlier one

### update

```ruby
class Foo
  def run = 1
  def run = "x"
end

def call = Foo.new.run
```

### result

```rbs
class Foo
  def run: -> "x"
end

class Object < BasicObject
  def call: -> "x"
end
```
