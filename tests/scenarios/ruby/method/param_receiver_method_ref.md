# Ruby / Method / Param Receiver Method Ref

## String param receiver upcase

### update

```ruby
class A
  def up(s) = s.upcase
end
A.new.up("x")
```

### result

```rbs
class A
  def up: (String s) -> String
end
```

## String param receiver strip chain

### update

```ruby
class A
  def strip_up(s) = s.strip.upcase
end
A.new.strip_up(" y ")
```

### result

```rbs
class A
  def strip_up: (String s) -> String
end
```

## Integer params operator plus

### update

```ruby
class A
  def add(a, b) = a + b
end
A.new.add(1, 2)
```

### result

```rbs
class A
  def add: (Integer a, Integer b) -> Integer
end
```

## Array element param first

### update

```ruby
class A
  def hd(arr) = arr.first
end
A.new.hd([1])
```

### result

```rbs
class A
  def hd: (Array[Integer] arr) -> Integer?
end
```

## Array param map block param

### update

```ruby
class A
  def mp(arr) = arr.map { |x| x }
end
A.new.mp([1])
```

### result

```rbs
class A
  def mp: (Array[Integer] arr) -> Array[Integer]
end
```

## Union param common method resolves

### update

```ruby
class A
  def s(v) = v.to_s
end
A.new.s(1)
A.new.s("x")
```

### result

```rbs
class A
  def s: ((Integer | String) v) -> String
end
```

## Typed param then block increments

### update

```ruby
class A
  def thn(x) = x.then { |y| y + 1 }
end
A.new.thn(3)
```

### result

```rbs
class A
  def thn: (Integer x) -> Integer
end
```

## Record param index access

### update

```ruby
class A
  def idx(h) = h[:key]
end
A.new.idx({ key: 5 })
```

### result

```rbs
class A
  def idx: ({ key: Integer } h) -> Integer
end
```

## Safe navigation on typed param

### update

```ruby
class A
  def nav(s) = s&.upcase
end
A.new.nav("z")
```

### result

```rbs
class A
  def nav: (String s) -> String
end
```

## Recursive param receiver does not diverge

### update

```ruby
class A
  def rec(n) = rec(n).succ
end
A.new.rec(1)
```

### result

```rbs
class A
  def rec: (Integer n) -> untyped
end
```
