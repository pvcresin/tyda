# Ruby / Method / Type Parameters

## A pass-through parameter becomes a method type parameter

### update

```ruby
class Ident
  def id(x) = x
end

Ident.new.id(1)
Ident.new.id("s")
```

### result

```rbs
class Ident
  def id: [X] (X x) -> X
end
```

## A pass-through parameter without callers becomes a method type parameter

### update

```ruby
class Keeper
  def keep(x) = x
end
```

### result

```rbs
class Keeper
  def keep: [X] (X x) -> X
end
```

## Two pass-through parameters become separate type parameters

### update

```ruby
class Pairer
  def pair(a, b) = [a, b]
end
```

### result

```rbs
class Pairer
  def pair: [A, B] (A a, B b) -> [A, B]
end
```

## A dispatched parameter in the return type gets a union bound from callers

### update

```ruby
class Cat
  def name = "cat"
end

class Dog
  def name = "dog"
end

class Namer
  def touch(x)
    x.name
    x
  end
end

Namer.new.touch(Cat.new)
Namer.new.touch(Dog.new)
```

### result

```rbs
class Cat
  def name: -> "cat"
end

class Dog
  def name: -> "dog"
end

class Namer
  def touch: [X < Cat | Dog] (X x) -> X
end
```

## A used parameter without callers stays untyped

### update

```ruby
class Toucher
  def touch(x)
    x.name
    x
  end
end
```

### result

```rbs
class Toucher
  def touch: (untyped x) -> untyped
end
```

## A reassigned parameter does not become a type parameter

### update

```ruby
class Resetter
  def reset(x)
    x = x.to_s
    x
  end
end

Resetter.new.reset(1)
```

### result

```rbs
class Resetter
  def reset: (Integer x) -> String
end
```

## A parameter narrowed by a pattern keeps the narrowed return

### update

```ruby
class Matcher
  def only_integer(x)
    x => Integer
    x
  end
end

Matcher.new.only_integer(1)
```

### result

```rbs
class Matcher
  def only_integer: (Integer x) -> Integer
end
```

## A parameter returned beside an untyped value does not become a type parameter

### update

```ruby
class Picker
  def pick(x, flag) = flag ? x : unknown_value
end

Picker.new.pick(1, true)
```

### result

```rbs
class Picker
  def pick: (Integer x, bool flag) -> untyped
end
```

## A used parameter that callers only pass nil does not become a type parameter

### update

```ruby
class NilPasser
  def pass(x)
    x.inspect
    x
  end
end

NilPasser.new.pass(nil)
```

### result

```rbs
class NilPasser
  def pass: (nil x) -> nil
end
```

## Yielding a parameter links the block types

### update

```ruby
class Yielder
  def with_value(x) = yield(x)
end
```

### result

```rbs
class Yielder
  def with_value: [X, R] (X x) { (X) -> R } -> R
end
```

## A method returning its block value takes the block return as a type parameter

### update

```ruby
class Locker
  def lock = yield
  def maybe(flag) = flag ? yield : nil
end

class LockerUser
  def value = Locker.new.lock { 1 }
end
```

### result

```rbs
class Locker
  def lock: [R] { () -> R } -> R
  def maybe: [R] (untyped flag) { () -> R } -> R?
end

class LockerUser
  def value: -> 1
end
```

## Each call returns the argument bound to the type parameter

### update

```ruby
class Echo
  def id(x) = x
end

class EchoUser
  def text = Echo.new.id("x")
  def number = Echo.new.id(1)
end
```

### result

```rbs
class Echo
  def id: [X] (X x) -> X
end

class EchoUser
  def text: -> "x"
  def number: -> 1
end
```

## Type parameter names follow the parameter names

### update

```ruby
class Names
  def pick(details, data) = [details, data]
  def ids(user_id, user_name) = [user_id, user_name]
  def map_row(row) = yield(row)
end
```

### result

```rbs
class Names
  def pick: [De, Da] (De details, Da data) -> [De, Da]
  def ids: [UI, UN] (UI user_id, UN user_name) -> [UI, UN]
  def map_row: [Ro, Re] (Ro row) { (Ro) -> Re } -> Re
end
```
