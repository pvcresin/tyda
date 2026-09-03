# Ruby / Class / Basic

## Method definition inside class

### update

```ruby
class A
  def bar = 42
end
```

### result

```rbs
class A
  def bar: -> 42
end
```

## Multiple methods inside class

### update

```ruby
class A
  def name = "x"

  def age = 30
end
```

### result

```rbs
class A
  def name: -> "x"
  def age: -> 30
end
```

## Resolve self.class::CONST inside instance method

### update

```ruby
class Foo
  VERSION = "1.0"

  def a = self.class::VERSION
end
```

### result

```rbs
class Foo
  VERSION: "1.0"

  def a: -> "1.0"
end
```

## Preserve Object's implicit superclass

### update

```ruby
def answer = 42
```

### result

```rbs
class Object < BasicObject
  def answer: -> 42
end
```

## Preserve an explicit Object superclass

### update

```ruby
class Object < CustomBase
  def answer = 42
end
```

### result

```rbs
class Object < CustomBase
  def answer: -> 42
end
```
