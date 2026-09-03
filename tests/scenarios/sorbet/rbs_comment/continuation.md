# Sorbet / RBS Comment / Continuation

## #| basic continuation line

### update

`sorbet/config`

```ruby
.
```

```ruby
#: (
#|   Integer,
#|   String
#| ) -> Float
def calc(x, y) = x.to_f
```

### result

```rbs
class Object < BasicObject
  def calc: (Integer x, String y) -> Float
end
```

## #| continuation line with long return type

### update

`sorbet/config`

```ruby
.
```

```ruby
#: (String) ->
#|   Array[Integer]
def parse_ids(input) = []
```

### result

```rbs
class Object < BasicObject
  def parse_ids: (String input) -> Array[Integer]
end
```
