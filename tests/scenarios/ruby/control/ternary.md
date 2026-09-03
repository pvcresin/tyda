# Ruby / Control / Ternary

## Ternary with different types

### update

```ruby
def foo(x) = x ? "yes" : 42
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> (42 | "yes")
end
```

## Ternary with same type

### update

```ruby
def foo(x) = x ? "yes" : "no"
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> ("no" | "yes")
end
```

## Nested conditional expression

### update

```ruby
def foo(x, y)
  if x
    if y
      1
    else
      2
    end
  else
    3
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x, untyped y) -> (1 | 2 | 3)
end
```

## Ternary-selected hash receiver mutation updates the chosen hash

### update

```ruby
def store(kind, key, info)
  required = {}
  optional = {}
  (kind == :required ? required : optional)[key] = info
  [required, optional]
end

def a = store(:required, :name, "n")
```

### result

```rbs
class Object < BasicObject
  def store: (Symbol kind, Symbol key, String info) -> ([Hash[Symbol, String], Hash[untyped, untyped]] | [Hash[untyped, untyped], Hash[Symbol, String]])
  def a: -> [Hash[Symbol, String], Hash[untyped, untyped]] | [Hash[untyped, untyped], Hash[Symbol, String]]
end
```
