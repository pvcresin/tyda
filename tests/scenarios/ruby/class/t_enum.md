# Ruby / Class / T::Enum

## Basic T::Enum with method

### update

```ruby
class Suit < T::Enum
  enums do
    Hearts = new
    Diamonds = new
  end

  def display_name = "suit"
end
```

### result

```rbs
class Suit < T::Enum
  def display_name: -> "suit"
end
```
