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

#: (Array[untyped]) -> untyped
def queue_unknown_items(values)
  queue = Queue.new
  values.each { |value| queue << value }
  queue << :stop

  result = nil
  while value = queue.pop
    break if value == :stop
    result = value
  end
  result
end
```

### result

```rbs
class Object < BasicObject
  def queue_value: -> Symbol?
  def queue_aliases: -> (Integer | String)?
  def queue_unknown_items: (Array[untyped] values) -> untyped
end
```

## Mutex and Monitor synchronize return the block value

### update

```ruby
require "monitor"

class SyncBox
  def initialize
    @lock = Mutex.new
    @monitor = Monitor.new
    @items = [1]
  end

  def count = @lock.synchronize { @items.size }
  def label = @monitor.synchronize { "ready" }
end
```

### result

```rbs
class SyncBox
  def initialize: -> void
  def count: -> Integer
  def label: -> "ready"
end
```
