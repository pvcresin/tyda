# Ruby / Method / Untyped Predicate Return

## Predicate suffix does not force an unknown receiver's return to bool

### update

```ruby
class PredicateValue
  def maybe? = :sentinel
end

class PredicateCaller
  def from_untyped_receiver(receiver)
    receiver.maybe?
  end

  def from_known_receiver = PredicateValue.new.maybe?
end
```

### result

```rbs
class PredicateCaller
  def from_untyped_receiver: (untyped receiver) -> untyped
  def from_known_receiver: -> :sentinel
end

class PredicateValue
  def maybe?: -> :sentinel
end
```
