# Rails / Routes / Path Helpers

## Resolve nested resource and member collection helpers

### update

`config/routes.rb`

```ruby
Rails.application.routes.draw do
  namespace :admin do
    resources :users do
      resources :posts
      member do
        get :preview
      end
      collection do
        get :search
      end
    end
  end
end
```

```ruby
class Links
  def posts_for(user) = admin_user_posts_path(user)

  def preview_for(user) = preview_admin_user_path(user)

  def search_users = search_admin_users_path
end
```

### result

```rbs
class Links
  def posts_for: (untyped user) -> String
  def preview_for: (untyped user) -> String
  def search_users: -> String
end
```

## Treat dynamic segments as required helper args

### update

`config/routes.rb`

```ruby
Rails.application.routes.draw do
  get "users/:id/profile", to: "profiles#show", as: :user_profile
end
```

```ruby
class Profiles
  def profile_for(id) = user_profile_path(id)
end
```

### result

```rbs
class Profiles
  def profile_for: (untyped id) -> String
end
```

## Resolve resources resource and named route helpers

### update

`config/routes.rb`

```ruby
Rails.application.routes.draw do
  resources :users
  resource :profile
  get "settings", to: "pages#show", as: :settings
end
```

```ruby
class Links
  def users_index = users_path

  def user_show(user) = user_path(user)

  def new_user_form = new_user_path

  def edit_user_form(user) = edit_user_path(user)

  def profile_page = profile_path

  def settings_page = settings_path
end
```

### result

```rbs
class Links
  def users_index: -> String
  def user_show: (untyped user) -> String
  def new_user_form: -> String
  def edit_user_form: (untyped user) -> String
  def profile_page: -> String
  def settings_page: -> String
end
```

## Resolve namespace helpers

### update

`config/routes.rb`

```ruby
Rails.application.routes.draw do
  namespace :admin do
    resources :users
    get "dashboard", to: "dashboard#show", as: :dashboard
  end
end
```

```ruby
class AdminLinks
  def users_index = admin_users_path

  def user_show(user) = admin_user_path(user)

  def new_user_form = new_admin_user_path

  def edit_user_form(user) = edit_admin_user_path(user)

  def dashboard = admin_dashboard_path
end
```

### result

```rbs
class AdminLinks
  def users_index: -> String
  def user_show: (untyped user) -> String
  def new_user_form: -> String
  def edit_user_form: (untyped user) -> String
  def dashboard: -> String
end
```
