# Ruby / Class / Module reflection

## `Module#name`, `Module#ancestors`, and `Module#instance_methods`

### update

```ruby
class Foo
  def greet = "hi"
end

class CustomAncestors
  def self.ancestors = 1
end

module AncestorsOverride
  def ancestors = 2
end

class ExtendedAncestors
  extend AncestorsOverride
end

class UnknownBaseChild < UnknownBase
end

class Probe
  def class_name        = Foo.name
  def my_class          = self.class
  def my_class_name     = self.class.name
  def instance_methods  = Foo.instance_methods
  def ancestors         = Foo.ancestors
  def custom_ancestors  = CustomAncestors.ancestors
  def extended_ancestors = ExtendedAncestors.ancestors
  def unknown_ancestors = UnknownBaseChild.ancestors
end
```

### result

```rbs
module AncestorsOverride
  def ancestors: -> 2
end

class CustomAncestors
  def self.ancestors: -> 1
end

class ExtendedAncestors
  extend AncestorsOverride
end

class Foo
  def greet: -> "hi"
end

class Probe
  def class_name: -> String?
  def my_class: -> singleton(Probe)
  def my_class_name: -> String?
  def instance_methods: -> Array[Symbol]
  def ancestors: -> [singleton(Foo), singleton(Object), singleton(Kernel), singleton(BasicObject)]
  def custom_ancestors: -> 1
  def extended_ancestors: -> 2
  def unknown_ancestors: -> Array[Module]
end
```
