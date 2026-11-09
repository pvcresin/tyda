# Known Issues / Flow Guards

## A repeated ENV read keeps its truthy guard

```ruby
class RepeatedEnvRead
  def run
    consume(ENV["TOKEN"]) if ENV["TOKEN"]
  end

  def consume(value) = value
end
```

### result

```rbs
class RepeatedEnvRead
  def run: -> String?
  def consume: (String value) -> String
end
```
