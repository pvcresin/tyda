# Known issues / Active Record Persistence

## increment! and decrement! return the record

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end

class User < ActiveRecord::Base
  def bump = increment!(:visits)

  def drop = decrement!(:visits)
end
```

### result

```rbs
class User < ActiveRecord::Base
  def bump: -> User
  def drop: -> User
end
```
