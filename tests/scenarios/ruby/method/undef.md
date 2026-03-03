# Ruby / Method / Undef

## undef hides direct and inherited methods

### update

```ruby
class UndefBase
  def bar = "bar"
end

class UndefChild < UndefBase
  undef bar
end

class UndefDirect
  def foo = "foo"
  undef foo
end

class UndefUse
  def base = UndefBase.new.bar
  def child = UndefChild.new.bar
  def direct = UndefDirect.new.foo
end
```

### result

```rbs
class UndefBase
  def bar: -> "bar"
end

class UndefUse
  def base: -> "bar"
  def child: -> untyped
  def direct: -> untyped
end
```

## Conditional undef keeps the method

### update

```ruby
class A
  def always = 1
  def maybe = 2

  undef always
  undef maybe if RUBY_VERSION > "3"
end
```

### result

```rbs
class A
  def maybe: -> 2
end
```
