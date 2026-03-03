# Sorbet / RBS Comment / Type Aliases

## type alias comment

### update

`sorbet/config`

```ruby
.
```

```ruby
#: type int_or_string =
#| Integer |
#| String

#: (int_or_string) -> int_or_string
def id(x) = x
```

### result

```rbs
class Object
  def id: ((Integer | String) x) -> (Integer | String)
end
```

## type alias inherits outer class scope

### update

`sorbet/config`

```ruby
.
```

```ruby
class Box
  #: type elemish = Integer | String

  class Inner
    #: (elemish) -> elemish
    def id(x) = x
  end
end
```

### result

```rbs
class Box::Inner
  def id: ((Integer | String) x) -> (Integer | String)
end
```
