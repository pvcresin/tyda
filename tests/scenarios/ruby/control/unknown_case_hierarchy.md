# Ruby / Control / Unknown Case Hierarchy

## Keep a case branch reachable when the superclass is unresolved

### update

```ruby
class KnownParent
  def self.===(value) = value.is_a?(self)
end

def unknown_parent_case
  child = UnresolvedHierarchy::Child.new
  case child
  when KnownParent
    :parent
  else
    :other
  end
end
```

```rbs
class UnresolvedHierarchy::Child < UnresolvedHierarchy::MissingParent
end
```

### result

```rbs
class KnownParent
  def self.===: (untyped value) -> bool
end

class Object < BasicObject
  def unknown_parent_case: -> :other | :parent
end
```
