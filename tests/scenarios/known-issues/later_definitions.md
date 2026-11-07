# Known Issues / Later Definitions

## A self call keeps the guarded return of a class defined later

```ruby
class Finder
  def conversation(uri)
    return unless uri.start_with?("x")
    Conversation.lookup(uri)
  end

  def direct(uri) = conversation(uri)
end

class Conversation
  def self.lookup(uri) = new
end
```

### result

```rbs
class Conversation
  def self.lookup: (untyped uri) -> Conversation
end

class Finder
  def conversation: (untyped uri) -> Conversation?
  def direct: (untyped uri) -> Conversation?
end
```

## A constant defined later types the expressions that use it

```ruby
module Clock
  class Timer
    FIRST, SECOND = (1..2).map { |i| i * RATE }
    SCALED = 5 * RATE

    def ticks = 5 * RATE
  end
end

module Clock
  RATE = 12
end
```

### result

```rbs
module Clock
  RATE: 12
end

class Clock::Timer
  FIRST: Integer?
  SECOND: Integer?
  SCALED: Integer

  def ticks: -> Integer
end
```
