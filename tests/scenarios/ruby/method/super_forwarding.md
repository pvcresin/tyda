## Explicit rest splat in super preserves positional boundaries

```yaml
known_issue: true
```

### update

```ruby
class ExplicitSplatParent
  def pair(first, second) = [first, second]
end

class ExplicitSplatChild < ExplicitSplatParent
  def pair(*args) = super(*args)
end

def call_explicit_splat = ExplicitSplatChild.new.pair(1, "two")
```

### result

```rbs
class ExplicitSplatChild < ExplicitSplatParent
  def pair: (*(Integer | String) args) -> [Integer, String]
end

class ExplicitSplatParent
  def pair: (Integer first, String second) -> [Integer, String]
end

class Object < BasicObject
  def call_explicit_splat: -> [Integer, String]
end
```

## Forwarded arguments through explicit super ellipsis

```yaml
known_issue: true
```

### update

```ruby
class EllipsisParent
  def call(required, optional: :default) = [required, optional]
end

class EllipsisChild < EllipsisParent
  def call(...) = super(...)
end

def call_ellipsis = EllipsisChild.new.call("value", optional: :set)
```

### result

```rbs
class EllipsisChild < EllipsisParent
  def call: (*String, **Symbol, ?untyped &block) -> [String, Symbol]
end

class EllipsisParent
  def call: (String required, ?optional: Symbol) -> [String, Symbol]
end

class Object < BasicObject
  def call_ellipsis: -> [String, Symbol]
end
```

## Explicit block forwarding through super retains the block signature

```yaml
known_issue: true
```

### update

```ruby
class BlockParent2
  def call(required) = yield required
end

class BlockChild2 < BlockParent2
  def call(required, &block) = super(required, &block)
end

def call_block = BlockChild2.new.call("value") { |value| value.to_sym }
```

### result

```rbs
class BlockChild2 < BlockParent2
  def call: (String required) { (String) -> :value } -> :value
end

class BlockParent2
  def call: (String required) { (String) -> :value } -> :value
end

class Object < BasicObject
  def call_block: -> :value
end
```

## `super(**nil)` suppresses keyword forwarding

### update

```ruby
class KeywordNilParent
  def call(required, optional: 1) = [required, optional]
end

class KeywordNilChild < KeywordNilParent
  def call(required, **kwargs) = super(required, **nil)
end

def call_keyword_nil = KeywordNilChild.new.call("value", optional: "ignored")
```

### result

```rbs
class KeywordNilChild < KeywordNilParent
  def call: (String required, **String kwargs) -> [String, 1]
end

class KeywordNilParent
  def call: (String required, ?optional: Integer) -> [String, 1]
end

class Object < BasicObject
  def call_keyword_nil: -> [String, 1]
end
```

## Forwarded argument context resolves through a wrapper method

### update

```ruby
class ContextParent
  def identity(value) = value
end

class ContextChild < ContextParent
  def identity(value) = super(value)
end

class ContextCaller
  def wrapper(value) = ContextChild.new.identity(value)
  def run = wrapper("known")
end
```

### result

```rbs
class ContextCaller
  def wrapper: (String value) -> String
  def run: -> String
end

class ContextChild < ContextParent
  def identity: (String value) -> String
end

class ContextParent
  def identity: (String value) -> String
end
```

## A block literal on explicit super is forwarded to the parent

```yaml
known_issue: true
```

### update

```ruby
class ExplicitBlockParent
  def call(item) = yield item
end

class ExplicitBlockChild < ExplicitBlockParent
  def call(item) = super(item) { |value| value.to_sym }
end

def call_explicit_super_block = ExplicitBlockChild.new.call("value") { |value| value }
```

### result

```rbs
class ExplicitBlockChild < ExplicitBlockParent
  def call: (String item) -> Symbol
end

class ExplicitBlockParent
  def call: (String item) { (String) -> Symbol } -> Symbol
end

class Object < BasicObject
  def call_explicit_super_block: -> Symbol
end
```

## A block literal on implicit super replaces the caller's block

```yaml
known_issue: true
```

### update

```ruby
class ImplicitBlockParent
  def call(item) = yield item
end

class ImplicitBlockChild < ImplicitBlockParent
  def call(item) = super { |value| value.to_sym }
end

def call_implicit_super_block = ImplicitBlockChild.new.call("value") { |value| value }
```

### result

```rbs
class ImplicitBlockChild < ImplicitBlockParent
  def call: (String item) -> Symbol
end

class ImplicitBlockParent
  def call: (String item) { (String) -> Symbol } -> Symbol
end

class Object < BasicObject
  def call_implicit_super_block: -> Symbol
end
```

## An explicit empty super still forwards its attached block

```yaml
known_issue: true
```

### update

```ruby
class EmptyBlockParent
  def call(item = :default) = yield item.to_s
end

class EmptyBlockChild < EmptyBlockParent
  def call(item) = super() { |value| value.to_sym }
end

def call_empty_super_block = EmptyBlockChild.new.call("ignored") { |value| value }
```

### result

```rbs
class EmptyBlockChild < EmptyBlockParent
  def call: (String item) -> Symbol
end

class EmptyBlockParent
  def call: (?Symbol item) { (String) -> Symbol } -> Symbol
end

class Object < BasicObject
  def call_empty_super_block: -> Symbol
end
```
