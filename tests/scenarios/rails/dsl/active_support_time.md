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
