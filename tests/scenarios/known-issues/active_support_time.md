# Known issues / Active Support / Time

## Date day and second calculations return a zoned time

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Calendar
  def day_start = Date.current.beginning_of_day
  def day_midnight = Date.current.midnight
  def day_end = Date.current.end_of_day
  def day_middle = Date.current.middle_of_day
  def day_noon = Date.current.noon
  def before = Date.current.ago(60)
  def after = Date.current.since(60)
  def later = Date.current.in(60)
end
```

### result

```rbs
class Calendar
  def day_start: -> (ActiveSupport::TimeWithZone | Time)
  def day_midnight: -> (ActiveSupport::TimeWithZone | Time)
  def day_end: -> (ActiveSupport::TimeWithZone | Time)
  def day_middle: -> (ActiveSupport::TimeWithZone | Time)
  def day_noon: -> (ActiveSupport::TimeWithZone | Time)
  def before: -> (ActiveSupport::TimeWithZone | Time)
  def after: -> (ActiveSupport::TimeWithZone | Time)
  def later: -> (ActiveSupport::TimeWithZone | Time)
end
```

## Date has no hour minute or seconds since midnight helpers

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Calendar
  def hour_start = Date.current.beginning_of_hour
  def hour_end = Date.current.end_of_hour
  def minute_start = Date.current.beginning_of_minute
  def minute_end = Date.current.end_of_minute
  def seconds = Date.current.seconds_since_midnight
end
```

### result

```rbs
class Calendar
  def hour_start: -> untyped
  def hour_end: -> untyped
  def minute_start: -> untyped
  def minute_end: -> untyped
  def seconds: -> untyped
end
```

## DateTime seconds since midnight is Integer

```yaml
rails_version: 7.1.0
```

### update

```ruby
class Clock
  def date_time_seconds = DateTime.now.seconds_since_midnight
  def time_seconds = Time.now.seconds_since_midnight
end
```

### result

```rbs
class Clock
  def date_time_seconds: -> Integer
  def time_seconds: -> Float
end
```
