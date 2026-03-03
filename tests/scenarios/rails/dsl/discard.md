# Rails / DSL / Discard

## Generate discard methods from Discard::Model

### update

```ruby
class Post < ApplicationRecord
  include Discard::Model
end
```

### result

```rbs
class Post < ApplicationRecord
  include Discard::Model

  def discard: -> bool
  def discard!: -> bool
  def undiscard: -> bool
  def undiscard!: -> bool
  def discarded?: -> bool
  def kept?: -> bool
  def undiscarded?: -> bool
  def self.kept: -> ActiveRecord::Relation[Post]
  def self.undiscarded: -> ActiveRecord::Relation[Post]
  def self.discarded: -> ActiveRecord::Relation[Post]
  def self.with_discarded: -> ActiveRecord::Relation[Post]
end
```

## discard scopes chain on relation

### update

```ruby
class Post < ApplicationRecord
  include Discard::Model

  def self.visible = kept.discarded
end
```

### result

```rbs
class Post < ApplicationRecord
  include Discard::Model

  def discard: -> bool
  def discard!: -> bool
  def undiscard: -> bool
  def undiscard!: -> bool
  def discarded?: -> bool
  def kept?: -> bool
  def undiscarded?: -> bool
  def self.kept: -> ActiveRecord::Relation[Post]
  def self.undiscarded: -> ActiveRecord::Relation[Post]
  def self.discarded: -> ActiveRecord::Relation[Post]
  def self.with_discarded: -> ActiveRecord::Relation[Post]
  def self.visible: -> ActiveRecord::Relation[Post]
end
```
