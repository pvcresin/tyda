# Rails / DSL / Redmine

## Leave dynamic DSL declarations unresolved

### update

```ruby
class News < ApplicationRecord
  acts_as_attachable
  acts_as_watchable
  acts_as_event
  acts_as_webhookable
  acts_as_searchable
  acts_as_activity_provider
  include Redmine::SafeAttributes
  safe_attributes "title", "summary"
end
```

### result

```rbs
class News < ApplicationRecord
  include Redmine::SafeAttributes
end
```

## Preserve explicit instance methods

### update

```ruby
class Issue < ApplicationRecord
  include Redmine::SafeAttributes

  def editable?
    safe_attribute?("subject")
  end
end
```

### result

```rbs
class Issue < ApplicationRecord
  include Redmine::SafeAttributes

  def editable?: -> bool
end
```
