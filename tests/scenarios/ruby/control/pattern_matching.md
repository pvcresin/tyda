# Ruby / Control / Pattern Matching

## Apply narrowing from `in`

### update

```ruby
def foo(x)
  if x in String
    x
  else
    nil
  end
end
```

### result

```rbs
class Object
  def foo: (untyped x) -> String?
end
```

## Apply local binding from capture pattern

### update

```ruby
def foo(x)
  if x in Integer => y
    y
  end
end
```

### result

```rbs
class Object
  def foo: (untyped x) -> Integer?
end
```

## Treat `expr in Pattern` as bool

### update

```ruby
def foo(x)
  x in Integer
end

foo(1)
foo("x")
```

### result

```rbs
class Object
  def foo: ((Integer | String) x) -> bool
end
```

## Apply pin pattern

### update

```ruby
def foo(x)
  y = [1, 2]
  if x in ^y
    x
  end
end
```

### result

```rbs
class Object
  def foo: (untyped x) -> [1, 2]?
end
```

## Apply alternative pattern

### update

```ruby
def check(x)
  case x
  in 1 | 2
    :ok
  in 3 | 4 | 5
    :ok
  end
end
```

### result

```rbs
class Object
  def check: (untyped x) -> :ok?
end
```

## Apply range pattern

### update

```ruby
def check(x)
  case x
  in (0..)
    :ok1
  in -1
    :ok2
  end
end
```

### result

```rbs
class Object
  def check: (untyped x) -> (:ok1 | :ok2)?
end
```

## Keep type with right assignment pattern

### update

```ruby
def check(x)
  x => Integer
  x
end

check(1)
check("x")
```

### result

```rbs
class Object
  def check: ((Integer | String) x) -> (Integer | String)
end
```

## Use array pattern as condition

### update

```ruby
class A
end

def check(x)
  case x
  in 1, 2, 3
    :foo
  in [1, 2, 3, *]
    :bar
  in [String]
    :baz
  in A[1, 2, 3]
    :qux
  in [1,]
    :waldo
  else
    :zzz
  end
end

check([1].to_a)
```

### result

```rbs
class Object
  def check: (Array[Integer] x) -> (:bar | :baz | :foo | :qux | :waldo | :zzz)
end
```

## Use hash pattern as condition

### update

```ruby
class A
end

def check(x)
  case x
  in { a: Integer }
    :foo
  in { a: String, ** }
    :bar
  in { a: }
    :baz
  in A[a: Integer]
    :qux
  else
    :zzz
  end
end

check({ a: 42 })
```

### result

```rbs
class Object
  def check: ({ a: Integer } x) -> (:bar | :baz | :foo | :qux | :zzz)
end
```

## Use numeric literal pattern as condition

### update

```ruby
def check_numeric(x)
  case x
  in 1
    :int
  in 1.0
    :float
  in 1r
    :rational
  in 1i
    :complex
  else
    :zzz
  end
end

check_numeric(1)
```

### result

```rbs
class Object
  def check_numeric: (Integer x) -> (:complex | :float | :int | :rational | :zzz)
end
```

## Use string and symbol literal pattern as condition

### update

```ruby
def check_interpolation(x)
end

def check_text(x)
  case x
  in "foo"
    :string
  in "foo#{ check_interpolation(:ok_str) }"
    :interpolated_string
  in :foo
    :symbol
  in :"foo#{ check_interpolation(:ok_sym) }"
    :interpolated_symbol
  else
    :zzz
  end
end

check_text(:AAA)
```

### result

```rbs
class Object
  def check_interpolation: (untyped x) -> nil
  def check_text: (Symbol x) -> (:interpolated_string | :interpolated_symbol | :string | :symbol | :zzz)
end
```

## Use nil bool and special literal pattern as condition

### update

```ruby
def check_special(x)
  case x
  in nil
    :nil
  in false
    :false
  in true
    :false
  in __FILE__
    :file
  in __LINE__
    :line
  in __ENCODING__
    :encoding
  in %w[foo bar]
    :w_lit
  else
    :zzz
  end
end

check_special(nil)
```

### result

```rbs
class Object
  def check_special: (nil x) -> (:encoding | :false | :file | :line | :nil | :w_lit | :zzz)
end
```

## Branch with constant pattern

### update

```ruby
def check(x)
  case x
  in Integer
    :int
  in String
    :str
  end
end

check(1)
check("x")
```

### result

```rbs
class Object
  def check: ((Integer | String) x) -> (:int | :str)?
end
```

## Use find pattern as condition

### update

```ruby
def check(x)
  case x
  in *a, Integer, *b
    :foo
  in *a, String, *b
    :bar
  else
    :zzz
  end
end

check([1].to_a)
```

### result

```rbs
class Object
  def check: (Array[Integer] x) -> (:bar | :foo | :zzz)
end
```

## Apply pattern guard

### update

```ruby
def cond?(x) = x

def check(x)
  case x
  in 1 if cond?(:ok)
    :ok
  end
end
```

### result

```rbs
class Object
  def cond?: (Symbol x) -> Symbol
  def check: (untyped x) -> :ok?
end
```

## Keep variable pattern binding as untyped

### update

```ruby
def check(x)
  case x
  in y
    y
  end
end

check(1)
check("x")
```

### result

```rbs
class Object
  def check: ((Integer | String) x) -> (Integer | String)
end
```

## Keep array variable pattern binding as untyped

### update

```ruby
def check(x)
  case x
  in a, b, c, *rest
    [a, b, c, rest]
  end
end

check(1)
check("x")
```

### result

```rbs
class Object
  def check: ((Integer | String) x) -> [untyped, untyped, untyped, Array[untyped]]?
end
```

## Keep hash variable pattern binding as untyped

### update

```ruby
def check(x)
  case x
  in { a:, b:, c:, **rest }
    [a, b, c, rest]
  end
end

check(1)
check("x")
```

### result

```rbs
class Object
  def check: ((Integer | String) x) -> [untyped, untyped, untyped, Hash[untyped, untyped]]?
end
```

## Array variable pattern keeps tuple elements

### update

```ruby
def check(x)
  case x
  in a, b, c, *rest
    [a, b, c, rest]
  end
end

check([1, 2, 3, 4])
```

### result

```rbs
class Object
  def check: (Array[Integer] x) -> [Integer, Integer, Integer, Array[Integer]]?
end
```

## Hash variable pattern keeps record fields

### update

```ruby
def check(x)
  case x
  in { a:, b:, c:, **rest }
    [a, b, c, rest]
  end
end

check({ a: 1, b: 2, c: 3, d: 4 })
```

### result

```rbs
class Object
  def check: ({ a: Integer, b: Integer, c: Integer, d: Integer } x) -> [Integer, Integer, Integer, { d: Integer }]?
end
```

## Find pattern keeps before and after rest as arrays

### update

```ruby
def check(x)
  case x
  in *a, Integer, *b
    [a, b]
  end
end

check([1, 2, 3])
```

### result

```rbs
class Object
  def check: (Array[Integer] x) -> [Array[Integer], Array[Integer]]?
end
```

## Use local bound by one-line pattern matching later

### update

```ruby
def standalone_in_array
  value = [1, "x"]
  value in [a, b]
  b
end

def standalone_required_hash
  value = {name: "ruby", version: 3}
  value => {name:}
  name
end
```

### result

```rbs
class Object
  def standalone_in_array: -> "x"
  def standalone_required_hash: -> "ruby"
end
```

## Nested capture pattern binds the asserted type

### update

```ruby
class Decoder
  def point(v)
    case v
    in { x: Integer => x, y: Integer => y }
      x + y
    end
  end

  def pair(v)
    case v
    in [String => name, Integer => age]
      [name, age]
    end
  end
end
```

### result

```rbs
class Decoder
  def point: (untyped v) -> Integer?
  def pair: (untyped v) -> [String, Integer]?
end
```

## Pattern capture uses the binding after match

### update

```ruby
def check(a)
  case a
  in Integer => n
    n + 1
  in String => s
    s.upcase
  end
end

check(1)
check("foo")
```

### result

```rbs
class Object
  def check: ((Integer | String) a) -> (Integer | String)?
end
```
