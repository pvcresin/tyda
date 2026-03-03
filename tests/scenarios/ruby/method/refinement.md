# Ruby / Method / Refinement

## Refinement stays lexical and participates in inference

### update

```ruby
module Words
  refine String do
    def loud = upcase
  end
end

def before(value) = value.loud
def before_read = before("hello")

using Words

def call_loud(value) = value.loud

def read = call_loud("hello")
```

### result

```rbs
class Object < BasicObject
  def before: (String value) -> untyped
  def before_read: -> untyped
  def call_loud: (String value) -> String
  def read: -> String
end
```
