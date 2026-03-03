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
class Object
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
class Object
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
class Object
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
class Object
  def foo: { (Integer) -> 42 } -> 42
  def proxy: { (Integer) -> 42 } -> 42
  def bar: -> 42?
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
class Object
  def foo: (Integer x) { (Integer) -> 42 } -> 42
  def proxy: { (Integer) -> 42 } -> 42
  def bar: -> 42?
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
