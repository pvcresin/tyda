# Ruby / Method / Keyword Args

## Required keyword arg

### update

```ruby
def foo(x:) = x
foo(x: "hello")
```

### result

```rbs
class Object < BasicObject
  def foo: (x: String) -> String
end
```

## Optional keyword arg with default

### update

```ruby
def foo(x: 1) = x
```

### result

```rbs
class Object < BasicObject
  def foo: (?x: Integer) -> Integer
end
```

## Combine required and optional args

### update

```ruby
def foo(x:, y: 1) = x
foo(x: "hello")
```

### result

```rbs
class Object < BasicObject
  def foo: (x: String, ?y: Integer) -> String
end
```

## Call overrides optional keyword arg

### update

```ruby
def foo(x: 1) = x
foo(x: "str")
```

### result

```rbs
class Object < BasicObject
  def foo: (?x: (Integer | String)) -> (String | 1)
end
```

## Keyword arg with no calls

### update

```ruby
def foo(x:, y: "default") = x
```

### result

```rbs
class Object < BasicObject
  def foo: (x: untyped, ?y: String) -> untyped
end
```

## Double splat arg

### update

```ruby
def foo(**opts) = 42
```

### result

```rbs
class Object < BasicObject
  def foo: (**untyped opts) -> 42
end
```

## Forward a union of keyword records

```yaml
known_issue: true
```

### update

```ruby
class UnionRecordKeywordSplat
  def target(req:, **rest) = [req, rest]
  def forward(options) = target(req: options[:req], **options)
  def select(flag) = flag ? { req: "a", left: 1 } : { req: "b", right: "x" }
  def call(flag) = forward(select(flag))
end
```

### result

```rbs
class UnionRecordKeywordSplat
  def target: (req: String, **(Integer | String) rest) -> [String, { left: Integer } | { right: String }]
  def forward: (({ req: String, left: Integer } | { req: String, right: String }) options) -> [String, { left: Integer } | { right: String }]
  def select: (untyped flag) -> ({ req: "a", left: 1 } | { req: "b", right: "x" })
  def call: (untyped flag) -> [String, { left: Integer } | { right: String }]
end
```

## `**nil` rejects keyword args

### update

```ruby
def no_keywords(**nil) = 1
```

### result

```rbs
class Object < BasicObject
  def no_keywords: -> 1
end
```

## Infer return type from keyword arg

### update

```ruby
def foo(name:, age: 0) = name
foo(name: "Alice", age: 30)
```

### result

```rbs
class Object < BasicObject
  def foo: (name: String, ?age: Integer) -> String
end
```

## `**opts` enters scope as Hash[Symbol, untyped]

### update

```ruby
class A
  def self.opts(**opts) = opts
  def self.keys(**opts) = opts.keys
end
```

### result

```rbs
class A
  def self.opts: (**untyped opts) -> Hash[Symbol, untyped]
  def self.keys: (**untyped opts) -> Array[Symbol]
end
```

## Keyword rest values flow through nested forwarding

### update

```ruby
class KeywordRestForwarding
  def target(**options) = options
  def forward(**kwargs) = target(**kwargs)
end
KeywordRestForwarding.new.forward(extra: 1)
```

### result

```rbs
class KeywordRestForwarding
  def target: (**Integer options) -> { extra: 1 }
  def forward: (**Integer kwargs) -> { extra: 1 }
end
```

## Static keyword splat

### update

```ruby
def build(name:, count:) = [name, count]

def build_from_literal
  build(**{ name: "entry", count: 1 })
end

def build_from_local
  options = { name: "entry", count: 1 }
  build(**options)
end

def build_with_override
  options = { name: "entry" }
  build(**options, count: 1)
end

def build_from_merge
  options = { name: "entry" }
  build(**options.merge(count: 1))
end
```

### result

```rbs
class Object < BasicObject
  def build: (name: String, count: Integer) -> [String, Integer]
  def build_from_literal: -> [String, Integer]
  def build_from_local: -> [String, Integer]
  def build_with_override: -> [String, Integer]
  def build_from_merge: -> [String, Integer]
end
```

## Deferred record keyword splat resolves at the call site

```yaml
known_issue: true
```

### update

```ruby
class DeferredRecordKeywordSplat
  def target(value:, **extras) = [value, extras]
  def forward(options) = target(value: options[:value], **options)
  def call = forward({ value: "x", flag: true })
end
```

### result

```rbs
class DeferredRecordKeywordSplat
  def target: (value: String, **bool extras) -> [String, { flag: bool }]
  def forward: ({ value: String, flag: bool } options) -> [String, { flag: bool }]
  def call: -> [String, { flag: bool }]
end
```

## Static keyword splat reaches super

### update

```ruby
class Parent
  attr_reader :name, :count

  def initialize(name:, count:)
    @name = name
    @count = count
  end
end

class Child < Parent
  def initialize(name:, count:)
    options = { name:, count: }
    super(**options)
  end
end

def child_values
  child = Child.new(name: "entry", count: 1)
  [child.name, child.count]
end
```

### result

```rbs
class Child < Parent
  def initialize: (name: String, count: Integer) -> void
end

class Object < BasicObject
  def child_values: -> [String, Integer]
end

class Parent
  def name: -> String
  def count: -> Integer
  def initialize: (name: String, count: Integer) -> void
end
```

## Keyword splat widens the optional parameter

```yaml
known_issue: true
```

### update

```ruby
def foo(check: false) = nil

opt = { check: 1 }
foo(**opt)
```

### result

```rbs
class Object < BasicObject
  def foo: (?check: (bool | Integer)) -> nil
end
```

## Read a keyword rest value from caller record variants

```yaml
known_issue: true
```

```ruby
class KeywordRestReadFromRecordVariants
  def target(**rest) = rest[:value]
  def forward(options) = target(**options)
  def call(flag) = forward(flag ? { value: "x", left: true } : { value: "y", right: 1 })
end
```

### result

```rbs
class KeywordRestReadFromRecordVariants
  def target: (**(Integer | String | bool) rest) -> String
  def forward: (({ value: String, left: bool } | { value: String, right: Integer }) options) -> String
  def call: (untyped flag) -> String
end
```

## Dynamic hash branch keeps keyword splat values unknown

```ruby
class DynamicHashKeywordSplatUnion
  def target(**rest) = rest[:left]
  def call(flag) = target(**(flag ? { left: 1 } : Hash.new))
end
```

### result

```rbs
class DynamicHashKeywordSplatUnion
  def target: (**untyped rest) -> untyped
  def call: (untyped flag) -> untyped
end
```

## Preserve Hash method signatures after dynamic keyword splats

```yaml
known_issue: true
```

```ruby
class DynamicHashKeywordSplatCollections
  def collections(**rest) = [rest.keys, rest.values, rest.to_a, rest.entries, rest.to_h, rest.invert]
  def call(flag) = collections(**(flag ? { left: 1 } : Hash.new))
end
```

### result

```rbs
class DynamicHashKeywordSplatCollections
  def collections: (**untyped rest) -> [Array[untyped], Array[untyped], Array[[untyped, untyped]], Array[[untyped, untyped]], Hash[untyped, untyped], Hash[untyped, untyped]]
  def call: (untyped flag) -> [Array[untyped], Array[untyped], Array[[untyped, untyped]], Array[[untyped, untyped]], Hash[untyped, untyped], Hash[untyped, untyped]]
end
```

## Preserve known keys beside untyped record values

```ruby
class KeywordRestReadWithUntypedSibling
  def target(**rest) = rest[:value]
  def call = target(**{ value: "known", other: eval("true") })
end
```

### result

```rbs
class KeywordRestReadWithUntypedSibling
  def target: (**(String | untyped) rest) -> "known"
  def call: -> "known"
end
```

## Read a keyword rest value after multiple forwarding hops

```yaml
known_issue: true
```

```ruby
class KeywordRestReadAfterTwoHops
  def target(**rest) = rest[:value]
  def forward(options) = target(**options)
  def forward_again(options) = forward(options)
  def call(flag) = forward_again(flag ? { value: "x", left: true } : { value: "y", right: 1 })
end
```

### result

```rbs
class KeywordRestReadAfterTwoHops
  def target: (**(Integer | String | bool) rest) -> String
  def forward: (({ value: String, left: bool } | { value: String, right: Integer }) options) -> String
  def forward_again: (({ value: String, left: bool } | { value: String, right: Integer }) options) -> String
  def call: (untyped flag) -> String
end
```

## Read an optional key from a keyword rest record union

```yaml
known_issue: true
```

```ruby
class KeywordRestReadWithEmptyRecord
  def target(**rest) = rest[:only]
  def call(flag) = target(**(flag ? { only: 1 } : {}))
end
```

### result

```rbs
class KeywordRestReadWithEmptyRecord
  def target: (**Integer rest) -> 1?
  def call: (untyped flag) -> 1?
end
```

## Treat nil keyword splats as empty keyword arguments

```yaml
known_issue: true
```

```ruby
class KeywordRestReadWithNilSplatBranch
  def target(**rest) = rest[:value]
  def call(flag) = target(**(flag ? { value: "known", left: true } : nil))
end
```

### result

```rbs
class KeywordRestReadWithNilSplatBranch
  def target: (**(String | bool) rest) -> "known"?
  def call: (untyped flag) -> "known"?
end
```

## Read from a method called with nil keyword splat

```yaml
known_issue: true
```

```ruby
class KeywordRestReadFromNilSplat
  def target(**rest) = rest[:value]
  def call = target(**nil)
end
```

### result

```rbs
class KeywordRestReadFromNilSplat
  def target: (**untyped rest) -> nil
  def call: -> nil
end
```

## Fetch a keyword rest value from caller record variants

```yaml
known_issue: true
```

```ruby
class KeywordRestFetchFromRecordVariants
  def target(**rest) = rest.fetch(:value)
  def forward(options) = target(**options)
  def call(flag) = forward(flag ? { value: "x", left: true } : { value: "y", right: 1 })
end
```

### result

```rbs
class KeywordRestFetchFromRecordVariants
  def target: (**(Integer | String | bool) rest) -> String
  def forward: (({ value: String, left: bool } | { value: String, right: Integer }) options) -> String
  def call: (untyped flag) -> String
end
```

## Use the fetch fallback for missing keyword rest keys

```yaml
known_issue: true
```

```ruby
class KeywordRestFetchWithMissingKey
  def target(**rest) = rest.fetch(:value, "fallback")
  def call(flag) = target(**(flag ? { value: 1 } : {}))
end
```

### result

```rbs
class KeywordRestFetchWithMissingKey
  def target: (**Integer rest) -> (1 | "fallback")
  def call: (untyped flag) -> (1 | "fallback")
end
```

## Infer no normal return when fetch has no key or fallback

```yaml
known_issue: true
```

```ruby
class KeywordRestFetchMissingKey
  def target(**rest) = rest.fetch(:missing)
  def call = target(**{ present: true })
  def literal = {}.fetch(:missing)
end
```

### result

```rbs
class KeywordRestFetchMissingKey
  def target: (**bool rest) -> bot
  def call: -> bot
  def literal: -> bot
end
```

## Keep keyword rest fetches unknown for dynamic hashes

```yaml
known_issue: true
```

```ruby
class KeywordRestFetchFromDynamicHash
  def target(**rest) = rest.fetch(:value, "fallback")
  def call(options) = target(**options)
end
```

### result

```rbs
class KeywordRestFetchFromDynamicHash
  def target: (**untyped rest) -> (untyped | "fallback")
  def call: (untyped options) -> (untyped | "fallback")
end
```

## Use a fetch block only for missing keyword rest keys

```yaml
known_issue: true
```

```ruby
class KeywordRestFetchWithBlock
  def target(**rest) = rest.fetch(:value) { |key| key.to_s }
  def call(flag) = target(**(flag ? { value: 1, left: true } : { right: "x" }))
end
```

### result

```rbs
class KeywordRestFetchWithBlock
  def target: (**(Integer | String | bool) rest) -> (String | 1)
  def call: (untyped flag) -> (String | 1)
end
```

## Ignore a fetch block when a keyword rest fallback is given

```ruby
class KeywordRestFetchDefaultPrecedesBlock
  def target(**rest) = rest.fetch(:value, "fallback") { 0 }
  def call = target(**{ value: 1 })
end
```

### result

```rbs
class KeywordRestFetchDefaultPrecedesBlock
  def target: (**Integer rest) -> 1
  def call: -> 1
end
```

## Dig through a forwarded keyword rest record

```yaml
known_issue: true
```

```ruby
class KeywordRestDigFromRecordVariants
  def target(**rest) = rest.dig(:user, :name)
  def forward(options) = target(**options)
  def forward_again(options) = forward(options)
  def call(flag) = forward_again(flag ? { user: { name: "x", active: true } } : { user: { name: "y", count: 1 } })
end
```

### result

```rbs
class KeywordRestDigFromRecordVariants
  def target: (**({ name: String, active: bool } | { name: String, count: Integer }) rest) -> String
  def forward: (({ user: { name: String, active: bool } } | { user: { name: String, count: Integer } }) options) -> String
  def forward_again: (({ user: { name: String, active: bool } } | { user: { name: String, count: Integer } }) options) -> String
  def call: (untyped flag) -> String
end
```

## Keep nil when a nested dig key is absent

```ruby
class KeywordRestDigWithMissingNestedKey
  def target(**rest) = rest.dig(:user, :name)
  def call(flag) = target(**(flag ? { user: { name: "known" } } : { user: { active: true } }))
end
```

### result

```rbs
class KeywordRestDigWithMissingNestedKey
  def target: (**({ active: bool } | { name: String }) rest) -> "known"?
  def call: (untyped flag) -> "known"?
end
```

## Resolve deferred method returns inside fetch fallbacks

```yaml
known_issue: true
```

```ruby
class KeywordRestFetchWithDeferredFallback
  def fallback(value) = value
  def target(**rest) = rest.fetch(:value, fallback("known"))
  def call(flag) = target(**(flag ? { value: 1 } : {}))
end
```

### result

```rbs
class KeywordRestFetchWithDeferredFallback
  def fallback: (String value) -> String
  def target: (**Integer rest) -> (String | 1)
  def call: (untyped flag) -> (String | 1)
end
```

## Fetch multiple values from caller keyword record variants

```yaml
known_issue: true
```

```ruby
class KeywordRestFetchValuesFromRecordVariants
  def target(**rest) = rest.fetch_values(:value, :label)
  def call(flag) = target(**(flag ? { value: 1, label: "a" } : { value: 2, label: "b" }))
end
```

### result

```rbs
class KeywordRestFetchValuesFromRecordVariants
  def target: (**(Integer | String) rest) -> [1 | 2, "a" | "b"]
  def call: (untyped flag) -> [1 | 2, "a" | "b"]
end
```

## Infer no normal return when fetch_values has a required missing key

```yaml
known_issue: true
```

```ruby
class KeywordRestFetchValuesMissingKey
  def target(**rest) = rest.fetch_values(:missing)
  def call = target(**{ present: true })
end
```

### result

```rbs
class KeywordRestFetchValuesMissingKey
  def target: (**bool rest) -> bot
  def call: -> bot
end
```

## Use fetch_values block results for missing keyword rest keys

```yaml
known_issue: true
```

```ruby
class KeywordRestFetchValuesWithBlock
  def target(**rest) = rest.fetch_values(:value) { |key| key.to_s }
  def call(flag) = target(**(flag ? { value: 1 } : {}))
end
```

### result

```rbs
class KeywordRestFetchValuesWithBlock
  def target: (**Integer rest) -> [Integer | String]
  def call: (untyped flag) -> [Integer | String]
end
```

## Keep keyword rest values_at results tied to caller record variants

```yaml
known_issue: true
```

```ruby
class KeywordRestValuesAtRecordVariants
  def target(**rest) = rest.values_at(:left, :right)
  def call(flag) = target(**(flag ? { left: "x" } : { right: 1 }))
end
```

### result

```rbs
class KeywordRestValuesAtRecordVariants
  def target: (**(Integer | String) rest) -> ["x"?, 1?]
  def call: (untyped flag) -> ["x"?, 1?]
end
```

## Resolve keyword rest cardinality from caller records

```yaml
known_issue: true
```

```ruby
class KeywordRestStaticCardinality
  def fixed(**rest) = [rest.size, rest.length, rest.empty?]
  def varying(**rest) = [rest.size, rest.empty?]
  def uncalled(**rest) = [rest.size, rest.length, rest.empty?]
  def call(flag)
    [
      fixed(**(flag ? { left: "x", count: 1 } : { right: 2, enabled: true })),
      varying(**(flag ? { value: 1 } : {})),
    ]
  end
end
```

### result

```rbs
class KeywordRestStaticCardinality
  def fixed: (**(Integer | String | bool) rest) -> [2, 2, false]
  def varying: (**Integer rest) -> [0 | 1, bool]
  def uncalled: (**untyped rest) -> [Integer, Integer, bool]
  def call: (untyped flag) -> [[2, 2, false], [0 | 1, bool]]
end
```

## Resolve keyword rest collection methods from caller records

```yaml
known_issue: true
```

```ruby
class KeywordRestCollectionMethods
  def keys(**rest) = rest.keys
  def values(**rest) = rest.values
  def pairs(**rest) = rest.to_a
  def entries(**rest) = rest.entries
  def as_hash(**rest) = rest.to_h
  def inverted(**rest) = rest.invert
  def uncalled(**rest) = [rest.keys, rest.values, rest.to_a, rest.entries, rest.to_h, rest.invert]
  def empty_only(**rest) = [rest.keys, rest.values, rest.to_a, rest.entries, rest.to_h, rest.invert]
  def empty_call = empty_only(**{})
  def call(flag)
    input = flag ? { a: 1, b: "x" } : { a: 2, c: true }
    [
      keys(**input), values(**input), pairs(**input), entries(**input),
      as_hash(**input), inverted(**input),
    ]
  end
end
```

### result

```rbs
class KeywordRestCollectionMethods
  def keys: (**(Integer | String | bool) rest) -> Array[:a | :b | :c]
  def values: (**(Integer | String | bool) rest) -> Array[true | 1 | 2 | "x"]
  def pairs: (**(Integer | String | bool) rest) -> Array[[:a | :b | :c, true | 1 | 2 | "x"]]
  def entries: (**(Integer | String | bool) rest) -> Array[[:a | :b | :c, true | 1 | 2 | "x"]]
  def as_hash: (**(Integer | String | bool) rest) -> ({ a: 1, b: "x" } | { a: 2, c: true })
  def inverted: (**(Integer | String | bool) rest) -> Hash[true | 1 | 2 | "x", :a | :b | :c]
  def uncalled: (**untyped rest) -> [Array[Symbol], Array[untyped], Array[[Symbol, untyped]], Array[[Symbol, untyped]], Hash[Symbol, untyped], Hash[untyped, Symbol]]
  def empty_only: (**untyped rest) -> [Array[bot], Array[bot], Array[bot], Array[bot], {  }, Hash[bot, bot]]
  def empty_call: -> [Array[bot], Array[bot], Array[bot], Array[bot], {  }, Hash[bot, bot]]
  def call: (untyped flag) -> [Array[:a | :b | :c], Array[true | 1 | 2 | "x"], Array[[:a | :b | :c, true | 1 | 2 | "x"]], Array[[:a | :b | :c, true | 1 | 2 | "x"]], { a: 1, b: "x" } | { a: 2, c: true }, Hash[true | 1 | 2 | "x", :a | :b | :c]]
end
```
