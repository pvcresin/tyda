# Rails / Active Record / has_one create bang

## Generate the bang creator for a has_one association

### update

```ruby
class ActiveRecord::Base
end

class Configuration < ActiveRecord::Base
end

class BulkImport < ActiveRecord::Base
  has_one :offline_configuration, class_name: "Configuration"
end

BulkImport.new.create_offline_configuration!
```

### result

```rbs
class BulkImport < ActiveRecord::Base
  def offline_configuration: -> Configuration?
  def offline_configuration=: (Configuration offline_configuration) -> Configuration
  def build_offline_configuration: -> Configuration
  def create_offline_configuration: -> Configuration
  def create_offline_configuration!: -> Configuration
end
```
