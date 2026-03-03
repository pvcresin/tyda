# Ruby / Method / Endless

## Endless method without args

### update

```ruby
def answer = 42
```

### result

```rbs
class Object
  def answer: -> 42
end
```

## Endless method with args

### update

```ruby
def greet(name) = "Hello, #{name}!"

greet("World")
```

### result

```rbs
class Object
  def greet: (String name) -> String
end
```

## Endless method inside class

### update

```ruby
class Calc
  def pi = 3.14
end
```

### result

```rbs
class Calc
  def pi: -> 3.14
end
```
