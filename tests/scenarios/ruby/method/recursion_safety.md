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
  def fact: (Integer n) -> Integer
  def f: -> Integer
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

## Return-position cycles add nothing to the joined return type

### update

```ruby
class ReturnCycles
  def self_loop(flag)
    flag ? 1 : self_loop(flag)
  end

  def first(flag)
    flag ? 1 : second(flag)
  end

  def second(flag)
    flag ? "s" : third(flag)
  end

  def third(flag)
    flag ? :s : first(flag)
  end

  def nested(flag)
    flag ? [1] : [nested(flag)]
  end
end
```

### result

```rbs
class ReturnCycles
  def self_loop: (untyped flag) -> 1
  def first: (untyped flag) -> (1 | "s" | :s)
  def second: (untyped flag) -> (1 | "s" | :s)
  def third: (untyped flag) -> (1 | "s" | :s)
  def nested: (untyped flag) -> ([untyped] | [1])
end
```

## Direct recursive instance method resolves to Integer

### update

```ruby
class Calculator
  def fact(n)
    if n <= 1
      1
    else
      n * fact(n - 1)
    end
  end

  def run = fact(5)
end
```

### result

```rbs
class Calculator
  def fact: (Integer n) -> Integer
  def run: -> Integer
end
```
