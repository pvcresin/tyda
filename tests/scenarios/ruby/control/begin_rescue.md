# Ruby / Control / Begin Rescue

## Union types from begin and rescue

### update

```ruby
def foo
  begin
    "ok"
  rescue
    :error
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> "ok" | :error
end
```

## Same type in begin and rescue

### update

```ruby
def foo
  begin
    1
  rescue
    2
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> 1 | 2
end
```

## Multiple rescue clauses

### update

```ruby
def foo
  begin
    "ok"
  rescue ArgumentError
    :arg_error
  rescue
    42
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> 42 | "ok" | :arg_error
end
```

## begin without rescue

### update

```ruby
def foo
  begin
    42
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> 42
end
```

## begin rescue else uses else type for success path

### update

```ruby
def foo
  begin
    1
  rescue
    :error
  else
    "ok"
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> "ok" | :error
end
```

## Apply begin rescue else assignment to later local

### update

```ruby
def foo
  x = :start

  begin
    1
  rescue
    x = :rescue
  else
    x = :else
  end

  x
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> :else | :rescue
end
```

## Apply ensure assignment to local after begin rescue

### update

```ruby
def foo
  x = :start

  begin
    x = :body
  rescue
    x = :rescue
  ensure
    x = :ensure
  end

  x
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> :ensure
end
```

## Local reassigned inside begin unions across the method

### update

```ruby
def test(cond, val)
  if cond
    begin
      val = val.to_i
    rescue
      raise "bad"
    end
  end
  val
end

test(true, "42")
test(false, "hello")
```

### result

```rbs
class Object < BasicObject
  def test: (bool cond, String val) -> (Integer | String)
end
```
