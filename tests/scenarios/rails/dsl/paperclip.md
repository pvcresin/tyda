# Rails / DSL / Paperclip

## Generate attachment readers from model declarations

### update

```ruby
class ActiveRecord::Base
end

class Backup < ActiveRecord::Base
  has_attached_file :file
end
```

### result

```rbs
class Backup < ActiveRecord::Base
  def file: -> untyped
end
```

## A project definition of the macro still receives the call

### update

```ruby
class ActiveRecord::Base
end

class Backup < ActiveRecord::Base
  def self.has_attached_file(name, options = {}) = name

  has_attached_file :file, styles: 1
end
```

### result

```rbs
class Backup < ActiveRecord::Base
  def self.has_attached_file: [N] (N name, ?(Hash[untyped, untyped] | { styles: Integer }) options) -> N
  def file: -> untyped
end
```
