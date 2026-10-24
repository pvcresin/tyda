## Yield branch-local assigned value

### update

```ruby
def with_error(flag)
  if flag
    error = StandardError.new
    yield error
  end
end

def consume_error = with_error(true) { |error| error.backtrace }
```

### result

```rbs
class Object < BasicObject
  def with_error: (bool flag) { (StandardError) -> Array[String]? } -> Array[String]?
  def consume_error: -> Array[String]?
end
```

## Preserve nil yielded on a branch

### update

```ruby
def with_optional_error(include_nil)
  if include_nil
    yield nil
  else
    error = StandardError.new
    yield error
  end
  :done
end

def consume_optional_error(include_nil) = with_optional_error(include_nil) { |error| error }
```

### result

```rbs
class Object < BasicObject
  def with_optional_error: (untyped include_nil) { (StandardError?) -> untyped } -> :done
  def consume_optional_error: (untyped include_nil) -> :done
end
```

## Keep type from yield before reassignment

### update

```ruby
def yield_then_clear
  error = StandardError.new
  yield error
  error = nil
end

def consume_before_clear = yield_then_clear { |error| error.backtrace }
```

### result

```rbs
class Object < BasicObject
  def yield_then_clear: { (StandardError) -> Array[String]? } -> nil
  def consume_before_clear: -> nil
end
```

## Union types from multiple yields

### update

```ruby
def yield_union(flag)
  if flag
    yield 1
  else
    yield "text"
  end
  :done
end

def consume_union(flag) = yield_union(flag) { |value| value.to_s }
```

### result

```rbs
class Object < BasicObject
  def yield_union: (untyped flag) { ((Integer | String)) -> untyped } -> :done
  def consume_union: (untyped flag) -> :done
end
```

## Yield nested in a string mutation argument

### update

```ruby
class YieldFlow
  def content_tag(_tag, content) = content

  def render_nested(flag)
    if flag
      value = "text"
      html = +""
      html << content_tag("li", yield(value))
    end
    :done
  end

  def consume_nested(flag) = render_nested(flag) { |value| value.to_s }
end
```

### result

```rbs
class YieldFlow
  def content_tag: (String _tag, untyped content) -> untyped
  def render_nested: (untyped flag) { (String) -> untyped } -> :done
  def consume_nested: (untyped flag) -> :done
end
```

## Named block call preserves the call-site type

### update

```ruby
class YieldFlow
  def content_tag(_tag, content) = content

  def render_nested(flag, &block)
    if flag
      value = "text"
      html = +""
      html << content_tag("li", block.call(value))
    end
    :done
  end

  def consume_nested(flag) = render_nested(flag) { |value| value.to_s }
end
```

### result

```rbs
class YieldFlow
  def content_tag: (String _tag, untyped content) -> untyped
  def render_nested: (untyped flag) { (String) -> untyped } -> :done
  def consume_nested: (untyped flag) -> :done
end
```
