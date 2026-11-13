# Ruby / Control / Operator Write

## Arithmetic operator assignment

### update

```ruby
def minus_eq
  x = 10
  x -= 1
  x
end

def mul_eq
  x = 10
  x *= 2
  x
end

def mod_eq
  x = 10
  x %= 3
  x
end

def div_eq
  x = 10
  x /= 2
  x
end

def pow_eq
  x = 10
  x **= 2
  x
end
```

### result

```rbs
class Object < BasicObject
  def minus_eq: -> Integer
  def mul_eq: -> Integer
  def mod_eq: -> Integer
  def div_eq: -> Integer
  def pow_eq: -> Numeric
end
```

## Bitwise operator assignment

### update

```ruby
def bit_ops
  flags = 1
  flags |= 2
  flags &= 3
  flags ^= 4
  flags <<= 1
  flags >>= 1
  flags
end
```

### result

```rbs
class Object < BasicObject
  def bit_ops: -> Integer
end
```

## String += widens to String

### update

```ruby
def append_text
  text = "a"
  text += "b"
  text
end
```

### result

```rbs
class Object < BasicObject
  def append_text: -> String
end
```

## Operator assignment to instance variable

### update

```ruby
class Counter
  def initialize
    @count = 0
  end

  def bump
    @count += 1
    @count
  end
end
```

### result

```rbs
class Counter
  def initialize: -> void
  def bump: -> Integer
end
```

## Chained compound assignment widens through plus and minus

### update

```ruby
def folded
  n = 10
  n += 5
  n -= 3
  n
end
```

### result

```rbs
class Object < BasicObject
  def folded: -> Integer
end
```

## Index operator assignment on a typed Hash

### update

```rbs
class Tally
  def counts: -> Hash[Symbol, Integer]
end
```

```ruby
class Tally
  def ensure_key
    counts[:a] ||= 0
  end

  def bump
    counts[:a] += 1
  end
end
```

### result

```rbs
class Tally
  def ensure_key: -> Integer
  def bump: -> Integer
end
```

## Attribute operator writes pass the new value to the writer

### update

```ruby
class Tally
  def count = 0

  def count=(value)
    value
  end

  def label = :none

  def label=(value)
    value
  end

  def bump
    self.count += 1
  end

  def relabel
    self.label &&= :done
  end
end
```

### result

```rbs
class Tally
  def count: -> 0
  def count=: [V] (V value) -> V
  def label: -> :none
  def label=: [V] (V value) -> V
  def bump: -> Integer
  def relabel: -> :done
end
```
