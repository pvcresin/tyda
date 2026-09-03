# Ruby / Runtime / Concurrency

## Thread value returns the block result

### update

```ruby
def thread_value = Thread.new { 1 }.value
def thread_join_value = Thread.new { :ready }.join.value
```

### result

```rbs
class Object < BasicObject
  def thread_value: -> Integer
  def thread_join_value: -> Symbol
end
```

## Fiber resume returns the yielded value

### update

```ruby
def fiber_value
  fiber = Fiber.new do
    Fiber.yield :ready
    :done
  end
  fiber.resume
end
```

### result

```rbs
class Object < BasicObject
  def fiber_value: -> :ready
end
```

## Queue receives a typed value

### update

```ruby
def queue_value
  queue = Queue.new
  queue << :ready
  queue.pop
end

def queue_aliases
  queue = Queue.new
  queue.push(1)
  queue.enq("ready")
  queue.deq
end
```

### result

```rbs
class Object < BasicObject
  def queue_value: -> Symbol?
  def queue_aliases: -> (Integer | String)?
end
```
