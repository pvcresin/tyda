# Known Issues / Ruby Syntax

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
