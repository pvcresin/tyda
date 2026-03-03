# Ruby / Method / Def With Const Receiver

## Register `def Foo.bar` as Foo singleton method

### update

```ruby
class A
end

def A.hello = :hi

def f = A.hello
```

### result

```rbs
class A
  def self.hello: -> :hi
end

class Object
  def f: -> :hi
end
```

## Register def in `class << Foo` as Foo singleton method

### update

```ruby
class A
end

class << A
  def hello = :hi

  CONST = 7
end

def f = A.hello
```

### result

```rbs
class A
  def self.hello: -> :hi
end

class Object
  def f: -> :hi
end
```

## Treat ivar in `def Foo.bar` as Foo singleton ivar

### update

```ruby
class A
end

def A.set
  @v = 1
end

def A.get = @v
```

### result

```rbs
class A
  def self.set: -> 1
  def self.get: -> 1
end
```

## Ivar in a nested const-receiver def belongs to the receiver

### update

```ruby
class Foo
  def self.get = @ivar
end

class Bar
  def Foo.set
    @ivar = 1
  end
end
```

### result

```rbs
class Foo
  def self.get: -> 1
  def self.set: -> 1
end
```

## Class variable in a const-receiver def follows the lexical class

### update

```ruby
class Foo
  def self.from_foo = @@cvar
end

class Bar
  def Foo.demo
    @@cvar = 1
  end

  def self.from_bar = @@cvar
end
```

### result

```rbs
class Bar
  def self.from_bar: -> 1
end

class Foo
  def self.from_foo: -> untyped
  def self.demo: -> 1
end
```
