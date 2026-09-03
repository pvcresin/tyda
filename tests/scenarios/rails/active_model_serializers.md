# Rails / Active Model Serializers

## Apply attributes from macro and initialize super keywords

### update

```ruby
class ActiveModelSerializers::Model; end

class A::B < ActiveModelSerializers::Model
  attributes :x, :y, :z, :item

  def initialize(item, attrs)
    @item = item

    super(
      x: 'x',
      y: 123,
      z: nil,
    )
  end
end
```

### result

```rbs
class A::B < ActiveModelSerializers::Model
  def x: -> String
  def x=: (String x) -> "x"
  def y: -> Integer
  def y=: (Integer y) -> 123
  def z: -> nil
  def z=: (nil z) -> nil
  def item: -> untyped
  def item=: (untyped item) -> untyped
  def initialize: (untyped item, untyped attrs) -> void
end
```

## Apply super keywords through string-like helpers

### update

```ruby
class ActiveModelSerializers::Model; end

class A::B < ActiveModelSerializers::Model
  attributes :x, :y, :z, :item

  def initialize(item, attrs)
    @item = item

    super(
      x: foo(attrs['x']),
      y: foo(attrs['y']),
      z: nil,
    )
  end

  def foo(s) = s.strip[0, 10]
end
```

### result

```rbs
class A::B < ActiveModelSerializers::Model
  def x: -> untyped
  def x=: (untyped x) -> untyped
  def y: -> untyped
  def y=: (untyped y) -> untyped
  def z: -> nil
  def z=: (nil z) -> nil
  def item: -> untyped
  def item=: (untyped item) -> untyped
  def initialize: (untyped item, untyped attrs) -> void
  def foo: ((untyped) s) -> untyped
end
```

## verified_at prefers nilable DateTime from initialize

### update

```ruby
class ActiveModelSerializers::Model; end

class A::B < ActiveModelSerializers::Model
  attributes :x, :y, :z, :item

  def initialize(item, attrs)
    @data = attrs
    @item = item

    super(
      x: foo(attrs['x']),
      y: foo(attrs['y']),
      z: attrs['z']&.to_datetime,
    )
  end

  def mark!
    @data['z'] = self.z = Time.now.utc
  end

  def foo(s) = s.strip[0, 10]
end
```

### result

```rbs
class A::B < ActiveModelSerializers::Model
  def x: -> untyped
  def x=: (untyped x) -> untyped
  def y: -> untyped
  def y=: (untyped y) -> untyped
  def z: -> DateTime
  def z=: (DateTime z) -> DateTime
  def item: -> untyped
  def item=: (untyped item) -> untyped
  def initialize: (untyped item, untyped attrs) -> void
  def mark!: -> Time
  def foo: ((untyped) s) -> untyped
end
```

## present? returns bool in Rails context

### update

```ruby
class ActiveModelSerializers::Model; end

class A::B < ActiveModelSerializers::Model
  attributes :z

  def initialize(attrs)
    super(
      z: attrs['z']&.to_datetime,
    )
  end

  def ok? = z.present?
end
```

### result

```rbs
class A::B < ActiveModelSerializers::Model
  def z: -> DateTime
  def z=: (DateTime z) -> DateTime
  def initialize: (untyped attrs) -> void
  def ok?: -> bool
end
```

## Resolve ActiveModel::Serializer object and association DSL

### update

```rbs
class Item
  def x: -> "x"
end

class User
  def ok?: -> true
  def item: -> Item
  def items: -> Array[Item]
end

class ActiveModel::Serializer
end
```

```ruby
class ASerializer < ActiveModel::Serializer
  attributes :label
  belongs_to :item
  has_many :items

  def label = object.ok? ? "x" : "y"

  def item_name = object.item.x

  def first_item_name = items.first&.x
end
```

### result

```rbs
class ASerializer < ActiveModel::Serializer
  def item: -> Item
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def label: -> "x" | "y"
  def item_name: -> untyped
  def first_item_name: -> "x"?
end
```

## Infer CamelCase classes from attributes DSL ivars

### update

```ruby
class ActiveModelSerializers::Model; end

class Account
  def name = "x"
end

class Presenter < ActiveModelSerializers::Model
  attributes :account, :label

  def initialize(account)
    super(account: account, label: "default")
  end
end
```

### result

```rbs
class Account
  def name: -> "x"
end

class Presenter < ActiveModelSerializers::Model
  def account: -> Account
  def account=: (Account account) -> Account
  def label: -> String
  def label=: (String label) -> "default"
  def initialize: (Account account) -> void
end
```

## Serializer associations in a concern included block resolve on the includer

### update

```ruby
class Item
  def x = "x"
end

module ItemAssociations
  extend ActiveSupport::Concern

  included do
    belongs_to :item
    has_many :items
  end
end

class ASerializer < ActiveModel::Serializer
  include ItemAssociations
end

def item_name = ASerializer.new.item.x
def first_item_name = ASerializer.new.items.first.x
```

### result

```rbs
class ASerializer < ActiveModel::Serializer
  include ItemAssociations

  def item: -> Item
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
end

class Item
  def x: -> "x"
end

module ItemAssociations
  extend ActiveSupport::Concern

  def item: -> Item
  def item=: (Item item) -> Item
  def build_item: -> Item
  def create_item: -> Item
  def items: -> ActiveRecord::Associations::CollectionProxy[Item]
  def item_ids: -> Array[Integer]
  def item_ids=: (Array[Integer] item_ids) -> Array[Integer]
  def items=: (Array[Item] items) -> ActiveRecord::Associations::CollectionProxy[Item]
end

class Object < BasicObject
  def item_name: -> "x"
  def first_item_name: -> "x"
end
```

## AMS model attributes in a concern included block resolve on the includer

### update

```ruby
class ActiveModelSerializers::Model; end

module Presentable
  extend ActiveSupport::Concern

  included do
    attributes :label
  end
end

class Row < ActiveModelSerializers::Model
  include Presentable
end

def result = Row.new.label
```

### result

```rbs
class Object < BasicObject
  def result: -> untyped
end

module Presentable
  extend ActiveSupport::Concern
end

class Row < ActiveModelSerializers::Model
  include Presentable

  def label: -> untyped
end
```
