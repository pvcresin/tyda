# Ruby / Method / Visibility

## bare private / protected / public sections

### update

```ruby
class A
  def public_method = "public"

  private

  def private_method = "private"

  protected

  def protected_method = "protected"

  public

  def public_again = "again"
end
```

### result

```rbs
class A
  def public_method: -> "public"
  private def private_method: -> "private"
  private def protected_method: -> "protected"
  def public_again: -> "again"
end
```

## inline private def

### update

```ruby
class A
  private def secret = 42

  def public_info = "info"
end
```

### result

```rbs
class A
  private def secret: -> 42
  def public_info: -> "info"
end
```

## private symbol

### update

```ruby
class A
  def foo = 1
  def bar = 2

  private :foo
end
```

### result

```rbs
class A
  private def foo: -> 1
  def bar: -> 2
end
```

## private forward reference

### update

```ruby
class A
  private :foo

  def foo = 1
  def bar = 2
end
```

### result

```rbs
class A
  private def foo: -> 1
  def bar: -> 2
end
```

## private_class_method

### update

```ruby
class A
  def self.public_class_method = "public"

  private_class_method def self.secret_class_method = "secret"
end
```

### result

```rbs
class A
  def self.public_class_method: -> "public"
  private def self.secret_class_method: -> "secret"
end
```

## private_class_method symbol

### update

```ruby
class A
  def self.foo = 1
  def self.bar = 2

  private_class_method :foo
end
```

### result

```rbs
class A
  private def self.foo: -> 1
  def self.bar: -> 2
end
```

## private def self singleton

### update

```ruby
class A
  private def self.secret = "secret"

  def self.visible = "visible"
end
```

### result

```rbs
class A
  private def self.secret: -> "secret"
  def self.visible: -> "visible"
end
```

## module_function unaffected by private section

### update

```ruby
module M
  private

  module_function

  def helper = 1
end
```

### result

```rbs
module M
  def helper: -> 1
  def self.helper: -> 1
end
```

## private inside class self block

### update

```ruby
class A
  class << self
    def visible = 1

    private

    def hidden = 2
  end
end
```

### result

```rbs
class A
  def self.visible: -> 1
  private def self.hidden: -> 2
end
```
