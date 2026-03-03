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
