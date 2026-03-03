# Ruby / Control / Return

## Union type with early return

### update

```ruby
def foo(x)
  return "early" if x
  42
end
```

### result

```rbs
class Object
  def foo: (untyped x) -> (42 | "early")
end
```

## Union type with return nil

### update

```ruby
def foo(x)
  return nil if x
  "hello"
end
```

### result

```rbs
class Object
  def foo: (untyped x) -> "hello"?
end
```

## `return true if cond` returns true or nil

### update

```ruby
def maybe_true(x)
  return true if x
end
```

### result

```rbs
class Object
  def maybe_true: (untyped x) -> true | nil
end
```

## return without value

### update

```ruby
def foo(x)
  return if x
  42
end
```

### result

```rbs
class Object
  def foo: (untyped x) -> 42?
end
```

## Do not mix unreachable tail expression into return

### update

```ruby
def always_true
  return true
  1
end
```

### result

```rbs
class Object
  def always_true: -> true
end
```

## Multiple return statements

### update

```ruby
def classify(x)
  return :negative if x
  return :zero if x
  :positive
end
```

### result

```rbs
class Object
  def classify: (untyped x) -> (:negative | :positive | :zero)
end
```

## Early return nil does not remove tail untyped

### update

```ruby
def extract(x)
  return if x.nil?

  element = x.first
  element['href']
end
```

### result

```rbs
class Object
  def extract: (untyped x) -> (nil | untyped)
end
```

## += widens literal counter to Integer

### update

```ruby
def count(items)
  total = 0

  items.each do |_item|
    total += 1
  end

  total
end
```

### result

```rbs
class Object
  def count: (untyped items) -> Integer
end
```

## Block-internal return joins the enclosing method return union

### update

```ruby
def scan(item)
  [item].each { |element| return :found if element }
  0
end

def a = scan(true)
def b = scan(false)
```

### result

```rbs
class Object
  def scan: (bool item) -> (0 | :found)
  def a: -> 0 | :found
  def b: -> 0 | :found
end
```

## Multi-value return packs a tuple into the method union

### update

```ruby
def pair(flag)
  return 1, "x" if flag
  [2, "y"]
end

def a = pair(true)
def b = pair(false)
```

### result

```rbs
class Object
  def pair: (bool flag) -> ([1, "x"] | [2, "y"])
  def a: -> [1, "x"] | [2, "y"]
  def b: -> [1, "x"] | [2, "y"]
end
```
