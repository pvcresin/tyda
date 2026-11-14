# Known Issues / Rails Framework

## A controller action calls a concern method defined in another file

```yaml
rails_version: 7.1.0
```

### update

### file: app/controllers/concerns/paginated.rb

```ruby
module Paginated
  extend ActiveSupport::Concern

  def page_params = params.permit(:page)
end
```

```ruby
class PostsController < ActionController::Base
  include Paginated

  def index = page_params
end
```

### result

```rbs
class PostsController < ActionController::Base
  include Paginated

  def index: -> ActionController::Parameters
end
```
