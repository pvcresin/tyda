# Ruby / Class / Cyclic Class Safety

## Self superclass does not hang

### update

```ruby
class A < A
end

def f = A.new
```

### result

```rbs
class Object
  def f: -> A
end
```

## Self include does not hang

### update

```ruby
module M
  include M
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
  include M
end

class Object
  def f: -> C
end
```

## Mutual include does not hang

### update

```ruby
module A
  include B
end

module B
  include A
end

class C
  include A
end

def f = C.new
```

### result

```rbs
module A
  include B
end

module B
  include A
end

class C
  include A
end

class Object
  def f: -> C
end
```
