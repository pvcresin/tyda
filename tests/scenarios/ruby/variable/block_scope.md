# Ruby / Variable / Block Scope

## Local assignment in block is visible outside

### update

```ruby
class A
  def f
    x = :outer
    [1].each do
      x = :inner
    end
    x
  end
end
```

### result

```rbs
class A
  def f: -> :inner | :outer
end
```

## Block arg shadows outer local only inside block

### update

```ruby
class A
  def f
    x = :outer
    [1].each do |x|
      x
    end
    x
  end
end
```

### result

```rbs
class A
  def f: -> :outer
end
```

## Block-local `|; x|` does not affect outer local

### update

```ruby
class A
  def f
    x = :outer
    [1].each do |; x|
      x = :inner
    end
    x
  end
end
```

### result

```rbs
class A
  def f: -> :outer
end
```

## `for` loop variable stays in outer scope

### update

```ruby
class A
  def f
    for i in [1, 2, 3]
    end
    i
  end
end
```

### result

```rbs
class A
  def f: -> 1 | 2 | 3
end
```
