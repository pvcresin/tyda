# Ruby / Method / Keyword Method Return Ref

## Resolve keyword arg identity method through another method

### update

```ruby
class Formatter
  def fetch(name:) = name

  def build = fetch(name: "daily")
end
```

### result

```rbs
class Formatter
  def fetch: (name: String) -> String
  def build: -> String
end
```

## Propagate keyword arg type across classes

### update

```ruby
class Service
  def call(status:) = status
end

class Controller
  def index
    svc = Service.new
    svc.call(status: "ok")
  end
end
```

### result

```rbs
class Controller
  def index: -> String
end

class Service
  def call: (status: String) -> String
end
```

## Infer return from method with multiple keyword args

### update

```ruby
class Builder
  def build(name:, count:) = name

  def run = build(name: "test", count: 5)
end
```

### result

```rbs
class Builder
  def build: (name: String, count: Integer) -> String
  def run: -> String
end
```
