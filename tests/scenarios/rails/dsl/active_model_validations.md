# Rails / DSL / Active Model Validations

## Resolve errors and validity checks on models

### update

```ruby
class ActiveRecord::Base; end

class Report < ActiveRecord::Base
  def issues = errors

  def ok? = valid?

  def broken? = invalid?
end
```

### result

```rbs
class Report < ActiveRecord::Base
  def issues: -> ActiveModel::Errors
  def ok?: -> bool
  def broken?: -> bool
end
```

## Resolve errors on ActiveModel includers

### update

```ruby
module ActiveModel::Model; end

class SearchForm
  include ActiveModel::Model

  def issues = errors
end
```

### result

```rbs
class SearchForm
  include ActiveModel::Model

  def issues: -> ActiveModel::Errors
end
```
