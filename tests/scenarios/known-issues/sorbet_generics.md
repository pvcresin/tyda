# Known Issues / Sorbet Generics

## Calling through an RBI interface module keeps T::Helpers out of the output

```rbi
module Named
  extend T::Helpers
  interface!

  sig { abstract.returns(String) }
  def name; end
end

class Finder
  sig { returns(Named) }
  def self.find; end
end
```

```ruby
class FinderUser
  def value = Finder.find.name
end
```

### result

```rbs
class FinderUser
  def value: -> String
end
```
