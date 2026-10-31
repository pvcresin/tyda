# Rails / Action Controller

## Infer params and render in controller actions

### update

```ruby
class UsersController < ActionController::Base
  before_action :authenticate!

  def authenticate! = render

  def show = params
end
```

### result

```rbs
class UsersController < ActionController::Base
  def authenticate!: -> void
  def show: -> ActionController::Parameters
end
```

## Resolve params.require.permit as ActionController::Parameters

### update

```ruby
class UsersController < ActionController::Base
  def user_params = params.require(:user).permit(:name, :email)

  def as_hash = params.require(:user).to_h
end
```

### result

```rbs
class UsersController < ActionController::Base
  def user_params: -> ActionController::Parameters
  def as_hash: -> Hash[String, untyped]
end
```

## Resolve params accessors

### update

```ruby
class UsersController < ActionController::Base
  def read_param = params[:id].to_s

  def fetch_param = params.fetch(:id).to_s
end
```

### result

```rbs
class UsersController < ActionController::Base
  def read_param: -> String
  def fetch_param: -> String
end
```

## Handle respond_to blocks in controller actions

### update

```ruby
class UsersController < ActionController::Base
  def show
    respond_to do |format|
      format.json { render }
    end
  end
end
```

### result

```rbs
class UsersController < ActionController::Base
  def show: -> void
end
```

## Resolve controller request helpers

### update

```ruby
class UsersController < ActionController::Base
  def remote_ip = request.remote_ip

  def get_request = request.get?

  def notice = flash[:notice].to_s

  def user_id = session[:user_id].to_i
end
```

### result

```rbs
class UsersController < ActionController::Base
  def remote_ip: -> String
  def get_request: -> bool
  def notice: -> String
  def user_id: -> Integer
end
```

## Do not use respond_to fast path for controller-like names

### update

```ruby
class FakeController
  def respond_to = 42

  def show
    respond_to do |format|
      format.json { render }
    end
  end
end
```

### result

```rbs
class FakeController
  def respond_to: -> 42
  def show: -> 42
end
```

## Do not use fast path with custom ActionController respond_to

### update

```ruby
class UsersController < ActionController::Base
  def respond_to = "custom"

  def show
    respond_to do |format|
      format.json { render }
    end
  end
end
```

### result

```rbs
class UsersController < ActionController::Base
  def respond_to: -> "custom"
  def show: -> "custom"
end
```

## Show controller first-step method as synthetic

```yaml
include_synthetic_dsl_methods: true
```

### update

```ruby
class UsersController < ActionController::Base
  def show = params
end
```

### result

```rbs
class UsersController < ActionController::Base
  def params: -> ActionController::Parameters
  def render: -> void
  def redirect_to: -> void
  def redirect_back: -> void
  def redirect_back_or_to: -> void
  def head: -> void
  def send_data: -> void
  def send_file: -> void
  def show: -> ActionController::Parameters
end
```

## Infer set_ methods as void

### update

```ruby
class Admin::PostsController < ActionController::Base
  def set_post
    @post = find_post
  end

  def assign_role
    @role = lookup_role
  end
end
```

### result

```rbs
class Admin::PostsController < ActionController::Base
  def set_post: -> void
  def assign_role: -> void
end
```
