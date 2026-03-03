# Rails / Active Support / Hash With Indifferent Access

## Resolve hash accessors

### update

```rbs
class Store
  def options: -> HashWithIndifferentAccess
end

class HashWithIndifferentAccess
end
```

```ruby
class Store
  def read_option = options[:name].to_s

  def fetch_option = options.fetch(:name).to_s

  def slice_options = options.slice(:name)
end
```

### result

```rbs
class Store
  def read_option: -> String
  def fetch_option: -> String
  def slice_options: -> HashWithIndifferentAccess
end
```
