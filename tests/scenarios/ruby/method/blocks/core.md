# Ruby / Method / Blocks / Core

## each block

### update

```ruby
def with_block = [1, 2, 3].each { |x| x }
```

### result

```rbs
class Object
  def with_block: -> [1, 2, 3]
end
```

## map block

### update

```ruby
def with_map = [1, 2, 3].map { |x| x.to_s }
```

### result

```rbs
class Object
  def with_map: -> Array[String]
end
```

## select block

### update

```ruby
def with_select = [1, 2, 3].select { |x| x > 1 }
```

### result

```rbs
class Object
  def with_select: -> Array[1 | 2 | 3]
end
```

## Block arg

### update

```ruby
def with_block_param(&block) = 42
```

### result

```rbs
class Object
  def with_block_param: (?untyped &block) -> 42
end
```

## Block arg rest and post destructuring

### update

```ruby
def block_rest_post
  [[1, "x", :y]].map do |a, *b, c|
    [a, b, c]
  end
end
```

### result

```rbs
class Object
  def block_rest_post: -> Array[[1, ["x"], :y]]
end
```

## Block arg optional and post destructuring

### update

```ruby
def block_optional_post
  [[1, 2]].map do |a = :fallback, b, c|
    [a, b, c]
  end
end
```

### result

```rbs
class Object
  def block_optional_post: -> Array[[:fallback, 1, 2]]
end
```

## yield

### update

```ruby
def with_yield = yield 1
```

### result

```rbs
class Object
  def with_yield: { (Integer) -> untyped } -> untyped
end
```

## Propagate element type in block on bare receiver

### update

```ruby
class Sample
  #: -> Array[Integer]
  def foo
    [1, 2]
  end

  def bar
    foo.map do |x|
      x.digits
    end
  end
end
```

### result

```rbs
class Sample
  def foo: -> Array[Integer]
  def bar: -> Array[Array[Integer]]
end
```

## Pass local variable type from yield to block arg

### update

```ruby
class A
  def foo
    x = A.new
    yield x if block_given?
  end

  def bar
    foo do |x|
      x
    end
  end
end
```

### result

```rbs
class A
  def foo: { (A) -> A } -> A?
  def bar: -> A?
end
```

## Proc.new lambda and arrow syntax

### update

```ruby
def make_proc
  p = Proc.new { |x| x }
  l = lambda { |x| x }
  a = -> (x) { x }
  42
end
```

### result

```rbs
class Object
  def make_proc: -> 42
end
```

## Integer iteration without block returns Enumerator

### update

```ruby
def times_chain = 3.times.to_a
def upto_chain = 1.upto(3).to_a
def downto_chain = 3.downto(1).to_a
def times_block = 3.times { |i| i }
```

### result

```rbs
class Object
  def times_chain: -> Array[Integer]
  def upto_chain: -> Array[Integer]
  def downto_chain: -> Array[Integer]
  def times_block: -> 3
end
```

## Numeric step sequence chains

### update

```ruby
def integer_step_values = 1.step(5, 2).to_a

def integer_step_pairs
  1.step(5, 2).each_with_index.map { |value, index| [value, index] }
end

def integer_step_detect
  1.step(5, 2).each_with_index.detect { |value, index| index > 0 }
end

def keyword_step_values = 1.step(to: 5, by: 2).to_a

def float_step_values = 1.step(5, 0.5).to_a

def block_step_values
  values = []
  0.step(6, 2) { |value| values << value }
  values
end

def block_float_step_values
  values = []
  0.step(1, 0.5) { |value| values << value }
  values
end

def range_step_values = (1..5).step(2).to_a

def float_range_step_values = (0.0..1.0).step(0.5).to_a

def range_step_block_values
  values = []
  (1..5).step(2) { |value| values << value }
  values
end

def string_range_step_values = ("a".."c").step.to_a

def string_upto_values = "a".upto("c").to_a

def string_upto_block_values
  values = []
  "a".upto("c") { |value| values << value }
  values
end
```

### result

```rbs
class Object
  def integer_step_values: -> Array[Integer]
  def integer_step_pairs: -> Array[[Integer, Integer]]
  def integer_step_detect: -> [Integer, Integer]?
  def keyword_step_values: -> Array[Integer]
  def float_step_values: -> Array[Float]
  def block_step_values: -> Array[Integer]
  def block_float_step_values: -> Array[Float]
  def range_step_values: -> Array[Integer]
  def float_range_step_values: -> Array[Float]
  def range_step_block_values: -> Array[Integer]
  def string_range_step_values: -> Array[String]
  def string_upto_values: -> Array[String]
  def string_upto_block_values: -> Array[String]
end
```

## Return lambda as is

### update

```ruby
def make_lambda = -> () { 1 }
```

### result

```rbs
class Object
  def make_lambda: -> Proc
end
```
