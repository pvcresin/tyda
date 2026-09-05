# Ruby / Method / Recursion Safety

## Direct recursive method resolves to Integer without hanging

### update

```ruby
def fact(n)
  if n <= 1
    1
  else
    n * fact(n - 1)
  end
end

def f = fact(5)
```

### result

```rbs
class Object < BasicObject
  def fact: (Integer n) -> (Float | 1)
  def f: -> Float | 1
end
```

## Mutual recursive methods infer without hanging

### update

```ruby
def odd?(n)
  if n == 0
    false
  else
    even?(n - 1)
  end
end

def even?(n)
  if n == 0
    true
  else
    odd?(n - 1)
  end
end

def f = odd?(3)
```

### result

```rbs
class Object < BasicObject
  def odd?: (Integer n) -> bool
  def even?: (Integer n) -> bool
  def f: -> bool
end
```

## Unresolved receiver cycle remains bounded and untyped

### update

```ruby
class ReceiverA
  def call(other) = other.call
end

class ReceiverB
  def call(other) = other.call
end

def use_a = ReceiverA.new.call(ReceiverB.new)
def use_b = ReceiverB.new.call(ReceiverA.new)
```

### result

```rbs
class Object < BasicObject
  def use_a: -> untyped
  def use_b: -> untyped
end

class ReceiverA
  def call: (ReceiverB other) -> untyped
end

class ReceiverB
  def call: (ReceiverA other) -> untyped
end
```
