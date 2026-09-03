# Ruby / Control / For

## for loop over array

### update

```ruby
def for_test
  for x in [1, 2, 3]
    x
  end
end
```

### result

```rbs
class Object < BasicObject
  def for_test: -> [1, 2, 3]
end
```

## for loop over hash

### update

```ruby
def for_hash
  for k, v in { a: 1, b: 2 }
    k
  end
end
```

### result

```rbs
class Object < BasicObject
  def for_hash: -> { a: 1, b: 2 }
end
```

## for loop destructuring

### update

```ruby
def for_destructuring
  for key, value in [[:a, 1], [:b, 2]]
    key
  end
end
```

### result

```rbs
class Object < BasicObject
  def for_destructuring: -> [[:a, 1], [:b, 2]]
end
```

## for over non-empty fixed tuple keeps loop variable non-nil

### update

```ruby
def for_index_after_nonempty
  for x in [1, 2, 3]
  end
  x
end
```

### result

```rbs
class Object < BasicObject
  def for_index_after_nonempty: -> 1 | 2 | 3
end
```

## for over empty fixed tuple keeps loop variable nil

### update

```ruby
def for_index_after_empty
  for x in []
  end
  x
end
```

### result

```rbs
class Object < BasicObject
  def for_index_after_empty: -> nil
end
```
