# Rails / Active Record / Alias Attribute

## alias_attribute keeps getter and setter types

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base
  attribute :name, :string
end

class Project < ApplicationRecord
  alias_attribute :title, :name
end
```

### result

```rbs
class ApplicationRecord < ActiveRecord::Base
  def name: -> String?
  def name=: (String? name) -> String?
  def name_changed?: -> bool
  def name_previously_changed?: -> bool
  def saved_change_to_name?: -> bool
  def will_save_change_to_name?: -> bool
  def name_change: -> [String?, String?]
  def name_was: -> String?
  def name_previously_was: -> String?
  def name_before_last_save: -> String?
  def name_in_database: -> String?
  def name_previous_change: -> Array[String?]?
  def name_change_to_be_saved: -> Array[String?]?
  def saved_change_to_name: -> Array[String?]?
  def name_will_change!: -> void
  def restore_name!: -> void
  def clear_name_change: -> void
end

class Project < ApplicationRecord
  def title: -> String?
  def title=: (String? title) -> String?
end
```

## alias_attribute follows foreign key aliases

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Namespace < ApplicationRecord
end

class Project < ApplicationRecord
  belongs_to :namespace
  alias_attribute :parent_id, :namespace_id
end
```

### result

```rbs
class Project < ApplicationRecord
  def namespace: -> Namespace
  def namespace=: (Namespace namespace) -> Namespace
  def build_namespace: -> Namespace
  def create_namespace: -> Namespace
  def parent_id: -> Integer
  def parent_id=: (Integer parent_id) -> Integer
end
```
