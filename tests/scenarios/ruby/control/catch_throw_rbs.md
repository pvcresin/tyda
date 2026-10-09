## catch follows the official RBS return type without a throw

### update

```ruby
def catch_normal = catch(:finish) { :normal }
```

### result

```rbs
class Object < BasicObject
  def catch_normal: -> untyped
end
```

## catch follows the official RBS return type for a matching throw

### update

```ruby
def catch_throw = catch(:finish) { throw :finish, :done }
```

### result

```rbs
class Object < BasicObject
  def catch_throw: -> untyped
end
```

## catch follows the official RBS return type for a throw without a value

### update

```ruby
def catch_throw_without_value = catch(:finish) { throw :finish }
```

### result

```rbs
class Object < BasicObject
  def catch_throw_without_value: -> untyped
end
```

## catch follows the official RBS return type across normal and throw paths

### update

```ruby
def catch_conditional(flag)
  catch(:finish) do
    if flag
      throw :finish, :early
    else
      :normal
    end
  end
end
```

### result

```rbs
class Object < BasicObject
  def catch_conditional: (untyped flag) -> untyped
end
```
