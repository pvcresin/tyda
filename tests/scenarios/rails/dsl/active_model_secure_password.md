# Rails / DSL / Active Model Secure Password

## has_secure_password and confirmation validation

### update

```ruby
class User
  has_secure_password
  validates :email, confirmation: true
end
```

### result

```rbs
class User
  def password: -> String
  def password=: (String password) -> String
  def password_confirmation: -> String
  def password_confirmation=: (String password_confirmation) -> String
  def password_challenge: -> String
  def password_challenge=: (String password_challenge) -> String
  def password_salt: -> String
  def authenticate: (String unencrypted_password) -> (bool | self)
  def authenticate_password: (String unencrypted_password) -> (bool | self)
  def self.authenticate_by: (untyped attributes) -> self?
  def email_confirmation: -> untyped
  def email_confirmation=: (untyped email_confirmation) -> untyped
end
```

## Generate authenticate_by only for Rails 7.1 and later

```yaml
rails_version: 7.0.0
```

### update

```ruby
class User
  has_secure_password
end
```

### result

```rbs
class User
  def password: -> String
  def password=: (String password) -> String
  def password_confirmation: -> String
  def password_confirmation=: (String password_confirmation) -> String
  def password_challenge: -> String
  def password_challenge=: (String password_challenge) -> String
  def password_salt: -> String
  def authenticate: (String unencrypted_password) -> (bool | self)
  def authenticate_password: (String unencrypted_password) -> (bool | self)
end
```

## Generate authenticate_by in Rails 7.1

```yaml
rails_version: 7.1.0
```

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class User < ApplicationRecord
  has_secure_password
end
```

### result

```rbs
class User < ApplicationRecord
  def password: -> String
  def password=: (String password) -> String
  def password_confirmation: -> String
  def password_confirmation=: (String password_confirmation) -> String
  def password_challenge: -> String
  def password_challenge=: (String password_challenge) -> String
  def password_salt: -> String
  def authenticate: (String unencrypted_password) -> (bool | self)
  def authenticate_password: (String unencrypted_password) -> (bool | self)
  def self.authenticate_by: (untyped attributes) -> self?
end
```
