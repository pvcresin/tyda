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
    Entry.find_by(id: params[:id])
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
  private def require_entry: -> void?
  private def entry: -> Entry?
  private def render_title: (Entry found) -> "title"
end
```
