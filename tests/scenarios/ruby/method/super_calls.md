# Ruby / Method / Super Calls

## Explicit empty super keeps no arguments

### update

```ruby
class Parent
  def value(x = "default") = x
end

class Child < Parent
  def value(x) = super( )
end

def read = Child.new.value(1)
```

### result

```rbs
class Child < Parent
  def value: (Integer x) -> String
end

class Object
  def read: -> String
end

class Parent
  def value: (?String x) -> String
end
```

## Implicit super forwards the current arguments

### update

```ruby
class Parent
  def value(x) = x.to_s
end

class Child < Parent
  def value(x) = super
end

def read = Child.new.value(1)
```

### result

```rbs
class Child < Parent
  def value: (Integer x) -> String
end

class Object
  def read: -> String
end

class Parent
  def value: (untyped x) -> String
end
```

## Explicit super forwards an expression

### update

```ruby
class Parent
  def value(x) = x.to_s
end

class Child < Parent
  def value(x) = super(x)
end

def read = Child.new.value(1)
```

### result

```rbs
class Child < Parent
  def value: (Integer x) -> String
end

class Object
  def read: -> String
end

class Parent
  def value: (Integer x) -> String
end
```
