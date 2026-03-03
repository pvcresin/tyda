# Ruby / RBS Input / Nil Methods

## nil.to_s

### update

```ruby
def test_nil_to_s = nil.to_s
```

### result

```rbs
class Object
  def test_nil_to_s: -> ""
end
```

## nil.to_i

### update

```ruby
def test_nil_to_i = nil.to_i
```

### result

```rbs
class Object
  def test_nil_to_i: -> 0
end
```

## nil.to_f

### update

```ruby
def test_nil_to_f = nil.to_f
```

### result

```rbs
class Object
  def test_nil_to_f: -> Float
end
```
