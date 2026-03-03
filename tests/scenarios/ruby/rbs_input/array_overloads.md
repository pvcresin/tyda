# Ruby / RBS Input / Array Overloads

## first(n) on static Tuple returns head shape

### update

```ruby
def test_first_n = [1, 2, 3].first(2)
```

### result

```rbs
class Object
  def test_first_n: -> [1, 2]
end
```

## last(n) on static Tuple returns tail shape

### update

```ruby
def test_last_n = [1, 2, 3].last(2)
```

### result

```rbs
class Object
  def test_last_n: -> [2, 3]
end
```

## first on static Tuple returns first element

### update

```ruby
def test_first = [1, 2, 3].first
```

### result

```rbs
class Object
  def test_first: -> 1
end
```

## last on static Tuple returns last element

### update

```ruby
def test_last = ["a", "b"].last
```

### result

```rbs
class Object
  def test_last: -> "b"
end
```

## take and drop on static Tuple return slice shape

### update

```ruby
def test_take = [1, 2, 3, 4].take(2)

def test_drop = [1, 2, 3, 4].drop(2)
```

### result

```rbs
class Object
  def test_take: -> [1, 2]
  def test_drop: -> [3, 4]
end
```
