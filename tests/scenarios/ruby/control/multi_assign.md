# Ruby / Control / Multi Assign

## Basic multiple assignment

### update

```ruby
def multi_assign
  a, b = 1, 2
  a
end
```

### result

```rbs
class Object < BasicObject
  def multi_assign: -> 1
end
```

## Multiple assignment with different types

### update

```ruby
def multi_mixed
  a, b = "hello", 42
  b
end
```

### result

```rbs
class Object < BasicObject
  def multi_mixed: -> 42
end
```

## Multiple assignment to three variables

### update

```ruby
def multi_three
  x, y, z = 1, "a", :b
  y
end
```

### result

```rbs
class Object < BasicObject
  def multi_three: -> "a"
end
```

## Splat assignment on left side

### update

```ruby
def splat_left
  a, *b = 1, 2, 3
  b
end
```

### result

```rbs
class Object < BasicObject
  def splat_left: -> [2, 3]
end
```

## Splat assignment on right side

### update

```ruby
def splat_right
  *a, b = 1, 2, 3
  b
end
```

### result

```rbs
class Object < BasicObject
  def splat_right: -> 3
end
```

## Splat assignment with right-side rest

### update

```ruby
def splat_right_rest
  *a, b = 1, 2, 3
  a
end
```

### result

```rbs
class Object < BasicObject
  def splat_right_rest: -> [1, 2]
end
```

## Multiple assignment from array

### update

```ruby
def from_array
  a, b = [1, 2]
  a
end
```

### result

```rbs
class Object < BasicObject
  def from_array: -> 1
end
```

## Use first variable after multiple assignment

### update

```ruby
def first_var
  a, b = "hello", 42
  a
end
```

### result

```rbs
class Object < BasicObject
  def first_var: -> "hello"
end
```

## Multiple assignment from generic Array

### update

```ruby
class A
  #: -> Array[Integer]
  def items
    []
  end

  def from_generic_array
    a, *b, c = items
    [a, b, c]
  end
end
```

### result

```rbs
class A
  def items: -> Array[Integer]
  def from_generic_array: -> [Integer?, Array[Integer], Integer?]
end
```

## Multiple assignment from fixed tuple keeps positions

### update

```ruby
class A
  def from_fixed_tuple
    a, *b, c = [1, "x", :y]
    [a, b, c]
  end
end
```

### result

```rbs
class A
  def from_fixed_tuple: -> [1, ["x"], :y]
end
```

## Multiple assignment from tuple union keeps fixed shape

### update

```ruby
class A
  def tuple_union_assign
    [[1], [1, 2]].map do |tuple|
      a, b = tuple
      [a, b]
    end
  end
end
```

### result

```rbs
class A
  def tuple_union_assign: -> Array[[1, 2?]]
end
```

## Nested tuple destructuring keeps fixed shape

### update

```ruby
class A
  def nested_multi_assign
    a, (b, c) = [1, [2, 3]]
    [a, b, c]
  end
end
```

### result

```rbs
class A
  def nested_multi_assign: -> [1, 2, 3]
end
```

## Nested tuple destructuring with splat keeps fixed shape

### update

```ruby
class A
  def nested_multi_assign_with_splat
    a, (b, *c) = [1, [2, 3, 4]]
    [a, b, c]
  end
end
```

### result

```rbs
class A
  def nested_multi_assign_with_splat: -> [1, 2, [3, 4]]
end
```

## Nested masgn with trailing rest

### update

```ruby
def test
  a, (b, *rest, c) = [1, [2, 3, 4, 5]]
  [a, b, rest, c]
end
```

### result

```rbs
class Object < BasicObject
  def test: -> [1, 2, [3, 4], 5]
end
```

## Destructuring assignment to top-level constants

### update

```ruby
HEAD, *TAIL = [1, 2, 3]
```

### result

```rbs
HEAD: 1
TAIL: [2, 3]
```

## Destructuring assignment to module body constants

### update

```ruby
module Pair
  LEFT, RIGHT, *TAIL = [1, "two", :three, false]
  VALUES = [LEFT, RIGHT, TAIL]
end
```

### result

```rbs
module Pair
  LEFT: 1
  RIGHT: "two"
  TAIL: [:three, false]
  VALUES: [1, "two", [:three, false]]
end
```

## Destructure generic Array into module body constants

### update

```ruby
module Group
  TEXT = "1.2.3"

  module Part
    FIRST, SECOND, THIRD, *REST = Group::TEXT.split "."
    ITEMS = [FIRST, SECOND, THIRD, REST]
  end
end
```

### result

```rbs
module Group
  TEXT: "1.2.3"
end

module Group::Part
  FIRST: "1"
  REST: [ ]
  SECOND: "2"
  THIRD: "3"
  ITEMS: ["1", "2", "3", [ ]]
end
```

## Destructuring assignment to class body locals

### update

```ruby
class Holder
  first, *rest = [1, 2, 3]
  ITEMS = [first, rest]
end
```

### result

```rbs
class Holder
  ITEMS: [1, [2, 3]]
end
```

## Destructuring assignment to relative constant path

### update

```ruby
module Folder
  module Entry
  end

  Entry::NAME, *Entry::TAGS = ["one", :two, :three]
end
```

### result

```rbs
module Folder::Entry
  NAME: "one"
  TAGS: [:two, :three]
end
```

## Destructuring assignment to nested constant target

### update

```ruby
module Box
  (HEAD, *MIDDLE, LAST), FLAG = [[1, 2, 3, 4], true]
  VALUES = [HEAD, MIDDLE, LAST, FLAG]
end
```

### result

```rbs
module Box
  FLAG: true
  HEAD: 1
  LAST: 4
  MIDDLE: [2, 3]
  VALUES: [1, [2, 3], 4, true]
end
```

## Destructuring assignment to instance class and global variables

### update

```ruby
class Store
  def assign
    @item, @@state, $marker, *tail = [1, "two", :three, false]
    [@item, @@state, $marker, tail]
  end
end
```

### result

```rbs
class Store
  def assign: -> [1, "two", :three, [false]]
end
```
