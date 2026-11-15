# Known Issues / Instance Variable Mutation

## A replayed method pushes onto an instance variable once

### update

```ruby
class Errors
  attr_reader :errors

  def initialize
    @errors = []
  end

  def check(x)
    later(x)
  rescue ArgumentError
    @errors.push("a")
    nil
  end

  def later(x) = x
end

Errors.new.check(1)
```

### result

```rbs
class Errors
  def errors: -> Array["a"]
  def initialize: -> void
  def check: (Integer x) -> Integer?
  def later: [X] (X x) -> X
end
```
