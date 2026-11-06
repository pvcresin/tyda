# Ruby / Control / Require Flow

## Optional stdlib methods appear after the require runs

### update

```ruby
BEFORE_SHELLWORDS = "tool name".shellescape
require "shellwords"
AFTER_SHELLWORDS = "tool name".shellescape
```

### result

```rbs
BEFORE_SHELLWORDS: untyped
AFTER_SHELLWORDS: String
```

## Optional stdlib methods on lazily loaded classes appear after the require runs

```yaml
known_issue: true
```

### update

```ruby
BEFORE_PP = "tool".pretty_inspect
require "pp"
AFTER_PP = "tool".pretty_inspect

BEFORE_ABBREV = ["fast", "boat"].abbrev
require "abbrev"
AFTER_ABBREV = ["fast", "boat"].abbrev
```

### result

```rbs
BEFORE_PP: untyped
AFTER_PP: String
BEFORE_ABBREV: untyped
AFTER_ABBREV: Hash[String, String]
```

## Method bodies see a require that runs before the method is called

### update

```ruby
def escaped_after_load = "tool name".shellescape
require "shellwords"
escaped_after_load
```

### result

```rbs
class Object < BasicObject
  def escaped_after_load: -> String
end
```

## A call before the require runs its body without the feature

```yaml
known_issue: true
```

### update

```ruby
def inner_escape = "tool name".shellescape
def outer_escape = inner_escape
EARLY = outer_escape
require "shellwords"
```

### result

```rbs
EARLY: untyped
```

## Nested eager require loads the feature for later statements

### update

```ruby
def consume(values) = values
consume([require("shellwords")])
ESCAPED = "nested tool".shellescape
```

### result

```rbs
ESCAPED: String

class Object < BasicObject
  def consume: (Array[bool] values) -> Array[bool]
end
```
