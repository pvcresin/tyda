# Ruby / Method / Arguments

## Apply anonymous rest parameter to signature

### update

```ruby
def capture(*) = 1
```

### result

```rbs
class Object
  def capture: (*untyped) -> 1
end
```

## Apply anonymous block parameter to signature

### update

```ruby
def wrap(*, **, &) = nil
```

### result

```rbs
class Object
  def wrap: (*untyped, **untyped, ?untyped &block) -> nil
end
```

## Underscore method names without parentheses

### update

```ruby
def _(x) = x
def __ = :arg
def ___(x) = x

def underscore_arg = _ __
def underscore_nested = ___ _(__)
```

### result

```rbs
class Object
  def _: (Symbol x) -> Symbol
  def __: -> :arg
  def ___: (untyped x) -> Symbol
  def underscore_arg: -> Symbol
  def underscore_nested: -> Symbol
end
```

## Infer arg type from one call

### update

```ruby
def foo(x) = x
foo(1)
```

### result

```rbs
class Object
  def foo: (Integer x) -> Integer
end
```

## Infer union type from multiple calls

### update

```ruby
def foo(x) = x
foo(1)
foo("hello")
```

### result

```rbs
class Object
  def foo: ((Integer | String) x) -> (Integer | String)
end
```

## Infer multiple arg types

### update

```ruby
def add(a, b) = a
add(1, "hello")
```

### result

```rbs
class Object
  def add: (Integer a, String b) -> Integer
end
```

## Use untyped when there is no call

### update

```ruby
def foo(x) = x
```

### result

```rbs
class Object
  def foo: (untyped x) -> untyped
end
```

## Infer arg type from class method call

### update

```ruby
def foo(n) = n

class Foo
  def bar
    foo(1)
  end
end
```

### result

```rbs
class Foo
  def bar: -> Integer
end

class Object
  def foo: (Integer n) -> Integer
end
```

## Arg type for empty method

### update

```ruby
def foo(x)
end

foo(1)
```

### result

```rbs
class Object
  def foo: (Integer x) -> nil
end
```

## Infer arg types for multiple methods

### update

```ruby
def add(a, b) = a

def sub(a, b) = b

add(1, 2)
sub("x", "y")
```

### result

```rbs
class Object
  def add: (Integer a, Integer b) -> Integer
  def sub: (String a, String b) -> String
end
```

## Infer keyword argument as positional hash

### update

```ruby
def foo(x) = x

foo(a: 1)
```

### result

```rbs
class Object
  def foo: ({ a: Integer } x) -> { a: Integer }
end
```

## Optional default can reference previous parameter

### update

```ruby
def foo(a, b, x = 1, y = x)
end

foo(:a, :b, :x)
```

### result

```rbs
class Object
  def foo: (Symbol a, Symbol b, ?(Integer | Symbol) x, ?(Integer | Symbol) y) -> nil
end
```

## Merge multiple call sites into rest parameter

### update

```ruby
def foo(a, b, *rest) = rest

foo(:a, :b, :x)
foo(:a, :b, :y, :z)
```

### result

```rbs
class Object
  def foo: (Symbol a, Symbol b, *Symbol rest) -> Array[Symbol]
end
```

## Keep required parameter after rest separate

### update

```ruby
def foo(a, b, *rest, x, y)
end

foo(:a, :b, :r1, :r2, :r3, :x, :y)
```

### result

```rbs
class Object
  def foo: (Symbol a, Symbol b, *Symbol rest, Symbol x, Symbol y) -> nil
end
```

## Destructure required positional parameter

### update

```ruby
def check(z, (x, y))
  [z, y, x]
end

check(1, [1, "str"])
```

### result

```rbs
class Object
  def check: (Integer z, [Integer, String]) -> [Integer, "str", 1]
end
```

## Destructure binds inner names in order

### update

```ruby
def two(z, (x, y))
  [x, y]
end

two(1, [1, 2.0])
```

### result

```rbs
class Object
  def two: (Integer z, [Integer, Float]) -> [1, 2.0]
end
```

## Splat of a known tuple distributes to positionals

### update

```ruby
def take(a, b, c)
  [a, b, c]
end

take(*[1, "s"], :x)
```

### result

```rbs
class Object
  def take: (Integer a, String b, Symbol c) -> [Integer, String, Symbol]
end
```

## Heterogeneous array argument keeps tuple shape

### update

```ruby
def pair(a) = a

pair([1, "str"])
```

### result

```rbs
class Object
  def pair: ([Integer, String] a) -> [Integer, String]
end
```

## Same-length array arguments merge element-wise

### update

```ruby
def pick(a) = a

pick([1, "x"])
pick([2, :y])
```

### result

```rbs
class Object
  def pick: ([Integer, String | Symbol] a) -> [Integer, String | Symbol]
end
```

## Homogeneous array argument widens to Array

### update

```ruby
def nums(a) = a

nums([1, 2, 3])
```

### result

```rbs
class Object
  def nums: (Array[Integer] a) -> Array[Integer]
end
```

## Different-length array arguments fall back to Array

### update

```ruby
def vals(a) = a

vals([1, "str"])
vals([1, 2, 3])
```

### result

```rbs
class Object
  def vals: (Array[Integer | String] a) -> Array[Integer | String]
end
```

## Nested destructured parameters bind the last name

### update

```ruby
def combine(_prefix, (_a, (_b, _c)), (_d, *_e, f)) = f
combine(:p, [1, [2, 3]], [4, 5, 6])
```

### result

```rbs
class Object
  def combine: (Symbol _prefix, [Integer, Array[Integer]], Array[Integer]) -> 6
end
```
