# Rails / DSL / Grape

## Collect helpers block defs as endpoint instance methods

### update

```ruby
class Grape::API; end

class API::Base < Grape::API
  helpers do
    def current_user
      'user'
    end

    def authenticate!
      true
    end
  end
end
```

### result

```rbs
class API::Base < Grape::API
  def current_user: -> "user"
  def authenticate!: -> true
end
```

## Include helper modules passed to helpers

### update

```ruby
class Grape::API; end

module API::Helpers
  def pagination
    'page'
  end
end

class API::Users < Grape::API
  helpers ::API::Helpers
end

class Caller
  def call(api)
    api.pagination
  end
end
```

### result

```rbs
module API::Helpers
  def pagination: -> "page"
end

class API::Users < Grape::API
  include API::Helpers
end

class Caller
  def call: (untyped api) -> untyped
end
```

## Collect helpers nested inside namespace blocks

### update

```ruby
class Grape::API; end

class API::Projects < Grape::API
  namespace :projects do
    helpers do
      def find_project
        'project'
      end
    end
  end
end
```

### result

```rbs
class API::Projects < Grape::API
  def find_project: -> "project"
end
```
