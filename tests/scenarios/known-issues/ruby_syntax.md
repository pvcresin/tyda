# Known Issues / Ruby Syntax

## A case when range keeps the matching branch for an Integer local

### update

```rbs
class RangeWhen
  def score: -> Integer
end
```

```ruby
class RangeWhen
  def label
    value = score
    case value
    when 1..5 then :low
    else :other
    end
  end
end
```

### result

```rbs
class RangeWhen
  def label: -> (:low | :other)
end
```

## A return on the right of or in an assignment adds nil to the method

### update

```rbs
class OrReturnAssign
  def name: -> String?
end
```

```ruby
class OrReturnAssign
  def required_name
    value = name || return
    value.length
  end
end
```

### result

```rbs
class OrReturnAssign
  def required_name: -> Integer?
end
```

## An instance variable read after an append keeps the element

### update

```ruby
class AppendInPlace
  def run
    @items = [0]
    @items << 1
    @items
  end
end
```

### result

```rbs
class AppendInPlace
  def run: -> [0, 1]
end
```

## An instance variable Hash keeps a key written in the same method

### update

```ruby
class IndexWriteInPlace
  def run
    @table = {}
    @table[:a] = 1
    @table
  end
end
```

### result

```rbs
class IndexWriteInPlace
  def run: -> Hash[:a, 1]
end
```
