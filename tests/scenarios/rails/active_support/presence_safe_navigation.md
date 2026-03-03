# Rails / Active Support / Presence Safe Navigation

## presence nilable receiver returns Integer or nil after safe navigation

### update

```ruby
class A
  #: () -> Integer
  def to_i = 1
end

class B
  def foo(x = nil)
    y = x.presence&.to_i
    y
  end
end

B.new.foo(A.new)
```

### result

```rbs
class A
  def to_i: -> Integer
end

class B
  def foo: (?A? x) -> Integer?
end
```

## Safe navigation on non-nil receiver returns Integer

### update

```ruby
class A
  #: () -> Integer
  def to_i = 1
end

class B
  def foo
    y = A.new&.to_i
    y
  end
end
```

### result

```rbs
class A
  def to_i: -> Integer
end

class B
  def foo: -> Integer
end
```

## presence safe navigation with unknown branch returns untyped or nil

### update

```ruby
class A
  def foo(x = nil)
    y = x.presence&.missing_call
    y
  end
end

A.new.foo("x")
```

### result

```rbs
class A
  def foo: (?String? x) -> (nil | untyped)
end
```
