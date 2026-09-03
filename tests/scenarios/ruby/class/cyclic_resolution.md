# Ruby / Class / Cyclic Resolution

## Analysis finishes with cyclic superclass

### update

```ruby
class A < B
end

class B < C
end

class C < A
end

def f = A.new
```

### result

```rbs
class Object < BasicObject
  def f: -> A
end
```

## Bare constant through missing parent is untyped

### update

```ruby
class A < Missing
  def f = MissingConst
end
```

### result

```rbs
class A < Missing
  def f: -> untyped
end
```

## Self prepend is stable as ancestor

### update

```ruby
module M
  prepend M
end

class C
  include M
end

def f = C.new
```

### result

```rbs
class C
  include M
end

module M
  prepend M
end

class Object < BasicObject
  def f: -> C
end
```
