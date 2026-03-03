# Ruby / RBS Input / Recursive Alias

## Recursive alias widens nested elements conservatively

### update

```rbs
type node = Integer | Array[node]
```

```ruby
#: (node) -> node
def identity(value) = value
```

### result

```rbs
class Object
  def identity: ((Integer | Array[Integer | Array[node]]) value) -> (Integer | Array[Integer | Array[node]])
end
```
