# Ruby / Method / Array set operations

## Static `&`, `|`, `-` on tuple literals refine elements statically

### update

```ruby
class Probe
  def intersection = [1, 2, 3] & [2, 3, 4]
  def union        = [1, 2] | [2, 3]
  def difference   = [1, 2, 3] - [2]
end
```

### result

```rbs
class Probe
  def intersection: -> Array[2 | 3]
  def union: -> Array[1 | 2 | 3]
  def difference: -> Array[1 | 3]
end
```

## Static `+`, `*` on tuple literals refine elements statically

### update

```ruby
class Probe
  def concat     = [1] + [2, 3]
  def repeat_int = [1, 2] * 3
  def join_str   = [1, 2, 3] * ","
end
```

### result

```rbs
class Probe
  def concat: -> Array[1 | 2 | 3]
  def repeat_int: -> Array[1 | 2]
  def join_str: -> "1,2,3"
end
```
