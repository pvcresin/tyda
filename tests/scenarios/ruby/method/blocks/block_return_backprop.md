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
  def run: (Integer n) { (Float) -> "ok" } -> "ok"
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
  def run: (Integer n) { (Float) -> "ok" } -> "ok"
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
  def run: { (Integer) -> 1 } -> 1
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
  def run: { (Integer, String) -> [1, ""] } -> [1, ""]
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
  def run: (Integer n) { (Float) -> "ok" } -> "ok"
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
  def self.to_text: { () -> "ready"? } -> nil | untyped
  def self.from_nil: -> nil
  def self.from_string: -> String
end
```
