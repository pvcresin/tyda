# Ruby / Class / Constant Path Head Through Include

## Qualified path head resolves through an included module's nested namespace

### update

```ruby
module Formats
  module MyBridge
    HEADERS = ["a", "b"].freeze
    COL_NUMBERS = { x: 0 }.freeze
  end
end

class Reader
  include Formats

  def headers = MyBridge::HEADERS
  def col = MyBridge::COL_NUMBERS
end
```

### result

```rbs
module Formats::MyBridge
  HEADERS: ["a", "b"]
  COL_NUMBERS: { x: 0 }
end

class Reader
  include Formats

  def headers: -> ["a", "b"]
  def col: -> { x: 0 }
end
```

## Bare reference to an included module's nested namespace resolves to its singleton

### update

```ruby
module Formats
  module MyBridge
  end
end

class Reader
  include Formats

  def mod = MyBridge
end
```

### result

```rbs
class Reader
  include Formats

  def mod: -> singleton(Formats::MyBridge)
end
```
