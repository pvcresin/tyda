# Ruby / Control / If Else

## if else returns different types

### update

```ruby
def foo(x)
  if x
    1
  else
    "hello"
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> (1 | "hello")
end
```

## if else returns same type

### update

```ruby
def foo(x)
  if x
    1
  else
    2
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> (1 | 2)
end
```

## if without else unions with nil

### update

```ruby
def foo(x)
  if x
    1
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> 1?
end
```

## local assigned on one branch is nilable after the branch

### update

```ruby
def conditional_local(flag)
  if flag
    value = :present
  end
  value
end
```

### result

```rbs
class Object < BasicObject
  def conditional_local: (untyped flag) -> :present?
end
```

## local assigned on both branches joins branch types

### update

```ruby
def complete_branch_local(flag)
  if flag
    value = 1
  else
    value = "other"
  end
  value
end
```

### result

```rbs
class Object < BasicObject
  def complete_branch_local: (untyped flag) -> (1 | "other")
end
```

## assignment on an exiting branch does not reach the following read

### update

```ruby
def branch_local_after_return(flag)
  if flag
    value = :assigned
    return :returned
  end
  value
end
```

### result

```rbs
class Object < BasicObject
  def branch_local_after_return: (untyped flag) -> :returned?
end
```

## complete assignments across elsif exits stay non-nil

### update

```ruby
def body_after_range_branches(ranges, head)
  if ranges.nil?
    ranges = [:single]
  elsif ranges.empty?
    return :empty
  else
    partial = true
    body = :partial
  end

  if head
    body = :head
  elsif !partial
    body = :full
  end

  body
end

def maybe_ranges(flag)
  flag ? [0..10] : nil
end

class RangeResponseIterator
  def bytesize
    10
  end
end

def body_after_typed_range_branches(flag, head)
  ranges = maybe_ranges(flag)
  if ranges.nil?
    ranges = [:single]
  elsif ranges.empty?
    return :empty
  else
    partial = true
    body = :partial
  end

  if head
    body = :head
  elsif !partial
    body = :full
  end

  body
end

def iterator_body_after_typed_range_branches(flag, head)
  ranges = maybe_ranges(flag)
  if ranges.nil?
    ranges = [0..10]
  elsif ranges.empty?
    return :empty
  else
    partial = true
    body = RangeResponseIterator.new
  end

  if head
    body = []
  elsif !partial
    body = :full
  end

  body
end

```

### result

```rbs
class Object < BasicObject
  def body_after_range_branches: (untyped ranges, untyped head) -> (:empty | :full | :head | :partial)
  def maybe_ranges: (untyped flag) -> [Range[Integer]]?
  def body_after_typed_range_branches: (untyped flag, untyped head) -> (:full | :head | :partial)
  def iterator_body_after_typed_range_branches: (untyped flag, untyped head) -> (:full | RangeResponseIterator | [ ])
end

class RangeResponseIterator
  def bytesize: -> 10
end
```

## Preserve tuple correlation across nested branches

```yaml
known_issue: true
```

### update

```ruby
def maybe_ranges(flag)
  flag ? [0..10] : nil
end

class RangeResponseIterator
  def bytesize
    10
  end
end

def range_response_after_nested_branch(flag, head, single_range)
  ranges = maybe_ranges(flag)
  status = 200
  headers = {}
  if ranges.nil?
    ranges = [0..10]
  elsif ranges.empty?
    return :empty
  else
    partial = true
    if single_range
      headers[:content_range] = :single
    else
      headers[:content_type] = :multiple
    end
    status = 206
    body = RangeResponseIterator.new
  end

  if head
    body = []
  elsif !partial
    body = :full
  end

  [status, headers, body]
end
```

### result

```rbs
class Object < BasicObject
  def maybe_ranges: (untyped flag) -> [Range[Integer]]?
  def range_response_after_nested_branch: (untyped flag, untyped head, untyped single_range) -> ([200, Hash[untyped, untyped], :full] | [200, Hash[untyped, untyped], [ ]] | [206, Hash[:content_range, :single] | Hash[:content_type, :multiple], RangeResponseIterator] | [206, Hash[:content_range, :single] | Hash[:content_type, :multiple], [ ]])
end

class RangeResponseIterator
  def bytesize: -> 10
end
```

## Array.new with a literal size of one is nonempty

### update

```ruby
def first_range_after_empty_guard
  ranges = Array.new(1, 0..10)
  return :empty if ranges.empty?
  ranges[0]
end
```

### result

```rbs
class Object < BasicObject
  def first_range_after_empty_guard: -> Range[Integer]
end
```

## Clear updates the local array value

### update

```ruby
def first_generic_array_after_clear
  values = [1].map { |value| value }
  return :empty if values.empty?
  values.clear
  values[0]
end

first_generic_array_after_clear
```

### result

```rbs
class Object < BasicObject
  def first_generic_array_after_clear: -> nil
end
```

## Invalid Array#clear arguments do not update instance variables

### update

```ruby
class InvalidArrayClear
  def first_value
    @values = [1]
    begin
      @values.clear(:invalid)
    rescue ArgumentError
    end
    @values.first
  end
end
```

### result

```rbs
class InvalidArrayClear
  def first_value: -> 1?
end
```

## Array#clear ignores blocks and empties a local instance variable

### update

```ruby
class ClearIvarWithBlock
  def first_value
    @values = [1]
    @values.clear { :ignored }
    @values[0]
  end
end
```

### result

```rbs
class ClearIvarWithBlock
  def first_value: -> nil
end
```

## Correlate a one-sided local with a primitive branch value

### update

```ruby
class MissingLocalCorrelation
  def safe
    [180, 3000].each do |frame|
      break_end = frame * 2 if frame == 3000
      if frame == 180
        :first
      else
        break_end + 10
      end
    end
  end

  def unsafe
    [180, 3600, 3000].each do |frame|
      break_end = frame * 2 if frame == 3000
      if frame == 180
        :first
      else
        break_end + 10
      end
    end
  end
end
```

### result

```rbs
class MissingLocalCorrelation
  def safe: -> [180, 3000]
  def unsafe: -> [180, 3600, 3000]
end
```
