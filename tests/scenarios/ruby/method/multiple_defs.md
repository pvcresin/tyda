# Ruby / Method / Multiple Defs

## Multiple top-level methods

### update

```ruby
def foo = 1

def bar = "hello"
```

### result

```rbs
class Object
  def foo: -> 1
  def bar: -> "hello"
end
```
