# Ruby / Class / Module reflection

## `Module#name`, `Module#ancestors`, and `Module#instance_methods`

### update

```ruby
class Foo
  def greet = "hi"
end

class Probe
  def class_name        = Foo.name
  def my_class          = self.class
  def my_class_name     = self.class.name
  def instance_methods  = Foo.instance_methods
  def ancestors         = Foo.ancestors
end
```

### result

```rbs
class Foo
  def greet: -> "hi"
end

class Probe
  def class_name: -> String?
  def my_class: -> singleton(Probe)
  def my_class_name: -> String?
  def instance_methods: -> Array[Symbol]
  def ancestors: -> Array[Module]
end
```
