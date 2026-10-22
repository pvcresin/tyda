# Ruby / Method / Param Receiver Method Ref

## String param receiver upcase

### update

```ruby
class A
  def up(s) = s.upcase
end
A.new.up("x")
```

### result

```rbs
class A
  def up: (String s) -> String
end
```

## Source method call uses its resolved parameter summary

### update

```ruby
class DeferredSourceCall
  def identity(value) = value
  def use(value) = identity(value)
end

DeferredSourceCall.new.use("known")
```

### result

```rbs
class DeferredSourceCall
  def identity: (String value) -> String
  def use: (String value) -> String
end
```

## String param receiver strip chain

### update

```ruby
class A
  def strip_up(s) = s.strip.upcase
end
A.new.strip_up(" y ")
```

### result

```rbs
class A
  def strip_up: (String s) -> String
end
```

## Integer params operator plus

### update

```ruby
class A
  def add(a, b) = a + b
end
A.new.add(1, 2)
```

### result

```rbs
class A
  def add: (Integer a, Integer b) -> Integer
end
```

## Array element param first

### update

```ruby
class A
  def hd(arr) = arr.first
end
A.new.hd([1])
```

### result

```rbs
class A
  def hd: (Array[Integer] arr) -> Integer?
end
```

## Array param map block param

### update

```ruby
class A
  def mp(arr) = arr.map { |x| x }
end
A.new.mp([1])
```

### result

```rbs
class A
  def mp: (Array[Integer] arr) -> Array[Integer]
end
```

## Union param common method resolves

### update

```ruby
class A
  def s(v) = v.to_s
end
A.new.s(1)
A.new.s("x")
```

### result

```rbs
class A
  def s: ((Integer | String) v) -> String
end
```

## Typed param then block increments

### update

```ruby
class A
  def thn(x) = x.then { |y| y + 1 }
end
A.new.thn(3)
```

### result

```rbs
class A
  def thn: (Integer x) -> Integer
end
```

## Record param index access

### update

```ruby
class A
  def idx(h) = h[:key]
end
A.new.idx({ key: 5 })
```

### result

```rbs
class A
  def idx: ({ key: Integer } h) -> Integer
end
```

## Safe navigation on typed param

### update

```ruby
class A
  def nav(s) = s&.upcase
end
A.new.nav("z")
```

### result

```rbs
class A
  def nav: (String s) -> String
end
```

## Recursive param receiver does not diverge

### update

```ruby
class A
  def rec(n) = rec(n).succ
end
A.new.rec(1)
def use_rec = A.new.rec(1)
```

### result

```rbs
class A
  def rec: (Integer n) -> untyped
end

class Object < BasicObject
  def use_rec: -> untyped
end
```

## Recursive arithmetic parameter resolves after caller substitution

```yaml
known_issue: true
```

### update

```ruby
class RecursiveEmitter
  def emit(value, depth)
    if depth <= 0
      value
    else
      forward(value + 2, depth - 1)
    end
  end

  def forward(value, depth)
    emit(value, depth)
  end
end

RecursiveEmitter.new.emit(1, 1)
```

### result

```rbs
class RecursiveEmitter
  def emit: (Integer value, Integer depth) -> Integer
  def forward: (Integer value, Integer depth) -> Integer
end
```

## Param receiver resolves the matching arithmetic overload

### update

```ruby
class FloatAdder
  def add(left, right) = left + right
end

FloatAdder.new.add(1, 0.5)
```

### result

```rbs
class FloatAdder
  def add: (Integer left, Float right) -> Float
end
```

## Param receiver dispatches every observed union alternative

### update

```ruby
class UnionAdder
  def add(left, right) = left + right
end

UnionAdder.new.add(1, 1)
UnionAdder.new.add(1, 0.5)
```

### result

```rbs
class UnionAdder
  def add: (Integer left, (Integer | Float) right) -> (Integer | Float)
end
```

## Param receiver preserves an unknown argument alternative

### update

```ruby
class MaybeAdder
  def add(left, right) = left + right
end

MaybeAdder.new.add(1, 1)
MaybeAdder.new.add(1, external_number)
```

### result

```rbs
class MaybeAdder
  def add: (Integer left, (Integer | untyped) right) -> (Integer | untyped)
end
```

## Param receiver respects a user-defined method owner without RBS

### update

```ruby
class CustomItself
  def itself = external_value
end

class ItselfWrapper
  def extract(value) = value.itself
end

ItselfWrapper.new.extract(CustomItself.new)
```

### result

```rbs
class CustomItself
  def itself: -> untyped
end

class ItselfWrapper
  def extract: (CustomItself value) -> untyped
end
```

## Union dispatch keeps the actual source method owner

### update

```ruby
class RbsMethodOwner
  #: (Integer | String) -> Symbol
  def classify(value) = :classified
end

class InferredMethodOwner
  def classify(value) = value
end

def call_rbs_method_owner(value)
  RbsMethodOwner.new.classify(value)
end

def call_inferred_method_owner(value)
  InferredMethodOwner.new.classify(value)
end

call_rbs_method_owner(1)
call_rbs_method_owner("text")
call_inferred_method_owner(1)
call_inferred_method_owner("text")
```

### result

```rbs
class InferredMethodOwner
  def classify: ((Integer | String) value) -> (Integer | String)
end

class Object < BasicObject
  def call_rbs_method_owner: ((Integer | String) value) -> Symbol
  def call_inferred_method_owner: ((Integer | String) value) -> (Integer | String)
end

class RbsMethodOwner
  def classify: ((Integer | String) value) -> Symbol
end
```
