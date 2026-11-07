# Ruby / Control / Array Element Facts

## Size guard removes nil from the first element

```yaml
known_issue: true
```

### update

```ruby
class ArraySizeGuardFact
  #: () -> Array[Integer]
  def ints = []

  def index_after_size_guard
    values = ints
    return :empty unless values.size >= 1
    values[0]
  end

  def slice_after_size_guard
    values = ints
    return :empty unless values.size >= 1
    values[1..]
  end
end
```

### result

```rbs
class ArraySizeGuardFact
  def ints: -> Array[Integer]
  def index_after_size_guard: -> (Integer | :empty)
  def slice_after_size_guard: -> (:empty | Array[Integer])
end
```

## Truthy index guard removes nil and false from the same index

```yaml
known_issue: true
```

### update

```ruby
class ArrayIndexTruthyFact
  #: () -> Array[Integer]
  def ints = []

  #: () -> Array[Integer | false]
  def maybe_false = []

  def nil_removed
    values = ints
    values[0] ? values[0] : :none
  end

  def false_removed
    values = maybe_false
    values[0] ? values[0] : :none
  end
end
```

### result

```rbs
class ArrayIndexTruthyFact
  def ints: -> Array[Integer]
  def maybe_false: -> Array[Integer | false]
  def nil_removed: -> (Integer | :none)
  def false_removed: -> (Integer | :none)
end
```

## Array element facts end when the array can change

### update

```ruby
class ArrayElementFactControls
  #: () -> Array[Integer]
  def ints = []

  def unguarded
    values = ints
    values[0]
  end

  def cleared_after_guard
    values = ints
    return :empty unless values.size >= 1
    values.clear
    values[0]
  end

  def cleared_through_alias
    values = ints
    return :empty unless values.size >= 1
    other = values
    other.clear
    values[0]
  end

  def overridden_size
    values = ints
    def values.size = 1
    return :empty unless values.size >= 1
    values[0]
  end

  def element_nil_kept
    values = [nil, 1]
    return :empty unless values.size >= 1
    values[0]
  end
end
```

### result

```rbs
class ArrayElementFactControls
  def ints: -> Array[Integer]
  def unguarded: -> Integer?
  def cleared_after_guard: -> :empty?
  def cleared_through_alias: -> :empty?
  def overridden_size: -> (Integer | :empty)?
  def element_nil_kept: -> :empty?
end
```

## Array element facts end when a helper can change the array

### update

```ruby
class ArrayElementFactEscapes
  #: () -> Array[Integer]
  def ints = []

  def clear_it(values)
    values.clear
  end

  def through_wrapper
    values = ints
    return :empty unless values.size >= 1
    clear_it(values)
    values[0]
  end

  def through_ivar_alias
    values = ints
    @alias_values = values
    return :empty unless values.size >= 1
    clear_alias
    values[0]
  end

  def clear_alias
    @alias_values.clear
  end
end
```

### result

```rbs
class ArrayElementFactEscapes
  def ints: -> Array[Integer]
  def clear_it: (Array[Integer] values) -> [ ]
  def through_wrapper: -> (Integer | :empty)?
  def through_ivar_alias: -> (Integer | :empty)?
  def clear_alias: -> [ ]
end
```
