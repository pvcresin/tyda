# Known Issues / Generics Output

## A pass-through parameter becomes a method type parameter

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
  def id: [T] (T x) -> T
end
```

## A pass-through parameter without callers becomes a method type parameter

```ruby
class Keeper
  def keep(x) = x
end
```

### result

```rbs
class Keeper
  def keep: [T] (T x) -> T
end
```

## Two pass-through parameters become separate type parameters

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
  def touch: [T < Cat | Dog] (T x) -> T
end
```

## Calling a proc parameter links its argument and return types

```ruby
class Applier
  def apply(f, x) = f.call(x)
end
```

### result

```rbs
class Applier
  def apply: [A, R] (^(A) -> R f, A x) -> R
end
```

## Yielding a parameter links the block types

```ruby
class Yielder
  def with_value(x) = yield(x)
end
```

### result

```rbs
class Yielder
  def with_value: [A, R] (A x) { (A) -> R } -> R
end
```
