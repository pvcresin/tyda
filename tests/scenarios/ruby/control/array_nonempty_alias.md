# Ruby / Control / Array Nonempty Alias

## Preserve nonempty knowledge through a local alias

### update

```ruby
def first_after_nonempty_guard
  values = [1].map { |value| value }
  return :empty if values.empty?
  values[0]
end

def first_after_nonempty_guard_alias
  values = [1].map { |value| value }
  return :empty if values.empty?
  alias_values = values
  alias_values[0]
end
```

### result

```rbs
class Object < BasicObject
  def first_after_nonempty_guard: -> 1
  def first_after_nonempty_guard_alias: -> 1
end
```

## Source-defined Array#empty? controls branch selection

### update

```ruby
class Array
  def empty? = true
end

class OverrideProbe
  def empty_result = [1].empty?
  def empty_branch = [1].empty? ? :empty : :nonempty
end
```

### result

```rbs
class Array[unchecked out E]
  def empty?: -> true
end

class OverrideProbe
  def empty_result: -> true
  def empty_branch: -> :empty
end
```

## Size and index guards refine repeated array reads

### update

```ruby
class ArrayIndexGuards
  def first_after_size_guard
    values = [1, 2, 3]
    if values.size >= 1
      values[0].succ
    else
      raise ArgumentError
    end
  end

  def slice_after_size_guard
    values = [1, 2, 3]
    if values.size >= 1
      values[1..] + values
    else
      raise ArgumentError
    end
  end

  def first_after_index_guard
    values = [1, 2, 3]
    if values[0]
      values[0].succ
    else
      raise ArgumentError
    end
  end
end
```

### result

```rbs
class ArrayIndexGuards
  def first_after_size_guard: -> Integer
  def slice_after_size_guard: -> Array[1 | 2 | 3]
  def first_after_index_guard: -> Integer
end
```


## Array reader guard follows a preceding type check

```rbs
class ExternalArrayHolder
  attr_reader values: Array[Integer]
end
```

```ruby
class ArrayReaderBranchGuard
  def first(value)
    if value.is_a?(ExternalArrayHolder) && value.values[0]
      value.values[0].succ
    else
      nil
    end
  end
end
```

### result

```rbs
class ArrayReaderBranchGuard
  def first: (untyped value) -> Integer?
end
```

## Truthy array element guards exclude false and nil

### update

```ruby
class ArrayIndexTruthyGuard
  #: (Array[Integer | false]) -> Integer?
  def first_truthy(values)
    if values[0]
      values[0].succ
    end
  end
end
```

### result

```rbs
class ArrayIndexTruthyGuard
  def first_truthy: (Array[Integer | false] values) -> Integer?
end
```
