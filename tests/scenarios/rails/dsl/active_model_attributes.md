# Rails / DSL / Active Model Attributes

## Apply type default and presence to accessors

### update

```ruby
module ActiveModel
  module Attributes
    module ClassMethods
    end
  end

  module Validations
    module ClassMethods
    end
  end
end

class SignupForm
  include ActiveModel::Attributes
  include ActiveModel::Validations

  attribute :age, :integer
  attribute :nickname, :string, default: "guest"
  attribute :joined_at, :datetime

  validates :age, presence: true
end
```

### result

```rbs
class SignupForm
  include ActiveModel::Attributes
  include ActiveModel::Validations

  def age: -> Integer
  def age=: (Integer age) -> Integer
  def nickname: -> String
  def nickname=: (String nickname) -> String
  def joined_at: -> (ActiveSupport::TimeWithZone | DateTime)?
  def joined_at=: ((ActiveSupport::TimeWithZone | DateTime)? joined_at) -> (ActiveSupport::TimeWithZone | DateTime)?
end
```

## Wrap array attribute in Array

### update

```ruby
module ActiveModel
  module Attributes
    module ClassMethods
    end
  end
end

class TagForm
  include ActiveModel::Attributes

  attribute :tags, :string, array: true
end
```

### result

```rbs
class TagForm
  include ActiveModel::Attributes

  def tags: -> Array[String]?
  def tags=: (Array[String]? tags) -> Array[String]?
end
```

## Keep json attribute untyped

### update

```ruby
module ActiveModel
  module Attributes
    module ClassMethods
    end
  end
end

class MetaForm
  include ActiveModel::Attributes

  attribute :meta, :json
end
```

### result

```rbs
class MetaForm
  include ActiveModel::Attributes

  def meta: -> untyped
  def meta=: (untyped meta) -> untyped
end
```
