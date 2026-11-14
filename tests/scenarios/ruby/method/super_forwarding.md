## Explicit rest splat in super preserves positional boundaries

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
  def pair: [F, S] (F first, S second) -> [F, S]
end

class Object < BasicObject
  def call_explicit_splat: -> [Integer, String]
end
```

## Forwarded arguments through explicit super ellipsis

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
  def call: (*String, **Symbol) ?{ (?) -> untyped } -> [String, Symbol]
end

class EllipsisParent
  def call: [R] (R required, ?optional: Symbol) -> [R, Symbol]
end

class Object < BasicObject
  def call_ellipsis: -> [String, Symbol]
end
```

## Explicit block forwarding through super retains the block signature

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
  def call: [R] (String required) { (String) -> R } -> R
end

class BlockParent2
  def call: [Req, Ret] (Req required) { (Req) -> Ret } -> Ret
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
  def call: [R] (R required, ?optional: Integer) -> [R, 1]
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
  def identity: [V] (V value) -> V
end
```

## A block literal on explicit super is forwarded to the parent

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
  def call: [I, R] (I item) { (I) -> R } -> R
end

class Object < BasicObject
  def call_explicit_super_block: -> Symbol
end
```

## A block literal on implicit super replaces the caller's block

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
  def call: [I, R] (I item) { (I) -> R } -> R
end

class Object < BasicObject
  def call_implicit_super_block: -> Symbol
end
```

## An explicit empty super still forwards its attached block

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
  def call: [R] (?Symbol item) { (String) -> R } -> R
end

class Object < BasicObject
  def call_empty_super_block: -> Symbol
end
```
