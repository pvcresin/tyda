# Ruby / Control / Case When

## Basic case when

### update

```ruby
def check(x)
  case x
  when 1
    "one"
  when 2
    "two"
  else
    "other"
  end
end
check(1)
```

### result

```rbs
class Object < BasicObject
  def check: (Integer x) -> ("one" | "other" | "two")
end
```

## case when with range

### update

```ruby
def range_case(x)
  case x
  when 1..5
    "low"
  when 6..10
    "high"
  end
end
range_case(3)
```

### result

```rbs
class Object < BasicObject
  def range_case: (Integer x) -> ("high" | "low")?
end
```

## case when with class

### update

```ruby
def class_case(x)
  case x
  when Integer
    "integer"
  when String
    "string"
  when Symbol
    "symbol"
  else
    "other"
  end
end
class_case(42)
```

### result

```rbs
class Object < BasicObject
  def class_case: (Integer x) -> ("integer" | "other" | "string" | "symbol")
end
```

## case without expression

### update

```ruby
def no_expr_case(x)
  case
  when x > 0
    "positive"
  when x < 0
    "negative"
  else
    "zero"
  end
end
no_expr_case(1)
```

### result

```rbs
class Object < BasicObject
  def no_expr_case: (Integer x) -> ("negative" | "positive" | "zero")
end
```

## case when narrows class before method call

### update

```ruby
class Formatter
  #: (untyped) -> String
  def format(x)
    case x
    when Integer
      x.to_s
    when String
      x.upcase
    else
      "unknown"
    end
  end
end
```

### result

```rbs
class Formatter
  def format: (untyped x) -> String
end
```

## case when narrowing changes method return

### update

```ruby
def describe(val)
  case val
  when Integer
    val + 1
  when String
    val.length
  else
    nil
  end
end
describe(42)
```

### result

```rbs
class Object < BasicObject
  def describe: (Integer val) -> Integer?
end
```

## case when with multiple conditions

### update

```ruby
def numeric_check(x)
  case x
  when Integer, Float
    x.to_f
  when String
    x.to_f
  end
end
numeric_check(42)
```

### result

```rbs
class Object < BasicObject
  def numeric_check: (Integer x) -> Float?
end
```

## `when ... then` form

### update

```ruby
def case_then(x)
  case x
  when 1 then :one
  when 2 then :two
  else :other
  end
end
```

### result

```rbs
class Object < BasicObject
  def case_then: (untyped x) -> (:one | :other | :two)
end
```

## case when narrows record discriminant

### update

```ruby
class RecordDiscriminantCase
  #: ({ kind: :text, value: String } | { kind: :count, value: Integer }) -> (Integer | Symbol)?
  def value(x)
    case x[:kind]
    when :text
      x[:value].to_sym
    when :count
      x[:value] + 1
    else
      nil
    end
  end
end
```

### result

```rbs
class RecordDiscriminantCase
  def value: (({ kind: :count, value: Integer } | { kind: :text, value: String }) x) -> (Integer | Symbol)?
end
```

## case when narrowing resolves RBS method return

### update

```rbs
class Stringifiable
  def stringify: -> String
end

class Numerable
  def to_num: -> Integer
end
```

```ruby
def convert(obj)
  case obj
  when Stringifiable
    obj.stringify
  when Numerable
    obj.to_num
  else
    nil
  end
end
```

### result

```rbs
class Object < BasicObject
  def convert: (untyped obj) -> (Integer | String)?
end
```

## Pattern matching in Ruby 3+

### update

```ruby
def pattern_match(x)
  case x
  in Integer
    "integer"
  in String
    "string"
  end
end
pattern_match(42)
```

### result

```rbs
class Object < BasicObject
  def pattern_match: (Integer x) -> ("integer" | "string")?
end
```

## Use pattern match captured local in return

### update

```ruby
def capture_match(x)
  if x in Integer => captured
    captured
  end
end

capture_match(1)
```

### result

```rbs
class Object < BasicObject
  def capture_match: (Integer x) -> Integer?
end
```

## Case when reassigns a shared local

### update

```ruby
def test(type, val)
  case type
  when :int
    val = val.to_i
  when :sym
    val = val.to_sym
  else
    val = val
  end
  val
end

test(:int, "42")
test(:sym, "hello")
```

### result

```rbs
class Object < BasicObject
  def test: (Symbol type, String val) -> (Integer | String | Symbol)
end
```

## Case when unions Integer or String

### update

```ruby
def foo(x)
  case x
  when Integer, String
    x
  else
    :other
  end
end

foo(1)
foo("s")
foo(:a)
```

### result

```rbs
class Object < BasicObject
  def foo: ((Integer | String | Symbol) x) -> (Integer | String | :other)
end
```
