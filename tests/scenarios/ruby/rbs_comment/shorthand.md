# Ruby / RBS Comment / Shorthand

## Type annotation for args and return

### update

```ruby
#: (String) -> Integer
def foo(x) = x.to_i
```

### result

```rbs
class Object
  def foo: (String x) -> Integer
end
```

## @rbs method type comment

### update

```ruby
# @rbs (String) -> Integer
def parse_count(value) = value.to_i
```

### result

```rbs
class Object
  def parse_count: (String value) -> Integer
end
```

## @rbs method type overload

### update

```ruby
# @rbs (Integer) -> String | (String) -> Integer
def flip(value) = value.is_a?(Integer) ? value.to_s : value.to_i
```

### result

```rbs
class Object
  def flip: (Integer value) -> String
          | (String value) -> Integer
end
```

## @rbs method type overload across comment lines

### update

```ruby
# @rbs (String) -> String
# @rbs (Integer) -> Integer | (Symbol) -> Symbol
def pick_value(value) = value
```

### result

```rbs
class Object
  def pick_value: (String value) -> String
                | (Integer value) -> Integer
                | (Symbol value) -> Symbol
end
```

## @rbs method type block overload

### update

```ruby
class InlineBlockOverload
  # @rbs () -> Enumerator[String, Array[Integer]]
  # @rbs () { (String) -> Integer } -> Array[Integer]
  def values(&block)
    if block
      [block.call("x")]
    else
      [].each
    end
  end
end
```

### result

```rbs
class InlineBlockOverload
  def values: -> Enumerator[String, Array[Integer]]
            | { (String) -> Integer } -> Array[Integer]
end
```

## @rbs method type overload with alias

### update

```rbs
type inline_label = String
```

```ruby
class InlineAliasOverload
  # @rbs (Integer) -> Integer | (inline_label) -> Symbol
  def cast(value)
    value.is_a?(String) ? :label : value
  end
end

def inline_alias_overload_string
  InlineAliasOverload.new.cast("name")
end

def inline_alias_overload_integer
  InlineAliasOverload.new.cast(1)
end
```

### result

```rbs
class InlineAliasOverload
  def cast: (Integer value) -> Integer
          | (String value) -> Symbol
end

class Object
  def inline_alias_overload_string: -> Symbol
  def inline_alias_overload_integer: -> Integer
end
```

## @rbs return annotation

### update

```ruby
# @rbs return: String
def annotated_return(value)
  value
end
```

### result

```rbs
class Object
  def annotated_return: (untyped value) -> String
end
```

## @rbs parameter annotations

### update

```ruby
# @rbs name: String
# @rbs count: Integer
# @rbs return: String
def inline_param_label(name, count:) = "#{name}: #{count}"
```

### result

```rbs
class Object
  def inline_param_label: (String name, count: Integer) -> String
end
```

## @rbs block annotation with trailing return

### update

```ruby
class InlineBlock
  # @rbs &block: (String) -> Integer
  def each_name(&block) #: void
    block.call("x")
  end
end
```

### result

```rbs
class InlineBlock
  def each_name: { (String) -> Integer } -> void
end
```

## Type annotation for multiple args

### update

```ruby
#: (String, Integer) -> bool
def bar(name, age) = true
```

### result

```rbs
class Object
  def bar: (String name, Integer age) -> bool
end
```

## Type annotation with no args

### update

```ruby
#: () -> String
def greeting = "hello"
```

### result

```rbs
class Object
  def greeting: -> String
end
```

## Type annotation with void return

### update

```ruby
#: (String) -> void
def puts_name(name) = puts(name)
```

### result

```rbs
class Object
  def puts_name: (String name) -> void
end
```

## Type annotation inside class

### update

```ruby
class User
  #: (String) -> String
  def greet(name) = "Hello, #{name}"
end
```

### result

```rbs
class User
  def greet: (String name) -> String
end
```

## Array and Hash type annotation

### update

```ruby
#: (Array) -> Hash
def convert(list) = {}
```

### result

```rbs
class Object
  def convert: (Array list) -> Hash
end
```

## Named parameter form

### update

```ruby
#: (String name, Integer age) -> bool
def check(name, age) = true
```

### result

```rbs
class Object
  def check: (String name, Integer age) -> bool
end
```

## Union type

### update

```ruby
#: (String | Integer) -> bool
def accept_either(x) = true
```

### result

```rbs
class Object
  def accept_either: ((Integer | String) x) -> bool
end
```

## Generic type Array[Integer]

### update

```ruby
#: (Array[Integer]) -> Integer
def sum(nums) = 0
```

### result

```rbs
class Object
  def sum: (Array[Integer] nums) -> Integer
end
```

## Optional type with nil

### update

```ruby
#: (String?) -> void
def maybe_print(s)
end
```

### result

```rbs
class Object
  def maybe_print: (String? s) -> void
end
```

## Complex return type Array[String]

### update

```ruby
#: () -> Array[String]
def names = []
```

### result

```rbs
class Object
  def names: -> Array[String]
end
```

## Hash type annotation with type params

### update

```ruby
#: (Hash[Symbol, String]) -> void
def process_options(opts)
end
```

### result

```rbs
class Object
  def process_options: (Hash[Symbol, String] opts) -> void
end
```

## Record return type

### update

```ruby
#: () -> { name: String, age: Integer }
def person_record = { name: "Alice", age: 30 }
```

### result

```rbs
class Object
  def person_record: -> { name: String, age: Integer }
end
```

## nil return type

### update

```ruby
#: () -> nil
def return_nil
end
```

### result

```rbs
class Object
  def return_nil: -> nil
end
```

## untyped type

### update

```ruby
#: (untyped) -> untyped
def pass_through(x) = x
```

### result

```rbs
class Object
  def pass_through: (untyped x) -> untyped
end
```

## Union type with three or more members

### update

```ruby
#: (String | Integer | Symbol) -> bool
def accept_many(x) = true
```

### result

```rbs
class Object
  def accept_many: ((Integer | String | Symbol) x) -> bool
end
```

## Nested generic type

### update

```ruby
#: (Array[Array[Integer]]) -> Integer
def deep_sum(matrix) = 0
```

### result

```rbs
class Object
  def deep_sum: (Array[Array[Integer]] matrix) -> Integer
end
```

## Hash[String, Array[Integer]] type

### update

```ruby
#: (Hash[String, Array[Integer]]) -> void
def process_grouped(data)
end
```

### result

```rbs
class Object
  def process_grouped: (Hash[String, Array[Integer]] data) -> void
end
```

## Generic class declaration with #[Elem] comment

### update

`sorbet/config`

```ruby
.
```

```ruby
class MyList #[Elem]
  def first = nil
end
```

### result

```rbs
class MyList[Elem]
  def first: -> nil
end
```
