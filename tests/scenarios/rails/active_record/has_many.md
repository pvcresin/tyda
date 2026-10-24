# Rails / Active Record / Has Many

## Generate has_many collection methods

### update

```ruby
class A
  has_many :items
end
```

### result

```rbs
class A
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[Item] items) -> ActiveRecord::Associations::CollectionProxy[Item]
end
```

## has_many class_name sets the class

### update

```ruby
class A
  has_many :items, class_name: "B"
end
```

### result

```rbs
class A
  def items: -> ActiveRecord::Associations::CollectionProxy[B]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[B] items) -> ActiveRecord::Associations::CollectionProxy[B]
end
```

## Inflect plural has_many names

### update

```ruby
class A
  has_many :items
end
```

### result

```rbs
class A
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[Item] items) -> ActiveRecord::Associations::CollectionProxy[Item]
end
```

## Resolve first and build from has_many proxy

### update

```ruby
class A < ApplicationRecord
  has_many :items

  def first_item = items.first

  def build_item = items.build
end
```

### result

```rbs
class A < ApplicationRecord
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[Item] items) -> ActiveRecord::Associations::CollectionProxy[Item]
  def first_item: -> Item?
  def build_item: -> Item
end
```

## Resolve has_many through and source targets

### update

```ruby
class A < ApplicationRecord
  has_many :items, through: :links, source: :item
end
```

### result

```rbs
class A < ApplicationRecord
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[Item] items) -> ActiveRecord::Associations::CollectionProxy[Item]
end
```

## Use has_many inverse_of for inverse target class

### update

```ruby
class A < ApplicationRecord
  belongs_to :item, class_name: "B", inverse_of: :items
end

class B < ApplicationRecord
  has_many :items, inverse_of: :item
end
```

### result

```rbs
class A < ApplicationRecord
  def item: -> B
  def item=: (B item) -> B
  def build_item: -> B
  def create_item: -> B
end

class B < ApplicationRecord
  def items: -> ActiveRecord::Associations::CollectionProxy[A]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[A] items) -> ActiveRecord::Associations::CollectionProxy[A]
end
```

## Apply custom inflections to has_many target inference

### update

`config/initializers/inflections.rb`

```ruby
ActiveSupport::Inflector.inflections(:en) do |inflect|
  inflect.irregular "person", "people"
end
```

```ruby
class ApplicationRecord; end

class Person < ApplicationRecord
end

class Team < ApplicationRecord
  has_many :people

  def first_person = people.first
end
```

### result

```rbs
class Team < ApplicationRecord
  def people: -> ActiveRecord::Associations::CollectionProxy[Person]
  def person_ids: -> Array[Integer]
  def person_ids=: (Array[Integer] person_ids) -> Array[Integer]
  def people=: (Array[Person] people) -> ActiveRecord::Associations::CollectionProxy[Person]
  def first_person: -> Person?
end
```
