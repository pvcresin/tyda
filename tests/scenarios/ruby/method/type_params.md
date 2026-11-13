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
  def id: [T] (T x) -> T
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
  def keep: [T] (T x) -> T
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
  def pair: [T, U] (T a, U b) -> [T, U]
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
  def touch: [T < Cat | Dog] (T x) -> T
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
