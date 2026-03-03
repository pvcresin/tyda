# Ruby / Class / BasicObject Inheritance

## BasicObject subclass cannot see top-level bare constants

### update

```ruby
TOP = 1

class A < BasicObject
  def f = TOP
end
```

### result

```rbs
TOP: 1

class A < BasicObject
  def f: -> untyped
end
```

## BasicObject subclass can see absolute `::TOP`

### update

```ruby
TOP = 1

class A < BasicObject
  def f = ::TOP
end
```

### result

```rbs
TOP: 1

class A < BasicObject
  def f: -> 1
end
```
