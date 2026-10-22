# Ruby / RBS Input / Array Overloads

## first(n) on static Tuple returns head shape

### update

```ruby
def test_first_n = [1, 2, 3].first(2)
```

### result

```rbs
class Object < BasicObject
  def test_first_n: -> [1, 2]
end
```

## last(n) on static Tuple returns tail shape

### update

```ruby
def test_last_n = [1, 2, 3].last(2)
```

### result

```rbs
class Object < BasicObject
  def test_last_n: -> [2, 3]
end
```

## first on static Tuple returns first element

### update

```ruby
def test_first = [1, 2, 3].first
```

### result

```rbs
class Object < BasicObject
  def test_first: -> 1
end
```

## last on static Tuple returns last element

### update

```ruby
def test_last = ["a", "b"].last
```

### result

```rbs
class Object < BasicObject
  def test_last: -> "b"
end
```

## take and drop on static Tuple return slice shape

### update

```ruby
def test_take = [1, 2, 3, 4].take(2)

def test_drop = [1, 2, 3, 4].drop(2)
```

### result

```rbs
class Object < BasicObject
  def test_take: -> [1, 2]
  def test_drop: -> [3, 4]
end
```

## Unknown arguments retain compatible overload returns

### update

```ruby
def test_array_times_unknown = [1] * OverloadInput.value
def test_unknown_argument = OverloadInput.value
def test_array_times_integer = [1] * 2
def test_array_times_string = [1] * ","
def overload_with_observed_union(value) = OverloadOrder.choose(value)
def test_known_and_unknown_overload_input
  overload_with_observed_union(1)
  overload_with_observed_union(OverloadInput.value)
end
def test_reversed_overload_unknown = OverloadOrder.choose(OverloadInput.value)
def test_reversed_overload_integer = OverloadOrder.choose(1)
def test_reversed_overload_string = OverloadOrder.choose("x")
def test_flipped_declaration_unknown = FlippedOverloadOrder.choose(OverloadInput.value)
def test_candidate_type_vars = CandidateTypeVars.choose(OverloadInput.value, OverloadInput.integer_value)
def test_unknown_overload_return = UnknownOverloadReturn.choose(OverloadInput.value)
def test_explicit_single_overload = ExplicitOverload.choose(OverloadInput.value)
```

```rbs
module OverloadInput
  def self.integer_value: -> Integer
  def self.value: -> untyped
end

class OverloadOrder
  def self.choose: (Integer value) -> String
                 | (String value) -> Integer
end

class FlippedOverloadOrder
  def self.choose: (String value) -> Integer
                 | (Integer value) -> String
end

class CandidateTypeVars
  def self.choose: [A] (A value, Integer marker) -> A
                 | [B] (String value, B marker) -> B
end

class UnknownOverloadReturn
  def self.choose: (String value) -> untyped
                 | (Integer value) -> Integer
end

class ExplicitOverload
  def self.choose: (untyped value) -> String
end
```

### result

```rbs
class Object < BasicObject
  def test_array_times_unknown: -> String | Array[1]
  def test_unknown_argument: -> untyped
  def test_array_times_integer: -> Array[1]
  def test_array_times_string: -> "1"
  def overload_with_observed_union: ((Integer | untyped) value) -> (Integer | String)
  def test_known_and_unknown_overload_input: -> Integer | String
  def test_reversed_overload_unknown: -> Integer | String
  def test_reversed_overload_integer: -> String
  def test_reversed_overload_string: -> Integer
  def test_flipped_declaration_unknown: -> Integer | String
  def test_candidate_type_vars: -> Integer | untyped
  def test_unknown_overload_return: -> Integer | untyped
  def test_explicit_single_overload: -> String
end
```
