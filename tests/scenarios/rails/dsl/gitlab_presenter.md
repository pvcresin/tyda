# Rails / DSL / Gitlab Presenter

## Define typed reader from presents with as keyword

### update

```ruby
class Gitlab::View::Presenter::Base; end

class Ci::Build; end

class Ci::BuildPresenter < Gitlab::View::Presenter::Base
  presents ::Ci::Build, as: :build

  def display_name = build
end
```

### result

```rbs
class Ci::BuildPresenter < Gitlab::View::Presenter::Base
  def build: -> Ci::Build
  def display_name: -> Ci::Build
end
```

## Define untyped readers from legacy symbol presents

### update

```ruby
class Gitlab::View::Presenter::Base; end

class MergeRequestPresenter < Gitlab::View::Presenter::Base
  presents :merge_request

  def title = merge_request
end
```

### result

```rbs
class MergeRequestPresenter < Gitlab::View::Presenter::Base
  def merge_request: -> untyped
  def title: -> untyped
end
```

## Resolve presenter runtime accessors

### update

```ruby
class Gitlab::View::Presenter::Base; end

class ProjectPresenter < Gitlab::View::Presenter::Base
  def allowed? = can?(current_user, :read_project, subject)
end
```

### result

```rbs
class ProjectPresenter < Gitlab::View::Presenter::Base
  def allowed?: -> bool
end
```
