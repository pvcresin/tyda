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
