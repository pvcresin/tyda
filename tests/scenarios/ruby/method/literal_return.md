# Ruby / Method / Literal Return

## Integer literal

### update

```ruby
def foo = 1
```

### result

```rbs
class Object < BasicObject
  def foo: -> 1
end
```

## String literal

### update

```ruby
def greet = "hello"
```

### result

```rbs
class Object < BasicObject
  def greet: -> "hello"
end
```

## Symbol literal

### update

```ruby
def status = :ok
```

### result

```rbs
class Object < BasicObject
  def status: -> :ok
end
```

## Return nil

### update

```ruby
def nothing = nil
```

### result

```rbs
class Object < BasicObject
  def nothing: -> nil
end
```

## Boolean literal

### update

```ruby
def enabled = true
```

### result

```rbs
class Object < BasicObject
  def enabled: -> true
end
```

## Float literal

### update

```ruby
def pi = 3.14
```

### result

```rbs
class Object < BasicObject
  def pi: -> 3.14
end
```

## Empty method definition

### update

```ruby
def noop
end
```

### result

```rbs
class Object < BasicObject
  def noop: -> nil
end
```
