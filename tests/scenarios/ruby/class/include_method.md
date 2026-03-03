# Ruby / Class / Include Method

## Call methods from included module

### update

```ruby
module Printable
  #: () -> String
  def to_display = "display"
end

class Item
  include Printable

  def show = to_display
end
```

### result

```rbs
class Item
  include Printable

  def show: -> String
end

module Printable
  def to_display: -> String
end
```

## Include multiple modules

### update

```ruby
module Nameable
  #: () -> String
  def full_name = "name"
end

module Ageable
  #: () -> Integer
  def age = 0
end

class Person
  include Nameable
  include Ageable

  def summary = full_name
end
```

### result

```rbs
module Ageable
  def age: -> Integer
end

module Nameable
  def full_name: -> String
end

class Person
  include Nameable
  include Ageable

  def summary: -> String
end
```
