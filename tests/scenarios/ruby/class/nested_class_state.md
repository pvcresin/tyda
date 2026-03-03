# Ruby / Class / Nested Class State

## Nested class does not share outer class ivars

### update

```ruby
class Outer
  @x = 1

  class Inner
    def self.x = @x
  end
end
```

### result

```rbs
class Outer::Inner
  def self.x: -> untyped
end
```

## Nested class can read outer constants through lexical scope

### update

```ruby
class Outer
  CONST = "outer"

  class Inner
    def self.f = CONST
  end
end
```

### result

```rbs
class Outer
  CONST: "outer"
end

class Outer::Inner
  def self.f: -> "outer"
end
```
