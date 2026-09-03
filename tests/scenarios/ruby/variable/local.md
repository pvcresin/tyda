# Ruby / Variable / Local

## Assign variable and return it

### update

```ruby
def foo
  x = 1
  x
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> 1
end
```

## Last assignment wins

### update

```ruby
def foo
  x = 1
  x = "hello"
  x
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> "hello"
end
```

## Track multiple variables

### update

```ruby
def foo
  x = 1
  y = "hello"
  y
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> "hello"
end
```

## Assign method return to variable

### update

```ruby
def source = 42
def consumer
  x = source
  x
end
```

### result

```rbs
class Object < BasicObject
  def source: -> 42
  def consumer: -> 42
end
```
