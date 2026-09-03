# Ruby / RBS Input / Hash Merge

## merge with different value types

### update

```ruby
def test_merge_diff
  h1 = { a: 1 }
  h2 = { b: "hello" }
  h1.merge(h2)
end
```

### result

```rbs
class Object < BasicObject
  def test_merge_diff: -> { a: 1, b: "hello" }
end
```

## merge with same value type

### update

```ruby
def test_merge_same
  h1 = { a: 1 }
  h2 = { b: 2 }
  h1.merge(h2)
end
```

### result

```rbs
class Object < BasicObject
  def test_merge_same: -> { a: 1, b: 2 }
end
```

## Hash#values returns Array[V]

### update

```ruby
def test_values = { a: 1, b: 2 }.values
```

### result

```rbs
class Object < BasicObject
  def test_values: -> Array[1 | 2]
end
```

## Hash#keys returns Array[K]

### update

```ruby
def test_keys = { a: 1, b: 2 }.keys
```

### result

```rbs
class Object < BasicObject
  def test_keys: -> Array[:a | :b]
end
```
