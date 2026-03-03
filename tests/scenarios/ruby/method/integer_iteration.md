# Ruby / Method / Integer iteration helpers

## `Integer#times`, `upto`, `downto`, `step` produce Integer enumerators

### update

```ruby
class Probe
  def times_to_array   = 3.times.to_a
  def times_map_string = 3.times.map { |i| i.to_s }
  def upto_to_array    = 1.upto(5).to_a
  def downto_to_array  = 5.downto(1).to_a
  def step_to_array    = 1.step(10, 2).to_a
end
```

### result

```rbs
class Probe
  def times_to_array: -> Array[Integer]
  def times_map_string: -> Array[String]
  def upto_to_array: -> Array[Integer]
  def downto_to_array: -> Array[Integer]
  def step_to_array: -> Array[Integer]
end
```
