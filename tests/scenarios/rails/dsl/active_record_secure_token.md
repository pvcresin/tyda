# Rails / DSL / Active Record Secure Token

## Generate regenerate methods from has_secure_token

```yaml
rails_version: 5.0.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class User < ApplicationRecord
  has_secure_token :auth_token
end
```

### result

```rbs
class User < ApplicationRecord
  def regenerate_auth_token: -> bool
end
```

## has_secure_token uses token as default attribute name

```yaml
rails_version: 5.0.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class ApiKey < ApplicationRecord
  has_secure_token
end
```

### result

```rbs
class ApiKey < ApplicationRecord
  def regenerate_token: -> bool
end
```

## Ignore has_secure_token in Rails 4.x

```yaml
rails_version: 4.2.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class User < ApplicationRecord
  has_secure_token :auth_token
end
```

### result

```rbs
```
