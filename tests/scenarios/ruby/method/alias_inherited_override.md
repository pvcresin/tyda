# Ruby / Method / Alias Inherited Override

## Capture an inherited method before a later override

```ruby
class BaseModel
  def reload = self
end

class ChildModel < BaseModel
  alias base_reload reload

  def reload(*)
    base_reload(*)
  end

  def refresh
    reload
  end
end
```

### result

```rbs
class BaseModel
  def reload: -> BaseModel
end

class ChildModel < BaseModel
  def reload: (*untyped) -> BaseModel
  def refresh: -> BaseModel
  alias base_reload reload
end
```
