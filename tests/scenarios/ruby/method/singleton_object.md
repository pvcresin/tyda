# Ruby / Method / Singleton Object

## Singleton method on an object local keeps its identity

### update

```ruby
def build
  item = Object.new
  def item.value = :ready
  item.value
end
```

### result

```rbs
class Object
  def build: -> :ready
end
```

## Dynamic singleton definition resolves a static method name

### update

```ruby
class Registry
  define_singleton_method(:fetch) do |key|
    key
  end

  def self.read = fetch(:value)
end
```

### result

```rbs
class Registry
  def self.fetch: (Symbol key) -> Symbol
  def self.read: -> Symbol
end
```
