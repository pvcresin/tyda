# Known Issues / Kernel Functions

## An explicit receiver cannot call a private Kernel function

```ruby
class Printer
  def own = 1
end

class Caller
  def explicit_select(x) = Printer.new.select(x)
  def explicit_print = Printer.new.print
end
```

### result

```rbs
class Caller
  def explicit_select: (untyped x) -> untyped
  def explicit_print: -> untyped
end

class Printer
  def own: -> 1
end
```
