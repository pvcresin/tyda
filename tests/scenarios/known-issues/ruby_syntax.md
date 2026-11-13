# Known Issues / Ruby Syntax

## A regexp match in an if condition sets numbered captures

### update

```rbs
class CaptureInCondition
  def line: -> String
end
```

```ruby
class CaptureInCondition
  def key
    if line =~ /\A(\w+)=/
      $1
    end
  end
end
```

### result

```rbs
class CaptureInCondition
  def key: -> String?
end
```

## A regexp match in a guard clause sets numbered captures

### update

```rbs
class CaptureInGuard
  def line: -> String
end
```

```ruby
class CaptureInGuard
  def key
    return unless /\A(\w+)=/ =~ line
    $1
  end
end
```

### result

```rbs
class CaptureInGuard
  def key: -> String?
end
```

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

## A case when else branch excludes the matched class

### update

```rbs
class KeyNormalizer
  def key: -> (String | Symbol)
end
```

```ruby
class KeyNormalizer
  def normalized_key
    value = key
    case value
    when Symbol then value.to_s
    else value
    end
  end
end
```

### result

```rbs
class KeyNormalizer
  def normalized_key: -> String
end
```

## A case when nil else branch excludes nil

### update

```rbs
class NilWhen
  def count: -> Integer?
end
```

```ruby
class NilWhen
  def count_or_zero
    value = count
    case value
    when nil then 0
    else value
    end
  end
end
```

### result

```rbs
class NilWhen
  def count_or_zero: -> Integer
end
```

## An alternative class pattern narrows its binding

### update

```rbs
class NumberPattern
  def value: -> (Integer | Float | String)
end
```

```ruby
class NumberPattern
  def number
    case value
    in Integer | Float => n then n
    else 0
    end
  end
end
```

### result

```rbs
class NumberPattern
  def number: -> (Integer | Float)
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

## A record key that is not a plain name is quoted

```ruby
class LinkedData
  def to_ld = { "@context": "https://schema.org", "data-id": 1 }
end
```

### result

```rbs
class LinkedData
  def to_ld: -> { :"@context" => "https://schema.org", :"data-id" => 1 }
end
```
