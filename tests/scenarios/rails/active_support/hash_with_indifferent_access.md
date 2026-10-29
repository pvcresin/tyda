# Rails / Active Support / Hash With Indifferent Access

## Resolve hash accessors

```yaml
known_issue: true
```

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
  def read_option: -> untyped
  def fetch_option: -> untyped
  def slice_options: -> HashWithIndifferentAccess
end
```
