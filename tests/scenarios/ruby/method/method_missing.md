# Ruby / Method / Method Missing

## method_missing and respond_to_missing?

### update

```ruby
class A
  def method_missing(name, *args)
    "handled"
  end

  def respond_to_missing?(name, include_private = false)
    true
  end
end
```

### result

```rbs
class A
  def method_missing: (untyped name, *untyped args) -> "handled"
  def respond_to_missing?: (untyped name, ?bool include_private) -> true
end
```

## send and define_method

### update

```ruby
class A
  def call_send
    send(:to_s)
  end

  def use_define
    define_method(:hello) { "world" }
  end
end
```

### result

```rbs
class A
  def call_send: -> String
  def use_define: -> untyped
end
```

## Keep local param name for singleton method_missing

### update

```ruby
class A
  class << self
    def method_missing(method, *args)
      args.first
    end
  end
end
```

### result

```rbs
class A
  def self.method_missing: (untyped method, *untyped args) -> untyped
end
```
