# Rails / DSL / Gettext

## Resolve translation helpers to String

### update

```ruby
class ApplicationHelper
  def label = _('users.label')

  def singular = s_('Menu|Label')

  def plural(n) = n_('one', 'many', n)
end
```

### result

```rbs
class ApplicationHelper
  def label: -> String
  def singular: -> String
  def plural: (untyped n) -> String
end
```

## Nn_ returns the marked keys

```yaml
known_issue: true
```

### update

```ruby
class ApplicationHelper
  def keys = Nn_('one', 'many')
end
```

### result

```rbs
class ApplicationHelper
  def keys: -> Array[String]
end
```

## np is not a translation helper

```yaml
known_issue: true
```

### update

```ruby
class ApplicationHelper
  def context_plural(n) = np('Menu', 'one', 'many', n)
end
```

### result

```rbs
class ApplicationHelper
  def context_plural: (untyped n) -> untyped
end
```

## Resolve nsgettext and npgettext to String

```yaml
known_issue: true
```

### update

```ruby
class ApplicationHelper
  def scoped_plural(n) = nsgettext('Menu|one', 'Menu|many', n)

  def context_plural(n) = npgettext('Menu', 'one', 'many', n)
end
```

### result

```rbs
class ApplicationHelper
  def scoped_plural: (untyped n) -> String
  def context_plural: (untyped n) -> String
end
```

## N_ returns its argument

```yaml
known_issue: true
```

### update

```ruby
class ApplicationHelper
  def mark(key) = N_(key)
end
```

### result

```rbs
class ApplicationHelper
  def mark: (untyped key) -> untyped
end
```
