# Ruby / RBS Input / Record Types

## Return RBS record return value as is

### update

```rbs
class A
  def foo: -> { x: String, y: Integer }
end
```

```ruby
class A
  def bar = foo
end
```

### result

```rbs
class A
  def bar: -> { x: String, y: Integer }
end
```

## Track RBS record field access

### update

```rbs
class A
  def foo: -> { x: String, y: Integer }
end
```

```ruby
class A
  def bar
    h = foo
    h[:x]
  end
end
```

### result

```rbs
class A
  def bar: -> String
end
```

## Preserve RBS optional record field

### update

```rbs
class A
  def foo: -> { ?x: String, y: Integer }
end
```

```ruby
class A
  def bar = foo
end
```

### result

```rbs
class A
  def bar: -> { ?x: String, y: Integer }
end
```

## Read RBS optional record field

### update

```rbs
class A
  def foo: -> { ?x: String, y: Integer }
end
```

```ruby
class A
  def bar
    h = foo
    h[:x]
  end
end
```

### result

```rbs
class A
  def bar: -> String?
end
```

## Read RBS record with variable key

### update

```rbs
class A
  def foo: -> { x: String, y: Integer }
end
```

```ruby
class A
  def bar(k)
    h = foo
    h[k]
  end
end
```

### result

```rbs
class A
  def bar: (untyped k) -> (Integer | String)?
end
```
