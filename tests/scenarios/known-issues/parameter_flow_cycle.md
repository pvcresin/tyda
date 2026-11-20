# Parameter flow cycle

## A value read back from the same parameter adds nothing to it

### update

```ruby
class FileCtx; end

class LocalEnv
  def initialize(file_context)
    @file_context = file_context
  end

  attr_reader :file_context
end

class Node
  def initialize(lenv)
    @lenv = lenv
  end

  def child = LocalEnv.new(@lenv.file_context)
end

Node.new(LocalEnv.new(FileCtx.new))
```

### result

```rbs
class FileCtx
end

class LocalEnv
  def initialize: (FileCtx file_context) -> void
  def file_context: -> FileCtx
end

class Node
  def initialize: (LocalEnv lenv) -> void
  def child: -> LocalEnv
end
```
