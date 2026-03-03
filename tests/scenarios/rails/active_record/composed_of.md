# Rails / Active Record / Composed Of

## Generate reader and writer from class_name

### update

```ruby
class A
  composed_of :balance, class_name: "Money"
end
```

### result

```rbs
class A
  def balance: -> Money
  def balance=: (Money balance) -> Money
end
```

## Default class name from attribute name

### update

```ruby
class B
  composed_of :address
end
```

### result

```rbs
class B
  def address: -> Address
  def address=: (Address address) -> Address
end
```
