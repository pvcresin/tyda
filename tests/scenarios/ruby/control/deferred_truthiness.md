# Ruby / Control / Deferred Truthiness

## Project an optional parameter after its call-site type resolves

```ruby
class DeferredTruthyReceiver
  #: () -> String
  def text = "value"

  def self.read(receiver = nil)
    receiver.text if receiver
  end
end

DeferredTruthyReceiver.read(DeferredTruthyReceiver.new)
```

### result

```rbs
class DeferredTruthyReceiver
  def text: -> String
  def self.read: (?DeferredTruthyReceiver? receiver) -> String?
end
```
