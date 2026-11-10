# Ruby / Class / Constant Alias

## `A::CONST` reads B constant after `A = B`

```yaml
known_issue: true
```

### update

```ruby
module B
  CONST = 7
end

A = B

def f = A::CONST
```

### result

```rbs
module A = B

module B
  CONST: 7
end

class Object < BasicObject
  def f: -> 7
end
```

## `A.new` returns B instance after `A = B`

```yaml
known_issue: true
```

### update

```ruby
class B
  def hello = :hi
end

A = B

def f = A.new
```

### result

```rbs
class A = B

class B
  def hello: -> :hi
end

class Object < BasicObject
  def f: -> B
end
```

## Chained aliases read final namespace constants

```yaml
known_issue: true
```

### update

```ruby
module Target
  V = "t"
end

A = Target
C = A

def f = C::V
```

### result

```rbs
module A = Target
module C = Target

class Object < BasicObject
  def f: -> "t"
end

module Target
  V: "t"
end
```

## `A::CONST` reads nested class constants after alias

```yaml
known_issue: true
```

### update

```ruby
module Outer
  module Inner
    V = 1
  end
end

A = Outer::Inner

def f = A::V
```

### result

```rbs
module A = Outer::Inner

class Object < BasicObject
  def f: -> 1
end

module Outer::Inner
  V: 1
end
```

## Cyclic alias returns untyped without looping

### update

```ruby
A = A

def f = A
```

### result

```rbs
A: untyped

class Object < BasicObject
  def f: -> untyped
end
```

## Class defined under a constant alias uses the alias target

```yaml
known_issue: true
```

### update

```ruby
module Outer
  CONST = 1
end

ALIAS = Outer

class ALIAS::NewClass
  def self.v = 2
end

def f = Outer::NewClass.v
def g = ALIAS::NewClass.v
```

### result

```rbs
module ALIAS = Outer

class Object < BasicObject
  def f: -> 2
  def g: -> 2
end

module Outer
  CONST: 1
end

class Outer::NewClass
  def self.v: -> 2
end
```

## Superclass through a constant alias

```yaml
known_issue: true
```

### update

```ruby
class Base
  def hello = :hi
end

AliasedBase = Base

class Foo < AliasedBase
end

def f = Foo.new.hello
```

### result

```rbs
class AliasedBase = Base

class Base
  def hello: -> :hi
end

class Object < BasicObject
  def f: -> :hi
end
```

## Mixin through a constant alias

```yaml
known_issue: true
```

### update

```ruby
module M
  def hello = :hi
end

AliasM = M

class Foo
  include AliasM
end

def f = Foo.new.hello
```

### result

```rbs
module AliasM = M

class Foo
  include M
end

module M
  def hello: -> :hi
end

class Object < BasicObject
  def f: -> :hi
end
```

## Alias assigned before the target still resolves constants

### update

```ruby
ALIAS = Foo

module Foo
  CONST = 1
end

def f = ALIAS::CONST
```

### result

```rbs
ALIAS: singleton(Foo)

module Foo
  CONST: 1
end

class Object < BasicObject
  def f: -> 1
end
```

## Self-referential namespace alias follows the module

```yaml
known_issue: true
```

### update

```ruby
module M
  SELF_REF = M

  class Thing
    CONST = 1
  end
end

def f = M::SELF_REF::Thing::CONST
```

### result

```rbs
module M
  module SELF_REF = M
end

class M::Thing
  CONST: 1
end

class Object < BasicObject
  def f: -> 1
end
```

## Ping-pong namespace aliases resolve each Deep::VALUE

```yaml
known_issue: true
```

### update

```ruby
module Left
  module Deep
    VALUE = "left"
  end
end

module Right
  module Deep
    VALUE = "right"
  end
end

Left::RIGHT_REF = Right
Right::LEFT_REF = Left

def f = Left::RIGHT_REF::Deep::VALUE
def g = Left::RIGHT_REF::LEFT_REF::Deep::VALUE
```

### result

```rbs
module Left
  module RIGHT_REF = Right
end

module Left::Deep
  VALUE: "left"
end

class Object < BasicObject
  def f: -> "right"
  def g: -> "left"
end

module Right
  module LEFT_REF = Left
end

module Right::Deep
  VALUE: "right"
end
```

## Inherited constant is visible through a subclass alias

```yaml
known_issue: true
```

### update

```ruby
class Foo
  CONST = 123
end

class Bar < Foo
end

ALIAS = Bar

def f = ALIAS::CONST
```

### result

```rbs
class ALIAS = Bar

class Foo
  CONST: 123
end

class Object < BasicObject
  def f: -> 123
end
```

## Singleton method defined on an alias belongs to the target

```yaml
known_issue: true
```

### update

```ruby
class Foo
end

ALIAS = Foo

def ALIAS.bar = :hi

def f = Foo.bar
def g = ALIAS.bar
```

### result

```rbs
class ALIAS = Foo

class Foo
  def self.bar: -> :hi
end

class Object < BasicObject
  def f: -> :hi
  def g: -> :hi
end
```

## Method call on a namespace alias uses the target singleton

```yaml
known_issue: true
```

### update

```ruby
class Foo
  def self.bar = :hi
end

ALIAS = Foo

def f = ALIAS.bar
```

### result

```rbs
class ALIAS = Foo

class Foo
  def self.bar: -> :hi
end

class Object < BasicObject
  def f: -> :hi
end
```

## Value alias is not a namespace for further constants

### update

```ruby
VALUE = 1
ALIAS = VALUE

def f = ALIAS
def g = ALIAS::NOPE
```

### result

```rbs
VALUE: 1
ALIAS: 1

class Object < BasicObject
  def f: -> 1
  def g: -> untyped
end
```

## `class Alias` reopens the aliased class

```yaml
known_issue: true
```

### update

```ruby
class Foo
  def self.build = 1
end

Alias = Foo

class Alias
  def value = 2
end

def run = Foo.new.value
```

### result

```rbs
class Alias = Foo

class Foo
  def self.build: -> 1
  def value: -> 2
end

class Object < BasicObject
  def run: -> 2
end
```
