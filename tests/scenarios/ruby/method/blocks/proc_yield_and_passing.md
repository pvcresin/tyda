# Ruby / Method / Blocks / Proc Yield And Passing

## then chaining with arithmetic

### update

```ruby
def test_then_chain
  "42".then { |s| s.to_i }.then { |n| n * 2 }
end
```

### result

```rbs
class Object
  def test_then_chain: -> Integer
end
```

## Lambda with params and call

### update

```ruby
def test_lambda_param_call
  f = -> (x) { x + 1 }
  f.call(10)
end
```

### result

```rbs
class Object
  def test_lambda_param_call: -> Integer
end
```

## Proc.new with params and call

### update

```ruby
def test_proc_new_param_call
  p = Proc.new { |x| x.to_s }
  p.call(42)
end
```

### result

```rbs
class Object
  def test_proc_new_param_call: -> String
end
```

## Stored lambda with call

### update

```ruby
def test_stored_lambda_call
  doubler = -> (n) { n * 2 }
  result = doubler.call(5)
  result
end
```

### result

```rbs
class Object
  def test_stored_lambda_call: -> Integer
end
```

## yield returns untyped

### update

```ruby
def test_yield_returns_untyped = yield 1
```

### result

```rbs
class Object
  def test_yield_returns_untyped: { (Integer) -> untyped } -> untyped
end
```

## yield with subsequent return value

### update

```ruby
def test_yield_then_value
  yield 1
  42
end
```

### result

```rbs
class Object
  def test_yield_then_value: { (Integer) -> untyped } -> 42
end
```

## &block parameter with call

### update

```ruby
def test_block_param_call(&block) = block.call(42)
```

### result

```rbs
class Object
  def test_block_param_call: { (Integer) -> untyped } -> untyped
end
```

## &block call uses call-site return

### update

```ruby
def apply_value(&block) = block.call(1)

def use_apply_value = apply_value { |value| value.to_s }
```

### result

```rbs
class Object
  def apply_value: { (Integer) -> String } -> String
  def use_apply_value: -> String
end
```

## &block call keeps multiple parameters

### update

```ruby
def build_pair(&block) = block.call("a", 1)

def use_build_pair = build_pair { |name, count| [name, count] }
```

### result

```rbs
class Object
  def build_pair: { (String, Integer) -> ["a", 1] } -> ["a", 1]
  def use_build_pair: -> ["a", 1]
end
```

## &block aliases call

### update

```ruby
def apply_index(&block) = block[1]

def use_apply_index = apply_index { |value| value.to_s }

def apply_yield(&block) = block.yield(:ok)

def use_apply_yield = apply_yield { |value| value.to_s }
```

### result

```rbs
class Object
  def apply_index: { (Integer) -> String } -> String
  def use_apply_index: -> String
  def apply_yield: { (Symbol) -> String } -> String
  def use_apply_yield: -> String
end
```

## &block call merges branch arguments

### update

```ruby
def choose_value(flag, &block)
  if flag
    block.call(:left)
  else
    block.call(:right)
  end
end

def use_choose_value = choose_value(true) { |value| value.to_s }
```

### result

```rbs
class Object
  def choose_value: (bool flag) { (Symbol) -> String } -> String
  def use_choose_value: -> String
end
```

## Pass Proc with block variable

### update

```ruby
def test_proc_variable_pass
  transformer = -> (x) { x.to_s }
  [1, 2, 3].map(&transformer)
end
```

### result

```rbs
class Object
  def test_proc_variable_pass: -> Array[String]
end
```

## reduce without initial value

### update

```ruby
def test_reduce_no_init = [1, 2, 3].reduce { |sum, x| sum + x }
```

### result

```rbs
class Object
  def test_reduce_no_init: -> Integer
end
```

## Pass method reference with &method(:name)

### update

```ruby
def test_method_ref = [1, 2, 3].map(&method(:to_s))
```

### result

```rbs
class Object
  def test_method_ref: -> Array[String]
end
```

## Method object call resolves project method

### update

```ruby
class Worker
  def normalize(value) = value.to_s
  def identity(value) = value
  def direct = method(:normalize).call(1)
  def via_proc = method(:normalize).to_proc.call(:name)
  def passthrough = method(:identity).call(:ok)
end
```

### result

```rbs
class Worker
  def normalize: (untyped value) -> String
  def identity: (untyped value) -> untyped
  def direct: -> String
  def via_proc: -> String
  def passthrough: -> :ok
end
```

## Method object as enumerable block

### update

```ruby
class Worker
  def normalize(value) = value.to_s
  def build(value) = [value, value.to_s]
  def values = [1, "x"].map(&method(:normalize))
  def pairs = [1, nil].filter_map(&method(:build))
end
```

### result

```rbs
class Worker
  def normalize: (untyped value) -> String
  def build: (untyped value) -> [untyped, String]
  def values: -> Array[String]
  def pairs: -> Array[[1?, String]]
end
```

## Explicit receiver method object

### update

```ruby
class Formatter
  def self.render(value) = value.to_s
  def label(value) = value.to_s
end

class Caller
  def class_values = [1, :x].map(&Formatter.method(:render))

  def instance_value
    formatter = Formatter.new
    formatter.public_method(:label).call(:ok)
  end
end
```

### result

```rbs
class Caller
  def class_values: -> Array[String]
  def instance_value: -> String
end

class Formatter
  def self.render: (untyped value) -> String
  def label: (untyped value) -> String
end
```

## Method object block destructures hash entries

### update

```ruby
class Builder
  def join_pair(key, value) = "#{key}:#{value}"
  def rows = { a: 1, b: 2 }.map(&method(:join_pair))
end
```

### result

```rbs
class Builder
  def join_pair: (untyped key, untyped value) -> String
  def rows: -> Array[String]
end
```

## then/yield_self method block receives receiver

### update

```ruby
class Worker
  def identity(value) = value

  def via_symbol = [1, 2].then(&:itself)
  def via_method = [1, 2].yield_self(&method(:identity))

  def via_local
    block = method(:identity)
    { name: "item" }.then(&block)
  end
end

class Converter
  def self.wrap(value) = [value, value]
end

class Caller
  def converted = :ok.yield_self(&Converter.method(:wrap))
end
```

### result

```rbs
class Caller
  def converted: -> [:ok, :ok]
end

class Converter
  def self.wrap: (untyped value) -> [untyped, untyped]
end

class Worker
  def identity: (untyped value) -> untyped
  def via_symbol: -> [1, 2]
  def via_method: -> [1, 2]
  def via_local: -> { name: "item" }
end
```
