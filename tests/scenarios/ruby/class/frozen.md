# Ruby / Class / Frozen

## freeze method

### update

```ruby
def frozen_str
  str = "hello".freeze
  str
end
```

### result

```rbs
class Object < BasicObject
  def frozen_str: -> "hello"
end
```

## freeze collection

### update

```ruby
def freeze_test
  arr = [1, 2, 3].freeze
  hash = { a: 1 }.freeze
  42
end
```

### result

```rbs
class Object < BasicObject
  def freeze_test: -> 42
end
```
