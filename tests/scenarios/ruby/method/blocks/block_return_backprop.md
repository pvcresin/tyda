# Ruby / Method / Blocks / Block Return Back-Propagation

## yield result becomes block sig return and method return

### update

```ruby
class C
  def run(n)
    yield 1.0
  end
end

C.new.run(12) { |x| "ok" }
```

### result

```rbs
class C
  def run: [R] (Integer n) { (Float) -> R } -> R
end
```

## block.call result back-propagates from the call-site block body

### update

```ruby
class C
  def run(n, &b)
    b.call(1.0)
  end
end

C.new.run(12) { |x| "ok" }
```

### result

```rbs
class C
  def run: [R] (Integer n) { (Float) -> R } -> R
end
```

## it parameter block return back-propagates

### update

```ruby
class C
  def run(&b)
    b.call(1)
  end
end

C.new.run { it }
```

### result

```rbs
class C
  def run: [R] { (Integer) -> R } -> R
end
```

## numbered parameter block return back-propagates

### update

```ruby
class C
  def run(&b)
    b.call(1, "")
  end
end

C.new.run { [_1, _2] }
```

### result

```rbs
class C
  def run: [R] { (Integer, String) -> R } -> R
end
```

## bare yield call back-propagates the block body return

### update

```ruby
def run(n)
  yield 1.0
end

run(12) { |x| "ok" }
```

### result

```rbs
class Object < BasicObject
  def run: [R] (Integer n) { (Float) -> R } -> R
end
```

## safe navigation waits for a yielded return type

### update

```ruby
class C
  def self.to_text
    yield&.to_s
  end

  def self.from_nil = to_text { nil }
  def self.from_string = to_text { "ready" }
end
```

### result

```rbs
class C
  def self.to_text: { () -> "ready"? } -> untyped
  def self.from_nil: -> nil
  def self.from_string: -> String
end
```

## Block passed to new records its return in initialize

```yaml
known_issue: true
```

### update

```ruby
class Foo
  def initialize(n)
    yield "x"
  end
end

class Builder
  def build = Foo.new(1) { |x| x.size }
end
```

### result

```rbs
class Builder
  def build: -> Foo
end

class Foo
  def initialize: (Integer n) { (String) -> Integer } -> void
end
```
