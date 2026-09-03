# Ruby / Control / Test assertion narrowing

## Minitest and RSpec assertions narrow a local

### update

```ruby
def minitest(value)
  assert_instance_of(String, value)
  value.upcase
end

def rspec(value)
  expect(value).to be_a(String)
  value.upcase
end

#: (String | nil) -> String
def rspec_not_nil(value)
  expect(value).not_to be_nil
  value.upcase
end

def rspec_nil(value)
  expect(value).to be_nil
  value
end
```

### result

```rbs
class Object < BasicObject
  def minitest: (untyped value) -> String
  def rspec: (untyped value) -> String
  def rspec_not_nil: (String? value) -> String
  def rspec_nil: (untyped value) -> nil
end
```
