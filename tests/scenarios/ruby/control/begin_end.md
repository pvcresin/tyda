# Ruby / Control / Begin End

## Use constant defined in BEGIN block later

### update

```ruby
BEGIN {
  BOOT_VALUE = 1
}

class A
  def foo = BOOT_VALUE
end
```

### result

```rbs
BOOT_VALUE: 1

class A
  def foo: -> 1
end
```

## Use method defined in BEGIN block later

### update

```ruby
BEGIN {
  def boot_value = "boot"
}

def use_boot_value = boot_value
```

### result

```rbs
class Object < BasicObject
  def boot_value: -> "boot"
  def use_boot_value: -> "boot"
end
```
