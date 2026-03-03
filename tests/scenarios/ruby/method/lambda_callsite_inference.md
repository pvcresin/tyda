# Ruby / Method / Lambda Callsite Inference

## Infer class body lambda args from call site

### update

```ruby
class Config
  env_int = lambda do |key, val|
    val
  end

  FOO = env_int.call("LIMIT_A", 4096)
  BAR = env_int.call("LIMIT_B", 8192)
end
```

### result

```rbs
class Config
  FOO: 4096
  BAR: 8192
end
```

## Infer class body proc args from call site

### update

```ruby
class Config
  say = proc do |name, count|
    count
  end

  ANSWER = say.call("hello", 42)
end
```

### result

```rbs
class Config
  ANSWER: 42
end
```

## Infer method body lambda args from call site

### update

```ruby
class Runner
  def run
    doubler = lambda do |n|
      n * 2
    end
    doubler.call(21)
  end
end
```

### result

```rbs
class Runner
  def run: -> Integer
end
```

## Infer class body lambda through `.()` call

### update

```ruby
class Config
  env_int = lambda do |k, v|
    v
  end
  LIMIT = env_int.("A", 123)
end
```

### result

```rbs
class Config
  LIMIT: 123
end
```

## Show call-site arg as block arg type in hover

### update

```ruby
class Config
  say_val = lambda do |key, val|
    val
  end
  OUT = say_val.call("A", 123)
end
```

### result

```rbs
class Config
  OUT: 123
end
```
