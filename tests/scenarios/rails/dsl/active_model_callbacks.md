# Rails / DSL / Active Model Callbacks

## Generate run_callbacks and filtered registrar

```yaml
include_synthetic_dsl_methods: true
```

### update

```ruby
class Photo
  extend ActiveModel::Callbacks

  define_model_callbacks :save, only: [:before]

  def upload
    run_callbacks(:save) { 1 }
  end
end
```

### result

```rbs
class Photo
  extend ActiveModel::Callbacks

  def run_callbacks: (Symbol kind) -> untyped
  def self.before_save: (*untyped args) -> untyped
  def upload: -> untyped
end
```

## Generate all callback kinds without only option

```yaml
include_synthetic_dsl_methods: true
```

### update

```ruby
class Coordinator
  extend ActiveModel::Callbacks

  define_model_callbacks :connect
end
```

### result

```rbs
class Coordinator
  extend ActiveModel::Callbacks

  def run_callbacks: (Symbol kind) -> untyped
  def self.before_connect: (*untyped args) -> untyped
  def self.after_connect: (*untyped args) -> untyped
  def self.around_connect: (*untyped args) -> untyped
end
```
