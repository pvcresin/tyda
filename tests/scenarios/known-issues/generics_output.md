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
