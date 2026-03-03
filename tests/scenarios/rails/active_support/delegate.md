# Rails / Active Support / Delegate

## Generate delegated methods

### update

```ruby
class User
  delegate :name, :email, to: :profile
end
```

### result

```rbs
class User
  def name: -> untyped
  def email: -> untyped
end
```

## Delegate a single method

### update

```ruby
class Order
  delegate :currency, to: :payment
end
```

### result

```rbs
class Order
  def currency: -> untyped
end
```

## Apply delegate prefix true

### update

```ruby
class Category
  #: () -> String
  def name = "Books"
end

class Product
  #: () -> Category
  def category = Category.new

  delegate :name, to: :category, prefix: true
end
```

### result

```rbs
class Category
  def name: -> String
end

class Product
  def category: -> Category
  def category_name: -> String
end
```

## Apply custom delegate prefix

### update

```ruby
class Customer
  #: () -> String
  def email = "x@example.com"
end

class Invoice
  #: () -> Customer
  def buyer = Customer.new

  delegate :email, to: :buyer, prefix: :customer
end
```

### result

```rbs
class Customer
  def email: -> String
end

class Invoice
  def buyer: -> Customer
  def customer_email: -> String
end
```

## delegate follows ivar targets

### update

```ruby
class Inner
  #: () -> String
  def head = "x"
end

class Wrapper
  #: (Inner inner) -> void
  def initialize(inner)
    @inner = inner
  end

  delegate :head, to: :@inner
end
```

### result

```rbs
class Inner
  def head: -> String
end

class Wrapper
  def initialize: (Inner inner) -> void
  def head: -> String
end
```

## delegate allow_nil keeps nil

### update

```ruby
class User
  #: () -> String
  def email = "x@example.com"
end

class Account
  #: () -> User?
  def user = nil

  delegate :email, to: :user, prefix: true, allow_nil: true
end
```

### result

```rbs
class Account
  def user: -> User?
  def user_email: -> String?
end

class User
  def email: -> String
end
```

## delegate [] follows target type

### update

```ruby
class Item
  #: () -> Integer
  def value = 1
end

class Box
  #: () -> Array[Item]
  def items = [Item.new]

  delegate :[], to: :items

  def first_value = self[0]&.value
end
```

### result

```rbs
class Box
  def items: -> Array[Item]
  def []: (*Integer args, **untyped kwargs) -> untyped
  def first_value: -> Integer?
end

class Item
  def value: -> Integer
end
```

## delegate allow_nil on untyped target stays untyped

### update

```ruby
class Widget
  delegate :weight, to: :thing, allow_nil: true

  def use = weight
end
```

### result

```rbs
class Widget
  def weight: -> untyped
  def use: -> untyped
end
```

## delegate on untyped target without allow_nil stays untyped

### update

```ruby
class Widget
  delegate :weight, to: :thing

  def use = weight
end
```

### result

```rbs
class Widget
  def weight: -> untyped
  def use: -> untyped
end
```

## delegate follows with_options to and allow_nil

### update

```ruby
class Settings
  #: () -> bool
  def archived = true
end

class Namespace
  #: () -> Settings?
  def settings = Settings.new

  with_options to: :settings do
    with_options allow_nil: true do
      delegate :archived
    end
  end

  def read = archived
end
```

### result

```rbs
class Namespace
  def settings: -> Settings?
  def archived: -> bool?
  def read: -> bool?
end

class Settings
  def archived: -> bool
end
```

## delegate splat constant keys

### update

```ruby
class Profile
  FIELD_MAP = { name: :name, email: :email }

  delegate(*FIELD_MAP.keys, to: :@resource)

  def read = name
end
```

### result

```rbs
class Profile
  FIELD_MAP: { name: :name, email: :email }

  def name: -> untyped
  def email: -> untyped
  def read: -> untyped
end
```

## delegate hash rocket to accessor

### update

```ruby
class Score
  extend Forwardable

  delegate %i[soc_list idf_hash] => :data_set

  def read = soc_list
end
```

### result

```rbs
class Score
  extend Forwardable

  def soc_list: -> untyped
  def idf_hash: -> untyped
  def read: -> untyped
end
```

## Compose nested delegate options

### update

```ruby
class ProjectSetting
  #: () -> Integer?
  def default_git_depth = nil

  #: (Integer? value) -> Integer?
  def default_git_depth=(value) = value
end

class Project
  #: () -> ProjectSetting?
  def ci_cd_settings = nil

  with_options to: :ci_cd_settings, allow_nil: true do
    with_options prefix: :ci do
      delegate :default_git_depth, :default_git_depth=
    end
  end
end
```

### result

```rbs
class Project
  def ci_cd_settings: -> ProjectSetting?
  def ci_default_git_depth: -> Integer?
  def ci_default_git_depth=: (Integer? value) -> Integer?
end

class ProjectSetting
  def default_git_depth: -> Integer?
  def default_git_depth=: (Integer? value) -> Integer?
end
```
