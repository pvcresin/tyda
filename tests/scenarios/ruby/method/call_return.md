# Ruby / Method / Call Return

## String-like call alone does not force String

### update

```ruby
class A
  def foo(x) = x.strip[0, 10]
end
```

### result

```rbs
class A
  def foo: (untyped x) -> untyped
end
```

## Return another method return value

### update

```ruby
def foo = 1
def bar = foo
```

### result

```rbs
class Object < BasicObject
  def foo: -> 1
  def bar: -> 1
end
```

## Chained method call

### update

```ruby
def first = "hello"
def second = first
def third = second
```

### result

```rbs
class Object < BasicObject
  def first: -> "hello"
  def second: -> "hello"
  def third: -> "hello"
end
```

## Method call inside class

### update

```ruby
class A
  def foo = 42
  def bar = foo
end
```

### result

```rbs
class A
  def foo: -> 42
  def bar: -> 42
end
```

## Infer Object method on untyped receiver

### update

```ruby
class Checker
  def check_nil(x) = x.nil?
  def check_equal(x, y) = x == y
  def object_identity(x) = x.object_id
  def to_string(x) = x.to_s
  def to_integer(x) = x.to_i
  def check_pred(x) = x.valid?
end
```

### result

```rbs
class Checker
  def check_nil: (untyped x) -> bool
  def check_equal: (untyped x, untyped y) -> bool
  def object_identity: (untyped x) -> Integer
  def to_string: (untyped x) -> String
  def to_integer: (untyped x) -> Integer
  def check_pred: (untyped x) -> bool
end
```

## Infer universal conversion methods on untyped receiver

### update

```ruby
class Conv
  def sym(x) = x.to_sym
  def len(x) = x.length
  def siz(x) = x.size
  def cnt(x) = x.count
  def flt(x) = x.to_f
  def arr(x) = x.to_a
  def neg(x) = !x
end
```

### result

```rbs
class Conv
  def sym: (untyped x) -> Symbol
  def len: (untyped x) -> Integer
  def siz: (untyped x) -> Integer
  def cnt: (untyped x) -> Integer
  def flt: (untyped x) -> Float
  def arr: (untyped x) -> Array
  def neg: (untyped x) -> bool
end
```

## Resolve Kernel conversion methods as bare calls

### update

```ruby
class A
  def to_i = Integer("42")
  def to_f = Float("3.14")
  def to_s_str = String(42)
  def to_k_via_singleton = Kernel.Integer("42")
end
```

### result

```rbs
class A
  def to_i: -> 42
  def to_f: -> Float
  def to_s_str: -> String
  def to_k_via_singleton: -> Integer
end
```

## Resolve super return from parent method

### update

```ruby
class Parent
  def greet = "hello"
end

class Child < Parent
  def greet
    super + " world"
  end
end
```

### result

```rbs
class Child < Parent
  def greet: -> String
end

class Parent
  def greet: -> "hello"
end
```

## super inside def self.new returns class instance

### update

```ruby
class Cached
  def initialize(key) = @key = key

  def self.new(key)
    super
  end
end
```

### result

```rbs
class Cached
  def initialize: (untyped key) -> void
  def self.new: (untyped key) -> Cached
end
```

## Enumerable inject and reduce with symbol proc

### update

```ruby
class A
  def self.a = [1,2,3].inject(:+)
  def self.b = [1,2,3].inject(0, :+)
  def self.c = [1.0,2.0].inject(:+)
  def self.d = [1,2,3].reduce(:*)
end
```

### result

```rbs
class A
  def self.a: -> Integer
  def self.b: -> Integer
  def self.c: -> Float
  def self.d: -> Integer
end
```

## Kernel rand(Integer) returns Integer

### update

```ruby
class A
  def self.rand0 = rand
  def self.rand_int = rand(10)
end
```

### result

```rbs
class A
  def self.rand0: -> Float
  def self.rand_int: -> Integer
end
```

## super positional arg reaches parent initialize param

### update

```ruby
class Base
  attr_reader :command, :summary

  def initialize(command, summary = nil)
    @command = command
    @summary = summary
  end
end

class Push < Base
  def initialize
    super "push", "Push a gem"
  end
end

Push.new
```

### result

```rbs
class Base
  def command: -> "push"
  def summary: -> "Push a gem"
  def initialize: (String command, ?String? summary) -> void
end

class Push < Base
  def initialize: -> void
end
```
