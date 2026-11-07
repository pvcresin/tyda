## yield nested in a call argument requires a block

### update

```ruby
class NestedYieldProbe
  def wrapper(value)
    value
  end

  def call
    wrapper(yield)
  end
end
```

### result

```rbs
class NestedYieldProbe
  def wrapper: (untyped value) -> untyped
  def call: { () -> untyped } -> untyped
end
```

## Block parameters in nested yields do not become nil

### update

```ruby
class CoreService
  def update_file = nil
end

class CoreServer
  def initialize
    @cores = { "lib" => CoreService.new }
  end

  def each_core
    @cores.each do |_path, core|
      yield core
    end
  end

  def update_all
    each_core { |core| core.update_file }
  end
end
```

### result

```rbs
class CoreServer
  def initialize: -> void
  def each_core: { (untyped) -> untyped } -> { "lib" => CoreService }
  def update_all: -> { "lib" => CoreService }
end

class CoreService
  def update_file: -> nil
end
```
