# Rails / Schema / Inflector

## Apply references null and default to accessor types

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "posts", force: :cascade do |t|
    t.references "a", null: false
    t.belongs_to "x", polymorphic: true
    t.string "status", default: "draft"
  end
end
```

```ruby
class ApplicationRecord; end

class Post < ApplicationRecord
  def foo = status.upcase

  def bar = a_id.succ

  def baz = x_type&.upcase
end
```

### result

```rbs
class Post < ApplicationRecord
  def foo: -> String
  def bar: -> Integer
  def baz: -> String?
end
```

## External RBS wins over schema accessors

### update

```rbs
class Post
  def status: -> Symbol
end
```

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "posts", force: :cascade do |t|
    t.string "status", default: "draft"
  end
end
```

```ruby
class Post
  def foo = status
end
```

### result

```rbs
class Post
  def foo: -> Symbol
end
```

## Prefer namespaced models from schema.rb

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "admin_users", force: :cascade do |t|
    t.string "name"
  end
end
```

`app/models/admin/user.rb`

```ruby
class Admin::User < ApplicationRecord
end
```

```ruby
class ApplicationRecord; end

class Admin::User < ApplicationRecord
  def foo = name.upcase
end
```

### result

```rbs
class Admin::User < ApplicationRecord
  def foo: -> String
end
```

## Apply irregular and acronym inflections to schema resolution

### update

`config/initializers/inflections.rb`

```ruby
ActiveSupport::Inflector.inflections(:en) do |inflect|
  inflect.irregular "person", "people"
  inflect.acronym "API"
end
```

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "people", force: :cascade do |t|
    t.string "name"
  end

  create_table "api_clients", force: :cascade do |t|
    t.string "api_key"
  end
end
```

`app/models/person.rb`

```ruby
class Person < ApplicationRecord
end
```

`app/models/api_client.rb`

```ruby
class APIClient < ApplicationRecord
end
```

```ruby
class ApplicationRecord; end

class Person < ApplicationRecord
  def foo = name.upcase
end

class APIClient < ApplicationRecord
  def bar = api_key.upcase
end
```

### result

```rbs
class APIClient < ApplicationRecord
  def bar: -> String
end

class Person < ApplicationRecord
  def foo: -> String
end
```

## Resolve standard schema columns and association accessors

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "posts", force: :cascade do |t|
    t.references "a", null: false
    t.belongs_to "x", polymorphic: true
    t.string "status", default: "draft"
  end
end
```

```ruby
class ApplicationRecord; end

class Post < ApplicationRecord
  def foo = id.succ

  def bar = created_at

  def baz = a
end
```

### result

```rbs
class Post < ApplicationRecord
  def foo: -> Integer
  def bar: -> Time
  def baz: -> A
end
```
