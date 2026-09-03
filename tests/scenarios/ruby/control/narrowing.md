# Ruby / Control / Narrowing

## Truthiness narrows then branch to non-nil

### update

```ruby
class Truthiness
  #: (String?) -> String
  def check(x)
    if x
      x.upcase
    else
      "none"
    end
  end
end
```

### result

```rbs
class Truthiness
  def check: (String? x) -> String
end
```

## Unless guard narrows body after early return

### update

```ruby
class UnlessGuard
  #: (Integer?) -> Integer
  def calc(x)
    unless x
      return 0
    end
    x + 1
  end
end
```

### result

```rbs
class UnlessGuard
  def calc: (Integer? x) -> Integer
end
```

## Early return guard narrows rest of method

### update

```ruby
class EarlyReturn
  #: (String?) -> String
  def process(x)
    return "" unless x
    x.upcase
  end
end
```

### result

```rbs
class EarlyReturn
  def process: (String? x) -> String
end
```

## Raise guard narrows rest of method

### update

```ruby
class RaiseGuard
  #: (Integer?) -> Integer
  def calc(x)
    raise "nil" if x.nil?
    x + 1
  end
end
```

### result

```rbs
class RaiseGuard
  def calc: (Integer? x) -> Integer
end
```

## nil? splits nil and non-nil branches

### update

```ruby
class NilCheck
  #: (String?) -> String
  def up(x)
    if x.nil?
      "none"
    else
      x.upcase
    end
  end
end
```

### result

```rbs
class NilCheck
  def up: (String? x) -> String
end
```

## is_a? narrows both branches of a union

### update

```ruby
class IsAGuard
  #: (Integer | String) -> String
  def check(x)
    if x.is_a?(Integer)
      x.to_s
    else
      x.upcase
    end
  end
end
```

### result

```rbs
class IsAGuard
  def check: ((Integer | String) x) -> String
end
```

## case when narrows the subject per branch

### update

```ruby
class CaseWhen
  #: (Integer | String) -> String
  def classify(v)
    case v
    when Integer
      v.to_s
    when String
      v.upcase
    else
      "unknown"
    end
  end
end
```

### result

```rbs
class CaseWhen
  def classify: ((Integer | String) v) -> String
end
```

## case in narrows the subject per pattern

### update

```ruby
class CaseIn
  #: (Integer | String) -> String
  def classify(v)
    case v
    in Integer
      v.to_s
    in String
      v.upcase
    end
  end
end
```

### result

```rbs
class CaseIn
  def classify: ((Integer | String) v) -> String
end
```

## else branch narrows to the complement

### update

```ruby
class ElseComplement
  #: (String?) -> (Symbol | 1)
  def convert(x)
    if x.nil?
      1
    else
      x.to_sym
    end
  end
end
```

### result

```rbs
class ElseComplement
  def convert: (String? x) -> (Symbol | 1)
end
```

## Reassignment resets narrowing

### update

```ruby
class ReassignReset
  #: (String?) -> String?
  def process(x)
    return "" unless x
    x = fetch
    x
  end

  #: -> String?
  def fetch = rand > 0.5 ? "s" : nil
end
```

### result

```rbs
class ReassignReset
  def process: (String? x) -> String?
  def fetch: -> String?
end
```

## AND conjunction narrows both operands

### update

```ruby
class AndConjunction
  #: (Integer | String | nil, Integer | String | nil) -> String
  def join(a, b)
    if a.is_a?(String) && b.is_a?(String)
      a + b
    else
      ""
    end
  end
end
```

### result

```rbs
class AndConjunction
  def join: ((Integer | String)? a, (Integer | String)? b) -> String
end
```

## Unnarrowable condition keeps the type unchanged

### update

```ruby
class ComplexCond
  #: (String?) -> String?
  def process(x)
    if helper(x)
      x
    else
      x
    end
  end

  #: (String?) -> bool
  def helper(x) = !x.nil?
end
```

### result

```rbs
class ComplexCond
  def process: (String? x) -> String?
  def helper: (String? x) -> bool
end
```

## Truthy ivar guard narrows to a non-nil class method

### update

```ruby
class Journal
  def user = "u"
end

class Issue
  def guard_if
    @current_journal = rand < 0.5 ? Journal.new : nil
    if @current_journal
      @current_journal.user
    end
  end
end
```

### result

```rbs
class Issue
  def guard_if: -> "u"?
end

class Journal
  def user: -> "u"
end
```

## Ivar short-circuit AND keeps the narrowed method return

### update

```ruby
class Journal
  def user = "u"
end

class Issue
  def guard_and
    @current_journal = rand < 0.5 ? Journal.new : nil
    @current_journal && @current_journal.user
  end
end
```

### result

```rbs
class Issue
  def guard_and: -> "u"
end

class Journal
  def user: -> "u"
end
```

## unless ivar.nil? guard narrows the ivar in the body

### update

```ruby
class Journal
  def user = "u"
end

class Issue
  def guard_unless_nil
    @current_journal = rand < 0.5 ? Journal.new : nil
    unless @current_journal.nil?
      @current_journal.user
    end
  end
end
```

### result

```rbs
class Issue
  def guard_unless_nil: -> "u"?
end

class Journal
  def user: -> "u"
end
```

## Diverging helper on or RHS narrows the LHS

### update

```ruby
def src = rand < 0.5 ? "x" : nil
def fail_now = raise "no"

def go
  x = src or fail_now
  x
end
```

### result

```rbs
class Object < BasicObject
  def src: -> "x"?
  def fail_now: -> bot
  def go: -> "x"
end
```

## Inherited always-raising helper narrows after a nil guard

### update

```ruby
class Base
  def boom(_msg) = raise "stop"
end

class Sub < Base
  def run
    x = rand < 0.5 ? "hello" : nil
    boom("nil") if x.nil?
    x
  end
end
```

### result

```rbs
class Base
  def boom: (untyped _msg) -> bot
end

class Sub < Base
  def run: -> "hello"
end
```

## Included always-raising helper narrows after a nil guard

### update

```ruby
module Helpers
  def boom(_msg) = raise "stop"
end

class Worker
  include Helpers

  def run
    x = rand < 0.5 ? "hello" : nil
    boom("nil") if x.nil?
    x
  end
end
```

### result

```rbs
module Helpers
  def boom: (untyped _msg) -> bot
end

class Worker
  include Helpers

  def run: -> "hello"
end
```

## Implicit-self always-raising helper narrows after a nil guard

### update

```ruby
def boom = raise "stop"

def go
  x = rand < 0.5 ? "hello" : nil
  boom if x.nil?
  x
end
```

### result

```rbs
class Object < BasicObject
  def boom: -> bot
  def go: -> "hello"
end
```

## Raise unless narrows nil away

### update

```ruby
def foo
  n = [1, nil].sample
  raise "x" unless n
  n
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> 1
end
```

## Raise assignment leaves a falsy residual

### update

```ruby
def foo
  n = [1, nil].sample
  if n
    n = raise
    1
  end
  n
end
```

### result

```rbs
class Object < BasicObject
  def foo: -> nil
end
```

## Literal equality else is the complement

### update

```ruby
def foo(x)
  if x == :a
    :yes
  else
    x
  end
end

foo(:a)
foo(:b)
```

### result

```rbs
class Object < BasicObject
  def foo: (Symbol x) -> Symbol
end
```
