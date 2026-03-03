# Sorbet / RBS Comment / Inline Assertions

## #: inline cast works without Sorbet detection

### update

```ruby
class Plain
  def value
    x = 1 #: as String
    x
  end
end
```

### result

```rbs
class Plain
  def value: -> String
end
```

## Ignore @rbs comment when Sorbet is detected

### update

`sorbet/config`

```ruby
.
```

```ruby
# @rbs () -> String
def sorbet_plain_value = 1
```

### result

```rbs
class Object
  def sorbet_plain_value: -> 1
end
```

## as cast in expression context

### update

`sorbet/config`

```ruby
.
```

```ruby
class Caster
  #: (Integer) -> Integer
  def id(x) = x

  def ints(x) = id(x #: as Integer)
end
```

### result

```rbs
class Caster
  def id: (Integer x) -> Integer
  def ints: (untyped x) -> Integer
end
```

## Remove nil with as !nil

### update

`sorbet/config`

```ruby
.
```

```ruby
class Muster
  #: (String?) -> String
  def unwrap(x)
    y = x #: as !nil
    y
  end
end
```

### result

```rbs
class Muster
  def unwrap: (String? x) -> String
end
```

## Use as untyped as escape hatch

### update

`sorbet/config`

```ruby
.
```

```ruby
class UnsafeComment
  def escape
    x = 1 #: as untyped
    x
  end
end
```

### result

```rbs
class UnsafeComment
  def escape: -> untyped
end
```

## Type assertions for ivar and constant

### update

`sorbet/config`

```ruby
.
```

```ruby
class Storage
  X = "1" #: as Integer

  def load
    @value = "2" #: as Integer
    @value
  end

  def self.value = X
end
```

### result

```rbs
class Storage
  X: Integer

  def load: -> Integer
  def self.value: -> Integer
end
```

## absurd comment

### update

`sorbet/config`

```ruby
.
```

```ruby
class Exhaustive
  def never(x)
    x #: absurd
  end
end
```

### result

```rbs
class Exhaustive
  def never: (untyped x) -> bot
end
```

## Type assertion on HEREDOC first line

### update

`sorbet/config`

```ruby
.
```

```ruby
class Doc
  X = <<~MSG #: Integer
    hello
  MSG

  def self.value = X
end
```

### result

```rbs
class Doc
  X: Integer

  def self.value: -> Integer
end
```

## Inline #: T annotation on attr_reader

### update

```ruby
class Profile
  attr_reader :email #: String
  attr_accessor :count #: Integer
end
```

### result

```rbs
class Profile
  def email: -> String
  def count: -> Integer
  def count=: (Integer count) -> Integer
end
```
