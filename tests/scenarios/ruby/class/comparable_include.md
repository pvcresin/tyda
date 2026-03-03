# Ruby / Class / Comparable include

## Comparable include adds comparison operators from `<=>`

### update

```ruby
class Version
  include Comparable

  def initialize(major) = @major = major
  attr_reader :major
  def <=>(other) = @major <=> other.major
end

class Probe
  def lt        = Version.new(1) <  Version.new(2)
  def le        = Version.new(1) <= Version.new(2)
  def gt        = Version.new(2) >  Version.new(1)
  def ge        = Version.new(2) >= Version.new(1)
  def between   = Version.new(1).between?(Version.new(0), Version.new(2))
  def clamp     = Version.new(1).clamp(Version.new(0), Version.new(2))
end
```

### result

```rbs
class Probe
  def lt: -> bool
  def le: -> bool
  def gt: -> bool
  def ge: -> bool
  def between: -> bool
  def clamp: -> Version
end

class Version
  include Comparable

  def initialize: (Integer major) -> void
  def major: -> 0 | 1 | 2
  def <=>: (untyped other) -> untyped
end
```
