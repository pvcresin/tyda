# Rails / Active Support / Concern

## Collect class_methods block methods as class methods

### update

```ruby
module Searchable
  class_methods do
    def search(query)
      query
    end
  end
end
```

### result

```rbs
module Searchable
  def self.search: (untyped query) -> untyped
end
```

## Collect included block methods as instance methods

### update

```ruby
module Trackable
  included do
    def tracked?
      true
    end
  end
end
```

### result

```rbs
module Trackable
  def tracked?: -> true
end
```

## Combine class_methods and included

### update

```ruby
module Auditable
  class_methods do
    def audit_log
      []
    end
  end

  included do
    def audited?
      false
    end
  end
end
```

### result

```rbs
module Auditable
  def self.audit_log: -> [ ]
  def audited?: -> false
end
```

## Resolve class_methods through including class

### update

```ruby
module Searchable
  extend ActiveSupport::Concern

  class_methods do
    def search = "found"
  end
end

class Article
  include Searchable
end

def result = Article.search
```

### result

```rbs
class Article
  include Searchable
end

class Object
  def result: -> "found"
end

module Searchable
  extend ActiveSupport::Concern

  def self.search: -> "found"
end
```

## Resolve cross-file class_methods through including class

### update

`app/models/concerns/searchable.rb`

```ruby
module Searchable
  extend ActiveSupport::Concern

  class_methods do
    def search = "found"
  end
end
```

```ruby
class Article
  include Searchable
end

def result = Article.search
```

### result

```rbs
class Article
  include Searchable
end

class Object
  def result: -> "found"
end
```

## Resolve cross-file included block through including class

### update

`app/models/concerns/trackable.rb`

```ruby
module Trackable
  extend ActiveSupport::Concern

  included do
    def tracked = :tracked
  end
end
```

```ruby
class Article
  include Trackable
end

def result = Article.new.tracked
```

### result

```rbs
class Article
  include Trackable
end

class Object
  def result: -> :tracked
end
```

## Scope in included block resolves to includer relation

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

module Searchable
  extend ActiveSupport::Concern

  included do
    scope :active, -> { where(active: true) }
  end
end

class Item < ApplicationRecord
  include Searchable
end

def result = Item.active
```

### result

```rbs
class Item < ApplicationRecord
  include Searchable
end

class Object
  def result: -> ActiveRecord::Relation[Item]
end

module Searchable
  extend ActiveSupport::Concern

  def self.active: -> ActiveRecord::Relation[self]
end
```

## Scope in included block resolves per includer

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

module Searchable
  extend ActiveSupport::Concern

  included do
    scope :active, -> { where(active: true) }
  end
end

class Item < ApplicationRecord
  include Searchable
end

class User < ApplicationRecord
  include Searchable
end

def item_result = Item.active
def user_result = User.active
```

### result

```rbs
class Item < ApplicationRecord
  include Searchable
end

class Object
  def item_result: -> ActiveRecord::Relation[Item]
  def user_result: -> ActiveRecord::Relation[User]
end

module Searchable
  extend ActiveSupport::Concern

  def self.active: -> ActiveRecord::Relation[self]
end

class User < ApplicationRecord
  include Searchable
end
```

## Typed attribute in included block resolves through includer

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

module Countable
  extend ActiveSupport::Concern

  included do
    attribute :hits, :integer
  end
end

class Item < ApplicationRecord
  include Countable
end

def result = Item.new.hits
```

### result

```rbs
module Countable
  extend ActiveSupport::Concern
end

class Item < ApplicationRecord
  include Countable
end

class Object
  def result: -> Integer?
end
```

## Untyped attribute in included block stays untyped

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

module Notable
  extend ActiveSupport::Concern

  included do
    attribute :notes
  end
end

class Item < ApplicationRecord
  include Notable
end

def result = Item.new.notes
```

### result

```rbs
class Item < ApplicationRecord
  include Notable
end

module Notable
  extend ActiveSupport::Concern
end

class Object
  def result: -> untyped
end
```

## Untyped attribute in included block uses includer schema column type

### update

`db/schema.rb`

```ruby
ActiveRecord::Schema[7.1].define(version: 2024_01_01) do
  create_table "items", force: :cascade do |t|
    t.string "notes"
  end
end
```

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

module Notable
  extend ActiveSupport::Concern

  included do
    attribute :notes
  end
end

class Item < ApplicationRecord
  include Notable
end

def result = Item.new.notes
```

### result

```rbs
class Item < ApplicationRecord
  include Notable
end

module Notable
  extend ActiveSupport::Concern
end

class Object
  def result: -> String?
end
```

## Attribute in included block renders on concern with synthetic flag

```yaml
include_synthetic_dsl_methods: true
```

### update

```ruby
module Countable
  extend ActiveSupport::Concern

  included do
    attribute :hits, :integer
  end
end
```

### result

```rbs
module Countable
  extend ActiveSupport::Concern

  def hits: -> Integer?
  def hits=: (Integer? hits) -> Integer?
  def hits_changed?: -> bool
  def hits_previously_changed?: -> bool
  def saved_change_to_hits?: -> bool
  def will_save_change_to_hits?: -> bool
  def hits_change: -> [Integer?, Integer?]
  def hits_was: -> Integer?
  def hits_previously_was: -> Integer?
  def hits_before_last_save: -> Integer?
  def hits_in_database: -> Integer?
  def hits_previous_change: -> Array[Integer?]?
  def hits_change_to_be_saved: -> Array[Integer?]?
  def saved_change_to_hits: -> Array[Integer?]?
  def hits_will_change!: -> void
  def restore_hits!: -> void
  def clear_hits_change: -> void
end
```

## User def after attribute in included block wins

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

module Colorable
  extend ActiveSupport::Concern

  DEFAULT_COLOR = "#6699cc"

  included do
    attribute :color, :string, default: DEFAULT_COLOR

    def color
      super || DEFAULT_COLOR
    end
  end
end
```

### result

```rbs
module Colorable
  extend ActiveSupport::Concern

  DEFAULT_COLOR: "#6699cc"

  def color: -> untyped | "#6699cc"
end
```

## Association in included block resolves through includer

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end
class Tagging < ApplicationRecord; end

module Taggable
  extend ActiveSupport::Concern

  included do
    has_many :taggings
  end
end

class Item < ApplicationRecord
  include Taggable
end

def result = Item.new.taggings
```

### result

```rbs
class Item < ApplicationRecord
  include Taggable
end

class Object
  def result: -> ActiveRecord::Associations::CollectionProxy[Tagging]
end

module Taggable
  extend ActiveSupport::Concern

  def taggings: -> ActiveRecord::Associations::CollectionProxy[Tagging]
  def tagging_ids: -> Array[Integer]
  def tagging_ids=: (Array[Integer] tagging_ids) -> Array[Integer]
  def taggings=: (Array[Tagging] taggings) -> ActiveRecord::Associations::CollectionProxy[Tagging]
end
```

## Enum in included block resolves per includer

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

module Statused
  extend ActiveSupport::Concern

  included do
    enum status: { active: 0, archived: 1 }
  end
end

class Item < ApplicationRecord
  include Statused
end

class User < ApplicationRecord
  include Statused
end

def item_result = Item.active
def user_result = User.active
```

### result

```rbs
class Item < ApplicationRecord
  include Statused
end

class Object
  def item_result: -> ActiveRecord::Relation[Item]
  def user_result: -> ActiveRecord::Relation[User]
end

module Statused
  extend ActiveSupport::Concern

  def active?: -> bool
  def active!: -> bool
  def self.active: -> ActiveRecord::Relation[self]
  def archived?: -> bool
  def archived!: -> bool
  def self.archived: -> ActiveRecord::Relation[self]
end

class User < ApplicationRecord
  include Statused
end
```

## Store accessor in included block resolves through includer

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

module Themeable
  extend ActiveSupport::Concern

  included do
    store_accessor :settings, :theme
  end
end

class Item < ApplicationRecord
  include Themeable
end

def result = Item.new.theme
```

### result

```rbs
class Item < ApplicationRecord
  include Themeable
end

class Object
  def result: -> untyped
end

module Themeable
  extend ActiveSupport::Concern
end
```

## Cross-file scope in included block resolves to includer relation

### update

`app/models/concerns/searchable.rb`

```ruby
module Searchable
  extend ActiveSupport::Concern

  included do
    scope :active, -> { where(active: true) }
  end
end
```

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

class Item < ApplicationRecord
  include Searchable
end

def result = Item.active
```

### result

```rbs
class Item < ApplicationRecord
  include Searchable
end

class Object
  def result: -> ActiveRecord::Relation[Item]
end
```
