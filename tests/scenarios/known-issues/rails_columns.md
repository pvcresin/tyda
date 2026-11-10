# Known Issues / Rails Columns

## An enum declared by the including model types the column read in a concern

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/entry.rb

```ruby
class Entry < ApplicationRecord
  include Listable

  enum :kind, { backlog: 0, closed: 1 }
end
```

```schema
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "entries", force: :cascade do |t|
    t.integer "kind"
  end
end
```

```ruby
module Listable
  extend ActiveSupport::Concern

  def title
    kind.humanize
  end
end
```

### result

```rbs
module Listable
  extend ActiveSupport::Concern

  def title: -> String
end
```

## A before action that halts on nil guards the action

```yaml
rails_version: 7.1.0
```

### update

### file: app/models/entry.rb

```ruby
class Entry < ApplicationRecord
  def title = "title"
end
```

```ruby
class EntriesController < ApplicationController
  before_action :require_entry

  def show
    render_title(entry)
  end

  private

  def require_entry
    head :not_found unless entry
  end

  def entry
    @entry ||= Entry.find_by(id: params[:id])
  end

  def render_title(found)
    found.title
  end
end
```

### result

```rbs
class EntriesController < ApplicationController
  def show: -> "title"
  private def require_entry: -> void
  private def entry: -> Entry?
  private def render_title: (Entry found) -> "title"
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
