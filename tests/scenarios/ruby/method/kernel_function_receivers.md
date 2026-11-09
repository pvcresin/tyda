# Ruby / Method / Kernel Function Receivers

## An explicit receiver uses its own declaration, not a Kernel function

```ruby
class Library
  def load_data(data) = Marshal.load(data)
  def seeded = Random.new(1).rand(100)
  def input_line = ARGF.lineno
end
```

### result

```rbs
class Library
  def load_data: (untyped data) -> untyped
  def seeded: -> Integer
  def input_line: -> Integer
end
```

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

## Only a self receiver can call a private method

```ruby
class Vault
  def peek = self.secret
  def print_self = self.print

  private def secret = 1
end

class Visitor
  def peek = Vault.new.secret
end
```

### result

```rbs
class Vault
  def peek: -> 1
  def print_self: -> nil
  private def secret: -> 1
end

class Visitor
  def peek: -> untyped
end
```
