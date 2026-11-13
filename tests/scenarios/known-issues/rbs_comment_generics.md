# Known Issues / RBS Comment Generics

## An interface embedded with @rbs!

```ruby
class Greeter
  # @rbs!
  #   interface _Named
  #     def name: () -> String
  #   end

  #: (_Named x) -> String
  def greet(x) = x.name
end
```

### result

```rbs
class Greeter
  interface _Named
    def name: -> String
  end

  def greet: (_Named x) -> String
end
```
