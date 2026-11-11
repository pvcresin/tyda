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
  def h: -> Array[String]?
end
```

## Map datetime from schema.rb to TimeWithZone

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
  def g = published_at
end
```

### result

```rbs
class Post < ApplicationRecord
  def g: -> ActiveSupport::TimeWithZone?
end
```

## Map explicit types in `t.column`

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "events", force: :cascade do |t|
    t.column "occurred_at", :timestamp, null: false
    t.column "due_on", :date
  end
end
```

```ruby
class ApplicationRecord; end

class Event < ApplicationRecord
  def happened_at = occurred_at
  def due_date = due_on
end
```

### result

```rbs
class Event < ApplicationRecord
  def happened_at: -> ActiveSupport::TimeWithZone
  def due_date: -> Date?
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
  def h: -> Array[String]?
end
```

## Map datetime from structure.sql to TimeWithZone

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
  def g = published_at
end
```

### result

```rbs
class Post < ApplicationRecord
  def g: -> ActiveSupport::TimeWithZone?
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

## Timestamps columns use the datetime column type

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "posts", force: :cascade do |t|
    t.timestamps
  end
end
```

### file: app/models/post.rb

```ruby
class Post < ApplicationRecord
end
```

```ruby
class PostTimes
  def created = Post.new.created_at
  def updated = Post.new.updated_at
end
```

### result

```rbs
class PostTimes
  def created: -> ActiveSupport::TimeWithZone
  def updated: -> ActiveSupport::TimeWithZone
end
```

## A uuid primary key types id as String

```yaml
rails_version: 7.1.0
```

### update

### file: db/schema.rb

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "jobs", id: :uuid, force: :cascade do |t|
    t.string "name"
  end
end
```

### file: app/models/job.rb

```ruby
class Job < ApplicationRecord
end
```

```ruby
class JobIds
  def job_id = Job.new.id
  def job_ids = Job.ids
end
```

### result

```rbs
class JobIds
  def job_id: -> String
  def job_ids: -> Array[String]
end
```

## TimeWithZone columns answer Time methods through the zone

```yaml
rails_version: 7.1.0
```

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "users", id: :serial, force: :cascade do |t|
    t.datetime "updated_at", null: false
  end
end
```

```ruby
class ApplicationRecord; end

class User < ApplicationRecord
  def key = id
  def elapsed = updated_at - Time.now
  def shifted = updated_at + 1.day
  def earlier = updated_at - 60
  def before = updated_at - 1.day
  def rounded = updated_at.round
  def sunday = updated_at.sunday?
  def day = updated_at.to_date
end
```

### result

```rbs
class User < ApplicationRecord
  def key: -> Integer
  def elapsed: -> Float
  def shifted: -> ActiveSupport::TimeWithZone
  def earlier: -> ActiveSupport::TimeWithZone
  def before: -> ActiveSupport::TimeWithZone
  def rounded: -> ActiveSupport::TimeWithZone
  def sunday: -> bool
  def day: -> Date
end
```

## Map PostgreSQL native column types

```yaml
rails_version: 7.1.0
```

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "hosts", id: :bigserial, force: :cascade do |t|
    t.inet "address", null: false
    t.interval "ttl", null: false
    t.money "price", null: false
    t.daterange "window", null: false
    t.timestamptz "seen_at", null: false
  end
end
```

```ruby
class ApplicationRecord; end

class Host < ApplicationRecord
  def key = id
  def ip = address
  def lifetime = ttl
  def cost = price
  def span = window
  def seen = seen_at
end
```

### result

```rbs
class Host < ApplicationRecord
  def key: -> Integer
  def ip: -> IPAddr
  def lifetime: -> ActiveSupport::Duration
  def cost: -> BigDecimal
  def span: -> Range[Date]
  def seen: -> ActiveSupport::TimeWithZone
end
```

## Attribute decorators keep the column reader and literal attribute reads use it

```yaml
rails_version: 7.1.0
```

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "account_stats", force: :cascade do |t|
    t.string "acct", default: "", null: false
    t.bigint "statuses_count", default: 0, null: false
  end
end
```

```ruby
class ApplicationRecord; end

class AccountStat < ApplicationRecord
  normalizes :acct, with: ->(acct) { acct.strip }
  encrypts :acct

  def shout = acct.upcase
  def statuses_count = [attributes["statuses_count"], 0].max
  def by_index = self[:acct]
end
```

### result

```rbs
class AccountStat < ApplicationRecord
  def self.normalize_value_for: (Symbol name, untyped value) -> untyped
  def shout: -> String
  def statuses_count: -> Integer
  def by_index: -> String
end
```

## super in an attribute override reaches the generated column method

```yaml
rails_version: 7.1.0
```

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "commit_statuses", force: :cascade do |t|
    t.string "before_sha"
    t.boolean "emails_enabled", null: false
  end
end
```

```ruby
class ApplicationRecord; end

class CommitStatus < ApplicationRecord
  def before_sha
    super || "0000"
  end

  def emails_enabled?
    !!super
  end
end
```

### result

```rbs
class CommitStatus < ApplicationRecord
  def before_sha: -> String
  def emails_enabled?: -> bool
end
```
