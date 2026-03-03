# Rails / DSL / ActiveSupport Time

## Resolve Time.current and TimeWithZone surface

### update

```ruby
class Clock
  def now = Time.current

  def midnight = Time.current.beginning_of_day

  def stamp = Time.current.iso8601

  def epoch = Time.current.to_i

  def tomorrow_noon = Time.current.tomorrow.noon

  def back_to_utc = Time.current.utc
end
```

### result

```rbs
class Clock
  def now: -> ActiveSupport::TimeWithZone
  def midnight: -> ActiveSupport::TimeWithZone
  def stamp: -> String
  def epoch: -> Integer
  def tomorrow_noon: -> ActiveSupport::TimeWithZone
  def back_to_utc: -> Time
end
```

## Resolve Date.current

### update

```ruby
class Calendar
  def today = Date.current
end
```

### result

```rbs
class Calendar
  def today: -> Date
end
```

