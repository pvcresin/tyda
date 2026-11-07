# Rails / DSL / Rack::Attack

## Resolve request methods through the Rack::Request superclass

### update

```ruby
class Rack::Attack::Request
  def query_params = params
end
```

### result

```rbs
class Rack::Attack::Request < Rack::Request
  def query_params: -> untyped
end

class Rack::Request
  def params: -> untyped
end
```
