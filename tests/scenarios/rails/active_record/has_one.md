# Rails / Active Record / Has One

## Generate has_one association methods

### update

```ruby
class A
  has_one :item
end
```

### result

```rbs
class A
  def item: -> Item?
  def item=: (Item item) -> Item
  def build_item: -> Item
  def create_item: -> Item
end
```

## has_one class_name sets the class

### update

```ruby
class A
  has_one :item, class_name: "B"
end
```

### result

```rbs
class A
  def item: -> B?
  def item=: (B item) -> B
  def build_item: -> B
  def create_item: -> B
end
```
