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
