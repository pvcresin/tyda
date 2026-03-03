# Ruby / Class / Prepend Order

## Prepended module method wins over class method

### update

```ruby
module M
  def hello = :module
end

class C
  prepend M

  def hello = :class
end

def f = C.new.hello
```

### result

```rbs
class C
  prepend M

  def hello: -> :class
end

module M
  def hello: -> :module
end

class Object
  def f: -> :module
end
```

## Use prepend class include order

### update

```ruby
module Pre
  def hello = :pre
end

module Inc
  def hello = :inc
end

class C
  prepend Pre
  include Inc

  def hello = :class
end

def f = C.new.hello
```

### result

```rbs
class C
  prepend Pre
  include Inc

  def hello: -> :class
end

module Inc
  def hello: -> :inc
end

class Object
  def f: -> :pre
end

module Pre
  def hello: -> :pre
end
```

## Prepended method is visible when class has no same-name method

### update

```ruby
module M
  def hello = :module
end

class C
  prepend M
end

def f = C.new.hello
```

### result

```rbs
class C
  prepend M
end

module M
  def hello: -> :module
end

class Object
  def f: -> :module
end
```
