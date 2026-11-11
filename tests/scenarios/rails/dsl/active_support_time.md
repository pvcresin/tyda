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

## Resolve Time.zone factories and calendar helpers

### update

```ruby
class Clock
  def zone = Time.zone

  def now = Time.zone.now

  def today = Time.zone.today

  def parsed = Time.zone.parse("2024-01-01")

  def tokyo = Time.current.in_time_zone("Tokyo")

  def month_start = Date.current.beginning_of_month

  def week_end = Date.current.end_of_week

  def day_end = Time.current.end_of_day

  def yesterday_at = Time.current.ago(1.day)

  def label = Time.current.to_fs(:db)
end
```

### result

```rbs
class Clock
  def zone: -> ActiveSupport::TimeZone
  def now: -> ActiveSupport::TimeWithZone
  def today: -> Date
  def parsed: -> ActiveSupport::TimeWithZone?
  def tokyo: -> ActiveSupport::TimeWithZone
  def month_start: -> Date
  def week_end: -> Date
  def day_end: -> ActiveSupport::TimeWithZone
  def yesterday_at: -> ActiveSupport::TimeWithZone
  def label: -> String
end
```

## Date.yesterday and Date.tomorrow return Date

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Calendar
  def yesterday = Date.yesterday

  def tomorrow = Date.tomorrow
end
```

### result

```rbs
class Calendar
  def yesterday: -> Date
  def tomorrow: -> Date
end
```

## Duration arithmetic with another duration returns a duration

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Timer
  def total = 1.day + 2.hours

  def remaining = 2.weeks - 1.day
end
```

### result

```rbs
class Timer
  def total: -> ActiveSupport::Duration
  def remaining: -> ActiveSupport::Duration
end
```

## Duration unit methods delegate to the value and return a duration

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Backoff
  INITIAL = 1.minute

  def delay(count) = (INITIAL * (2**count)).seconds

  def seconds_total = INITIAL.in_seconds
end
```

### result

```rbs
class Backoff
  INITIAL: ActiveSupport::Duration

  def delay: (untyped count) -> ActiveSupport::Duration
  def seconds_total: -> Integer
end
```

## Adding a duration to TimeWithZone returns TimeWithZone

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Clock
  def later = Time.current + 1.hour

  def zone_later = Time.zone.now + 1.hour
end
```

### result

```rbs
class Clock
  def later: -> ActiveSupport::TimeWithZone
  def zone_later: -> ActiveSupport::TimeWithZone
end
```

## TimeZone at and local return non-nil TimeWithZone

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Clock
  def epoch = Time.zone.at(0)

  def new_year = Time.zone.local(2024, 1, 1)
end
```

### result

```rbs
class Clock
  def epoch: -> ActiveSupport::TimeWithZone
  def new_year: -> ActiveSupport::TimeWithZone
end
```
