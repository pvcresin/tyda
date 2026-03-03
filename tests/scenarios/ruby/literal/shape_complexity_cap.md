# Ruby / Literal / Shape Complexity Cap

## Repeated `x << x` collapses to nested Array

### update

```ruby
class A
  def f
    x = []
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x << x
    x
  end
end
```

### result

```rbs
class A
  def f: -> Array[Array[Array[Array[Array[Array[Array[untyped]]]]]]]
end
```

## Repeated self array assignment collapses to Array

### update

```ruby
class A
  def f
    x = []
    x = [x, x, x, x]
    x = [x, x, x, x]
    x = [x, x, x, x]
    x = [x, x, x, x]
    x = [x, x, x, x]
    x = [x, x, x, x]
    x = [x, x, x, x]
    x = [x, x, x, x]
    x = [x, x, x, x]
    x = [x, x, x, x]
    x = [x, x, x, x]
    x = [x, x, x, x]
    x
  end
end
```

### result

```rbs
class A
  def f: -> Array[Array[Array[Array[Array[Array[Array[Array[Array[Array[[[[ ], [ ], [ ], [ ]], [[ ], [ ], [ ], [ ]], [[ ], [ ], [ ], [ ]], [[ ], [ ], [ ], [ ]]]]]]]]]]]]]
end
```

## Repeated self Hash write collapses to Hash[Symbol, untyped]

### update

```ruby
class A
  def f
    h = {}
    h[:k0] = h
    h[:k1] = h
    h[:k2] = h
    h[:k3] = h
    h[:k4] = h
    h[:k5] = h
    h[:k6] = h
    h[:k7] = h
    h[:k8] = h
    h[:k9] = h
    h[:k10] = h
    h[:k11] = h
    h[:k12] = h
    h
  end
end
```

### result

```rbs
class A
  def f: -> Hash[:k0 | :k1 | :k10 | :k11 | :k12 | :k2 | :k3 | :k4 | :k5 | :k6 | :k7 | :k8 | :k9, Hash[:k0 | :k1 | :k10 | :k11 | :k2 | :k3 | :k4 | :k5 | :k6 | :k7 | :k8 | :k9, untyped]]
end
```

## Repeated self Hash write with self key collapses to Hash

### update

```ruby
class A
  def f
    h = {}
    h[h] = h
    h[h] = h
    h[h] = h
    h[h] = h
    h[h] = h
    h
  end
end
```

### result

```rbs
class A
  def f: -> Hash[Hash[untyped, untyped], Hash[untyped, untyped]]
end
```
