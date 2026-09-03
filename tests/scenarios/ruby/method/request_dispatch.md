# Ruby / Method / Request Dispatch

## Dispatches a request-shaped record

### update

```ruby
class Dispatcher
  def call(message)
    case message[:method]
    when "read"
      message[:id]
    else
      nil
    end
  end
end
```

### result

```rbs
class Dispatcher
  def call: (untyped message) -> (nil | untyped)
end
```

## Dispatches a concrete request record

### update

```ruby
class Dispatcher
  def call(message)
    case message[:method]
    when "read"
      message[:id]
    else
      nil
    end
  end
end

def read = Dispatcher.new.call({ method: "read", id: 1 })
```

### result

```rbs
class Dispatcher
  def call: ({ method: String, id: Integer } message) -> Integer?
end

class Object < BasicObject
  def read: -> Integer?
end
```

## Request object keeps keyword constructor types

### update

```ruby
class Result
  def initialize(id:, response:)
    @id = id
    @response = response
  end

  def body = { id: @id, response: @response }
end

def build_result = Result.new(id: 1, response: "ok").body
```

### result

```rbs
class Object < BasicObject
  def build_result: -> { id: 1, response: "ok" }
end

class Result
  def initialize: (id: Integer, response: String) -> void
  def body: -> { id: 1, response: "ok" }
end
```
