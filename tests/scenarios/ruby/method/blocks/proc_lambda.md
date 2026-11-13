# Ruby / Method / Blocks / Proc Lambda

## Proc.new and call

### update

```ruby
def test_proc_new_call
  p = Proc.new { 42 }
  p.call
end
```

### result

```rbs
class Object < BasicObject
  def test_proc_new_call: -> 42
end
```

## Lambda and call

### update

```ruby
def test_lambda_call
  f = -> { "hello" }
  f.call
end
```

### result

```rbs
class Object < BasicObject
  def test_lambda_call: -> "hello"
end
```

## lambda `.()` call

### update

```ruby
def test_lambda_dot_call
  f = -> () { 1 }
  f.()
end
```

### result

```rbs
class Object < BasicObject
  def test_lambda_dot_call: -> 1
end
```

## Block forwarding in user-defined method

### update

```ruby
def foo = yield 42

def proxy(&blk) = foo(&blk)

def bar
  ret = nil
  foo do |x|
    ret = x
  end
  ret
end
```

### result

```rbs
class Object < BasicObject
  def foo: { (Integer) -> 42 } -> 42
  def proxy: { (Integer) -> 42 } -> 42
  def bar: -> 42
end
```

## Block forwarding with positional arg in user-defined method

### update

```ruby
def foo(x) = yield 42

def proxy(&blk) = foo(1, &blk)

def bar
  ret = nil
  foo(1) do |x|
    ret = x
  end
  ret
end
```

### result

```rbs
class Object < BasicObject
  def foo: (Integer x) { (Integer) -> 42 } -> 42
  def proxy: { (Integer) -> 42 } -> 42
  def bar: -> 42
end
```

## Proc curry and arity

### update

```ruby
class Calculator
  def adder         = ->(x, y) { x + y }.curry
  def doubler_arity = ->(x) { x * 2 }.arity
  def staged        = ->(x) { x.to_s }.curry.call(5)
end
```

### result

```rbs
class Calculator
  def adder: -> Proc
  def doubler_arity: -> Integer
  def staged: -> String
end
```

## Empty lambda parameter does not change an outer local

### update

```ruby
class Runner
  def run
    value = 1
    callback = ->(value) { }
    callback.call("text")
    value
  end
end
```

### result

```rbs
class Runner
  def run: -> 1
end
```

## Lambda optional parameter uses its default value

### update

```ruby
class Runner
  def run
    callback = ->(first, second = 2) { second }
    callback.call(1)
  end
end
```

### result

```rbs
class Runner
  def run: -> 2
end
```

## Lambda rest parameter collects the call arguments

### update

```ruby
class Runner
  def run
    callback = ->(*items) { items }
    callback.call(1, "text")
  end
end
```

### result

```rbs
class Runner
  def run: -> [1, "text"]
end
```

## Lambda post parameter takes the last call argument

### update

```ruby
class Runner
  def run
    callback = ->(first, *middle, last) { last }
    callback.call(1, 2, :done)
  end
end
```

### result

```rbs
class Runner
  def run: -> :done
end
```

## Lambda keyword parameter takes the call keyword or its default

### update

```ruby
class Runner
  def passed
    callback = ->(key: 1) { key }
    callback.call(key: "text")
  end

  def defaulted
    callback = ->(key: 1) { key }
    callback.call
  end
end
```

### result

```rbs
class Runner
  def passed: -> "text"
  def defaulted: -> 1
end
```

## Lambda block parameter receives the call block

### update

```ruby
class Runner
  def run
    callback = ->(&block) { block }
    callback.call { 1 }
  end
end
```

### result

```rbs
class Runner
  def run: -> (^() -> 1)
end
```

## Lambda positional parameter receives keyword arguments as a hash

### update

```ruby
class Runner
  def run
    callback = ->(options) { options }
    callback.call(key: 1)
  end
end
```

### result

```rbs
class Runner
  def run: -> { key: 1 }
end
```

## Proc spreads a single array argument over its parameters

### update

```ruby
class Runner
  def run
    callback = proc { |first, second| second }
    callback.call([1, "text"])
  end
end
```

### result

```rbs
class Runner
  def run: -> "text"
end
```

## return inside a lambda leaves only the lambda

### update

```ruby
class Runner
  def run
    callback = -> { return 1 }
    callback.call
  end
end
```

### result

```rbs
class Runner
  def run: -> 1
end
```

## break inside a lambda leaves only the lambda

### update

```ruby
class Runner
  def run
    callback = -> { break "text" }
    callback.call
  end
end
```

### result

```rbs
class Runner
  def run: -> "text"
end
```

## Proc keeps a single array argument with one positional parameter

### update

```ruby
class Runner
  def with_block = proc { |first, &block| first }.call([1, 2])
  def with_keyword = proc { |first, key: 1| first }.call([1, 2])
  def with_default = proc { |first = 5| first }.call([1, 2])
  def trailing_comma = proc { |first, | first }.call([1, 2])
end
```

### result

```rbs
class Runner
  def with_block: -> [1, 2]
  def with_keyword: -> [1, 2]
  def with_default: -> [1, 2]
  def trailing_comma: -> 1
end
```

## Lambda rest parameter receives keyword arguments as a hash

### update

```ruby
class Runner
  def run = ->(*items) { items }.call(1, key: 2)
end
```

### result

```rbs
class Runner
  def run: -> [1, { key: 2 }]
end
```

## Lambda and proc literals render as proc types

### update

```ruby
class Maker
  def make_lambda = ->(n) { "s" }
  def make_proc = proc { |n| "s" }
end
```

### result

```rbs
class Maker
  def make_lambda: -> (^(?) -> "s")
  def make_proc: -> (^(?) -> "s")
end
```
