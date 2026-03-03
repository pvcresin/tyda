# Rails / DSL / Oj

## Resolve Oj module functions

### update

```ruby
class PayloadCodec
  def parse(body)
    Oj.load(body)
  end

  def parse_strict(body)
    Oj.strict_load(body)
  end

  def parse_compat(body)
    Oj.compat_load(body)
  end

  def emit(obj)
    Oj.dump(obj)
  end

  def emit_json(obj)
    Oj.generate(obj)
  end
end
```

### result

```rbs
class PayloadCodec
  def parse: (untyped body) -> untyped
  def parse_strict: (untyped body) -> untyped
  def parse_compat: (untyped body) -> untyped
  def emit: (untyped obj) -> String
  def emit_json: (untyped obj) -> String
end
```

## Project-defined Oj wins over the plugin

### update

```ruby
module Oj
  def self.load(body)
    42
  end
end

class UserParser
  def parse(body)
    Oj.load(body)
  end
end
```

### result

```rbs
module Oj
  def self.load: (untyped body) -> 42
end

class UserParser
  def parse: (untyped body) -> 42
end
```
