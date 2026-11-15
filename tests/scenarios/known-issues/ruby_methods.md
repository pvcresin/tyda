# Known Issues / Ruby Methods

## A yield inside an each block passes the element type

```ruby
class NameList
  def each_name(names)
    names.each { |name| yield name }
  end
end

NameList.new.each_name(["a"]) { |name| name.size }
```

### result

```rbs
class NameList
  def each_name: (Array[String] names) { (String) -> Integer } -> Array[String]
end
```
