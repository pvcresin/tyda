# Known Issues / Rails Columns

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
  private def require_entry: -> true?
  private def entry: -> Entry?
  private def render_title: (Entry found) -> "title"
end
```
