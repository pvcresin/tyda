# Ruby / Method / Kernel Function Receivers

## An explicit receiver uses its own declaration, not a Kernel function

```ruby
class Library
  def load_data(data) = Marshal.load(data)
  def seeded = Random.new(1).rand(100)
  def input_line = ARGF.lineno
end
```

### result

```rbs
class Library
  def load_data: (untyped data) -> untyped
  def seeded: -> Integer
  def input_line: -> Integer
end
```
