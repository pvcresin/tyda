# Ruby / Class / Open Class

## Reopen class

### update

```ruby
class Dog
  def speak = "woof"
end

class Dog
  def fetch = "ball"
end
```

### result

```rbs
class Dog
  def speak: -> "woof"
  def fetch: -> "ball"
end
```

## Add method to standard library class

### update

```ruby
class String
  def shout = upcase
end
```

### result

```rbs
class String
  def shout: -> String
end
```

## Reopened class calls a method defined in another file

### update

### file: lib/foo.rb

```ruby
class Foo
  def base = 1
end
```

```ruby
class Foo
  def doubled = base
end
```

### result

```rbs
class Foo
  def base: -> 1
  def doubled: -> 1
end
```

## A later def in a reopened class replaces the earlier one

### update

```ruby
class Foo
  def run = 1
end

class Foo
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
