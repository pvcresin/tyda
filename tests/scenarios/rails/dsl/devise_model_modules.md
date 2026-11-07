# Rails / DSL / Devise model modules

## Register methods provided by enabled modules on that model

### update

```ruby
class ActiveRecord::Base
end

class User < ActiveRecord::Base
  devise :database_authenticatable, :confirmable,
    :two_factor_authenticatable, :two_factor_backupable
end

class PasswordUser < ActiveRecord::Base
  devise :database_authenticatable
end
```

### result

```rbs
class ActionController::Base
  def current_user: -> User
  def user_signed_in?: -> bool
  def authenticate_user!: -> void
  def user_session: -> untyped
  def current_password_user: -> PasswordUser
  def password_user_signed_in?: -> bool
  def authenticate_password_user!: -> void
  def password_user_session: -> untyped
end

class ActiveRecord::Base
  def self.devise: (*Symbol modules) -> void
end

class User < ActiveRecord::Base
  def confirmed?: -> bool
  def resend_confirmation_instructions: -> untyped
  def validate_and_consume_otp!: -> bool
  def invalidate_otp_backup_code!: -> bool
end
```
