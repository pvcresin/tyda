# Enum values from a method

## An enum mapping a class method returns defines its scopes

### update

```ruby
class ActiveRecord::Base; end
class ApplicationRecord < ActiveRecord::Base; end

module PipelineEnums
  def self.sources
    { push: 1, external: 6 }
  end
end

class Pipeline < ApplicationRecord
  enum :source, PipelineEnums.sources

  def self.external_only = external
end
```

### result

```rbs
module PipelineEnums
  def self.sources: -> { push: 1, external: 6 }
end

class Pipeline < ApplicationRecord
  def source: -> String
  def self.sources: -> Hash[String, Integer]
  def push?: -> bool
  def push!: -> bool
  def self.push: -> ActiveRecord::Relation[Pipeline]
  def external?: -> bool
  def external!: -> bool
  def self.external: -> ActiveRecord::Relation[Pipeline]
  def self.external_only: -> ActiveRecord::Relation[Pipeline]
end
```
