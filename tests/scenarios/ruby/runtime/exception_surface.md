# Ruby / Runtime / Exception Surface

## Resolve exception accessors on unknown error classes

### update

```ruby
class ErrorHandler
  def safe_parse
    parse!
  rescue Vendor::ApiError => e
    e.message
  end

  def trace
    parse!
  rescue Vendor::ApiError => e
    e.backtrace
  end
end
```

### result

```rbs
class ErrorHandler
  def safe_parse: -> String | untyped
  def trace: -> nil | untyped | Array[String]
end
```
