# Nested constant qualification

## A nested class from a sibling method keeps its full name in an instance variable

### update

```ruby
class AST
  def self.create(raw) = SigFuncType.new(raw)

  class SigFuncType
    attr_reader :blk

    def initialize(raw) = @blk = raw ? AST.create(nil) : nil
  end
end
```

### result

```rbs
class AST
  def self.create: (nil raw) -> AST::SigFuncType
end

class AST::SigFuncType
  def blk: -> AST::SigFuncType?
  def initialize: (nil raw) -> void
end
```
