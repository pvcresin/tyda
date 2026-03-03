# Ruby / Method / RSpec structural DSL

## let and subject stay lexical to the example group

### update

```ruby
class Calculator
  def add(left, right) = left + right
end

RSpec.describe Calculator do
  subject { described_class.new }
  let(:sum) { subject.add(1, 2) }
  let!(:eager_sum) { subject.add(3, 4) }
  let_it_be(:stored_sum) { subject.add(5, 6) }

  RESULT = sum
  EAGER_RESULT = eager_sum
  STORED_RESULT = stored_sum
end

def result = RESULT
def eager_result = EAGER_RESULT
def stored_result = STORED_RESULT
def outside = subject
```

### result

```rbs
RESULT: untyped
EAGER_RESULT: untyped
STORED_RESULT: untyped

class Calculator
  def add: (Integer left, Integer right) -> Integer
end

class Object < BasicObject
  def result: -> Integer
  def eager_result: -> Integer
  def stored_result: -> Integer
  def outside: -> untyped
end
```
