# Rails / DSL / Devise

## Infer controller helpers from devise model

### update

```ruby
class ActiveRecord::Base
end

class ActionController::Base
end

class ApplicationController < ActionController::Base
end

class User < ActiveRecord::Base
  devise :database_authenticatable, :recoverable
end

class UsersController < ApplicationController
  def show = current_user

  def require_login = authenticate_user!
end
```

### result

```rbs
class ActionController::Base
  def current_user: -> User
  def user_signed_in?: -> bool
  def authenticate_user!: -> void
  def user_session: -> untyped
end

class ActiveRecord::Base
  def self.devise: (*Symbol modules) -> void
end

class UsersController < ApplicationController
  def show: -> User
  def require_login: -> void
end
```

## Infer controller helpers from devise in a concern included block

### update

```ruby
class ActiveRecord::Base
end

class ActionController::Base
end

class ApplicationController < ActionController::Base
end

module Authenticatable
  extend ActiveSupport::Concern

  included do
    devise :database_authenticatable, :recoverable
  end
end

class User < ActiveRecord::Base
  include Authenticatable
end

class UsersController < ApplicationController
  def show = current_user
end
```

### result

```rbs
class ActionController::Base
  def current_user: -> User
  def user_signed_in?: -> bool
  def authenticate_user!: -> void
  def user_session: -> untyped
end

class ActiveRecord::Base
  def self.devise: (*Symbol modules) -> void
end

module Authenticatable
  extend ActiveSupport::Concern
end

class User < ActiveRecord::Base
  include Authenticatable
end

class UsersController < ApplicationController
  def show: -> User
end
```
