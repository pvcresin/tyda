# Known Issues / Generics Output

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
