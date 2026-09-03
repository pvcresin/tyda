# Ruby / Method / Forwarding

## Forward all args with ...

### update

```ruby
def forward_all(...) = target(...)
```

### result

```rbs
class Object < BasicObject
  def forward_all: (*untyped, **untyped, ?untyped &block) -> untyped
end
```

## Forward splat args

### update

```ruby
def splat_forward(*args) = other(*args)
```

### result

```rbs
class Object < BasicObject
  def splat_forward: (*untyped args) -> untyped
end
```

## Forward double splat args

### update

```ruby
def double_splat_forward(**opts) = other(**opts)
```

### result

```rbs
class Object < BasicObject
  def double_splat_forward: (**untyped opts) -> untyped
end
```

## Forward all parameter kinds

### update

```ruby
def combined_forward(*args, **opts, &block) = other(*args, **opts, &block)
```

### result

```rbs
class Object < BasicObject
  def combined_forward: (*untyped args, **untyped opts, ?untyped &block) -> untyped
end
```

## Forward anonymous rest in Ruby 3.2+

```yaml
ruby_version: 3.2.0
```

### update

```ruby
def anon_rest(*) = other(*)
```

### result

```rbs
class Object < BasicObject
  def anon_rest: (*untyped) -> untyped
end
```

## Forward anonymous kwargs in Ruby 3.2+

```yaml
ruby_version: 3.2.0
```

### update

```ruby
def anon_kwargs(**) = other(**)
```

### result

```rbs
class Object < BasicObject
  def anon_kwargs: (**untyped) -> untyped
end
```

## Forward anonymous block in Ruby 3.1+

```yaml
ruby_version: 3.1.0
```

### update

```ruby
def anon_block(&) = other(&)
```

### result

```rbs
class Object < BasicObject
  def anon_block: (?untyped &block) -> untyped
end
```

## Ruby 3.0 does not register anonymous rest forwarding

```yaml
ruby_version: 3.0.0
```

### update

```ruby
def anon_rest(*) = 42
```

### result

```rbs
class Object < BasicObject
  def anon_rest: -> 42
end
```

## Ruby 3.0 does not register anonymous block forwarding

```yaml
ruby_version: 3.0.0
```

### update

```ruby
def anon_block(&) = 42
```

### result

```rbs
class Object < BasicObject
  def anon_block: -> 42
end
```

## Forward typed splat args

### update

```ruby
def typed_splat(*args) = args

typed_splat(1, 2, 3)
```

### result

```rbs
class Object < BasicObject
  def typed_splat: (*Integer args) -> Array[Integer]
end
```

## lead arg + `...` forwarding

### update

```ruby
def head_forward(x, ...) = target(x, ...)

head_forward(1, "a")
```

### result

```rbs
class Object < BasicObject
  def head_forward: (Integer x, *String, **untyped, ?untyped &block) -> untyped
end
```

## literal lead arg + `...` forwarding

### update

```ruby
def foo(...)
  bar(1, ...)
end

foo("a", k: 2) { 3 }
```

### result

```rbs
class Object < BasicObject
  def foo: (*String, **Integer, ?untyped &block) -> untyped
end
```

## Forward all args to a defined method resolves its return

### update

```ruby
def bar(x)
  x
end

def foo(...)
  bar(...)
end

foo(42)
```

### result

```rbs
class Object < BasicObject
  def bar: (Integer x) -> Integer
  def foo: (*Integer, **untyped, ?untyped &block) -> Integer
end
```

## Forward all args distributes the first positional to the callee

### update

```ruby
def bar(a, b)
  [a, b]
end

def foo(...)
  bar(...)
end

foo(1, "s")
```

### result

```rbs
class Object < BasicObject
  def bar: ((Integer | String) a, untyped b) -> [Integer | String, untyped]
  def foo: (*(Integer | String), **untyped, ?untyped &block) -> [Integer | String, untyped]
end
```

## Forwarding prepends an extra positional

### update

```ruby
def foo(...)
  extra = "x"
  bar(extra, ...)
end

def bar(a, *b, **c)
  [a, b, c]
end

foo(2, x: 4, y: 5)
```

### result

```rbs
class Object < BasicObject
  def foo: (*Integer, **Integer, ?untyped &block) -> [String, Array[Integer], { x: 4, y: 5 }]
  def bar: (String a, *Integer b, **Integer c) -> [String, Array[Integer], { x: 4, y: 5 }]
end
```
