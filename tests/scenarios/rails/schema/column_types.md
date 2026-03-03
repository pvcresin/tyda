# Rails / Schema / Column Types

## Map schema.rb column types to accessor types

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "posts", force: :cascade do |t|
    t.jsonb "meta"
    t.jsonb "config", null: false
    t.json "payload"
    t.hstore "props"
    t.decimal "amount", precision: 10, scale: 2
    t.date "due_on"
    t.datetime "published_at"
    t.string "tags", array: true
  end
end
```

```ruby
class ApplicationRecord; end

class Post < ApplicationRecord
  def a = meta
  def b = config
  def c = payload
  def d = props
  def e = amount
  def f = due_on
  def g = published_at
  def h = tags
end
```

### result

```rbs
class Post < ApplicationRecord
  def a: -> untyped
  def b: -> untyped
  def c: -> untyped
  def d: -> untyped
  def e: -> BigDecimal?
  def f: -> Date?
  def g: -> (ActiveSupport::TimeWithZone | DateTime)?
  def h: -> Array[String]?
end
```

## Map structure.sql column types to accessor types

### update

`db/structure.sql`

```sql
CREATE TABLE public.posts (
    id bigint NOT NULL,
    meta jsonb,
    config jsonb NOT NULL,
    payload json,
    props hstore,
    amount numeric(10,2),
    due_on date,
    published_at timestamp(6) without time zone,
    tags character varying[]
);
```

```ruby
class ApplicationRecord; end

class Post < ApplicationRecord
  def a = meta
  def b = config
  def c = payload
  def d = props
  def e = amount
  def f = due_on
  def g = published_at
  def h = tags
end
```

### result

```rbs
class Post < ApplicationRecord
  def a: -> untyped
  def b: -> untyped
  def c: -> untyped
  def d: -> untyped
  def e: -> BigDecimal?
  def f: -> Date?
  def g: -> (ActiveSupport::TimeWithZone | DateTime)?
  def h: -> Array[String]?
end
```

## Keep not-null decimal and date non-nilable

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "posts", force: :cascade do |t|
    t.decimal "amount", null: false
    t.date "due_on", null: false
  end
end
```

```ruby
class ApplicationRecord; end

class Post < ApplicationRecord
  def a = amount
  def b = due_on
end
```

### result

```rbs
class Post < ApplicationRecord
  def a: -> BigDecimal
  def b: -> Date
end
```

## Resolve dirty-tracking methods from schema columns

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "posts", force: :cascade do |t|
    t.string "title"
    t.integer "views", null: false
  end
end
```

```ruby
class ApplicationRecord; end

class Post < ApplicationRecord
  def a = title_changed?
  def b = title_was
  def c = title_change
  def d = saved_change_to_title
  def e = views_in_database
  def f = title_will_change!
end
```

### result

```rbs
class Post < ApplicationRecord
  def a: -> bool
  def b: -> String?
  def c: -> [String?, String?]
  def d: -> Array[String?]?
  def e: -> Integer?
  def f: -> void
end
```
