# Rails / DSL / Active Hash

## Infer field accessors and finders from self.data

### update

```ruby
class ActiveHash::Base
end

class Country < ActiveHash::Base
  self.data = [
    { id: 1, name: "Japan", code: :jp, enabled: true },
    { id: 2, name: "France", code: :fr, enabled: false }
  ]
end
```

### result

```rbs
class Country < ActiveHash::Base
  def code: -> :fr | :jp
  def code=: (Symbol code) -> (:fr | :jp)
  def code?: -> bool
  def self.find_by_code: (Symbol code) -> Country?
  def self.find_all_by_code: (Symbol code) -> Array[Country]
  def enabled: -> bool
  def enabled=: (bool enabled) -> bool
  def enabled?: -> bool
  def self.find_by_enabled: (bool enabled) -> Country?
  def self.find_all_by_enabled: (bool enabled) -> Array[Country]
  def id: -> 1 | 2
  def id=: (Integer id) -> (1 | 2)
  def id?: -> bool
  def self.find_by_id: (Integer id) -> Country?
  def self.find_all_by_id: (Integer id) -> Array[Country]
  def name: -> "France" | "Japan"
  def name=: (String name) -> ("France" | "Japan")
  def name?: -> bool
  def self.find_by_name: (String name) -> Country?
  def self.find_all_by_name: (String name) -> Array[Country]
  def self.find: (Integer id) -> Country
end
```

## Infer ActiveHash scope as class method

### update

```ruby
class ActiveHash::Base
end

class Team < ActiveHash::Base
  self.data = [
    { id: 1, name: "Red" }
  ]

  scope :red, -> { all }
end
```

### result

```rbs
class Team < ActiveHash::Base
  def id: -> 1
  def id=: (Integer id) -> 1
  def id?: -> bool
  def self.find_by_id: (Integer id) -> Team?
  def self.find_all_by_id: (Integer id) -> Array[Team]
  def name: -> "Red"
  def name=: (String name) -> "Red"
  def name?: -> bool
  def self.find_by_name: (String name) -> Team?
  def self.find_all_by_name: (String name) -> Array[Team]
  def self.find: (Integer id) -> Team
  def self.red: -> ActiveHash::Relation[Team]
end
```
