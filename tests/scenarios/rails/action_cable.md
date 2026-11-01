# Rails / Action Cable

## Infer channel params

### update

```ruby
class ActionCable::Channel::Base; end

class NotificationsChannel < ActionCable::Channel::Base
  def channel_params = params
end
```

### result

```rbs
class NotificationsChannel < ActionCable::Channel::Base
  def channel_params: -> Hash[untyped, untyped]
end
```
