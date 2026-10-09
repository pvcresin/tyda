# Ruby / Method / Super Calls

## Explicit empty super keeps no arguments

### update

```ruby
class Parent
  def value(x = "default") = x
end

class Child < Parent
  def value(x) = super( )
end

def read = Child.new.value(1)
```

### result

```rbs
class Child < Parent
  def value: (Integer x) -> String
end

class Object < BasicObject
  def read: -> String
end

class Parent
  def value: (?String x) -> String
end
```

## Bare super in defaults forwards nil while super() uses the parent default

```yaml
known_issue: true
```

### update

```ruby
class DefaultSuperParent
  def implicit(value = :parent) = value
  def explicit(value = :parent) = value
  def pair(first, second = :parent) = [first, second]
  def keyword(optional: :parent) = optional
end

class DefaultSuperChild < DefaultSuperParent
  def implicit(value = super) = value
  def explicit(value = super()) = value
  def pair(first, second = super) = second
  def keyword(optional: super) = optional
end

class DefaultSuperCallSite
  def call
    [
      DefaultSuperChild.new.implicit,
      DefaultSuperChild.new.explicit,
      DefaultSuperChild.new.pair("first"),
      DefaultSuperChild.new.keyword,
    ]
  end
end
```

### result

```rbs
class DefaultSuperCallSite
  def call: -> [nil, Symbol, [String, nil], nil]
end

class DefaultSuperChild < DefaultSuperParent
  def implicit: (?nil value) -> nil
  def explicit: (?Symbol value) -> Symbol
  def pair: (String first, ?[String, nil] second) -> [String, nil]
  def keyword: (?optional: nil) -> nil
end

class DefaultSuperParent
  def implicit: (?Symbol? value) -> :parent?
  def explicit: (?Symbol value) -> Symbol
  def pair: (String first, ?Symbol? second) -> [String, :parent?]
  def keyword: (?optional: Symbol?) -> :parent?
end
```

## Bare super defaults use prior and supplied arguments

```yaml
known_issue: true
```

### update

```ruby
class ForwardedDefaultParent
  def triple(first = :parent_first, second = :parent_second, third = :parent_third) = [first, second, third]
  def keyword_forward(value = :parent, later: :parent_later) = later
  def keyword(before: :parent, current: :parent, after: :parent) = [before, current, after]
end

class ForwardedDefaultChild < ForwardedDefaultParent
  def triple(first = :child_first, second = super, third = :child_third) = second
  def keyword_forward(value = super, later: "child_later") = value
  def keyword(before: :child, current: super, after: "child") = current
end

class ForwardedDefaultCallSite
  def multiple = ForwardedDefaultChild.new.triple
  def keyword_forward = ForwardedDefaultChild.new.keyword_forward(later: 1)
  def keyword_current = ForwardedDefaultChild.new.keyword(after: 1)
end
```

### result

```rbs
class ForwardedDefaultCallSite
  def multiple: -> [Symbol, nil, nil]
  def keyword_forward: -> Integer
  def keyword_current: -> [Symbol, nil, Integer]
end

class ForwardedDefaultChild < ForwardedDefaultParent
  def triple: (?Symbol first, ?[Symbol, nil, nil] second, ?Symbol third) -> [Symbol, nil, nil]
  def keyword_forward: (?Integer value, ?later: (Integer | String)) -> Integer
  def keyword: (?before: Symbol, ?current: [Symbol, nil, Integer], ?after: (Integer | String)) -> [Symbol, nil, Integer]
end

class ForwardedDefaultParent
  def triple: (?Symbol first, ?Symbol? second, ?Symbol? third) -> [Symbol, :parent_second?, :parent_third?]
  def keyword_forward: (?Symbol? value, ?later: (Integer | Symbol)) -> (Integer | :parent_later)
  def keyword: (?before: Symbol, ?current: Symbol?, ?after: (Integer | Symbol)) -> [Symbol, :parent?, Integer | :parent]
end
```

## Implicit super forwards the current arguments

### update

```ruby
class Parent
  def value(x) = x.to_s
end

class Child < Parent
  def value(x) = super
end

def read = Child.new.value(1)
```

### result

```rbs
class Child < Parent
  def value: (Integer x) -> String
end

class Object < BasicObject
  def read: -> String
end

class Parent
  def value: (Integer x) -> String
end
```

## Explicit super forwards an expression

### update

```ruby
class Parent
  def value(x) = x.to_s
end

class Child < Parent
  def value(x) = super(x)
end

def read = Child.new.value(1)
```

### result

```rbs
class Child < Parent
  def value: (Integer x) -> String
end

class Object < BasicObject
  def read: -> String
end

class Parent
  def value: (Integer x) -> String
end
```

## Rest arguments forwarded by implicit super keep positional boundaries

```yaml
known_issue: true
```

### update

```ruby
class RestParent
  def pair(first, second) = [first, second]
end

class RestChild < RestParent
  def pair(*args) = super
end

def call_pair = RestChild.new.pair(1, "two")
```

### result

```rbs
class Object < BasicObject
  def call_pair: -> [Integer, String]
end

class RestChild < RestParent
  def pair: (*(Integer | String) args) -> [Integer, String]
end

class RestParent
  def pair: (Integer first, String second) -> [Integer, String]
end
```

## Rest arguments and the caller block pass through implicit super

```yaml
known_issue: true
```

### update

```ruby
class BlockParent
  def render(item)
    yield item if block_given?
  end
end

class BlockChild < BlockParent
  def render(*args, &block) = super
end

def call_render = BlockChild.new.render("value") { |item| item.to_sym }
```

### result

```rbs
class BlockChild < BlockParent
  def render: (*String args) { (String) -> :value } -> :value?
end

class BlockParent
  def render: (String item) { (String) -> :value } -> :value?
end

class Object < BasicObject
  def call_render: -> :value?
end
```

## Keyword rest forwarding through explicit super

### update

```ruby
class KeywordParent2
  def call(required:, optional: :default) = [required, optional]
end

class KeywordChild2 < KeywordParent2
  def call(**kwargs) = super(**kwargs)
end

def call_keyword = KeywordChild2.new.call(required: "value", optional: :set)
```

### result

```rbs
class KeywordChild2 < KeywordParent2
  def call: (**(String | Symbol) kwargs) -> [String, Symbol]
end

class KeywordParent2
  def call: (required: String, ?optional: Symbol) -> [String, Symbol]
end

class Object < BasicObject
  def call_keyword: -> [String, Symbol]
end
```

## `super` inside an included module continues after the module

```yaml
known_issue: true
```

### update

```ruby
class IncludedSuperParent
  def label(value) = [:parent, value]
end

module IncludedSuper
  def label(value) = super(value)
end

class IncludedSuperChild < IncludedSuperParent
  include IncludedSuper
end

class IncludedSuperOtherParent
  def label(value) = [:other, value]
end

class IncludedSuperOtherChild < IncludedSuperOtherParent
  include IncludedSuper
end

def call_included_super = IncludedSuperChild.new.label("value")
def call_included_super_other = IncludedSuperOtherChild.new.label("value")
```

### result

```rbs
module IncludedSuper
  def label: (String value) -> ([:other, String] | [:parent, String])
end

class IncludedSuperChild < IncludedSuperParent
  include IncludedSuper
end

class IncludedSuperOtherChild < IncludedSuperOtherParent
  include IncludedSuper
end

class IncludedSuperOtherParent
  def label: (String value) -> [:other, String]
end

class IncludedSuperParent
  def label: (String value) -> [:parent, String]
end

class Object < BasicObject
  def call_included_super: -> [:other, String] | [:parent, String]
  def call_included_super_other: -> [:other, String] | [:parent, String]
end
```

## Bare super preserves post-required argument positions

```yaml
known_issue: true
```

### update

```ruby
class PostRequiredSuperParent
  def value(value = :parent, required) = required
end

class PostRequiredSuperChild < PostRequiredSuperParent
  def value(value = super, required) = [value, required]
end

class PostRequiredSuperCallSite
  def omitted_optional = PostRequiredSuperChild.new.value(:required)
end
```

### result

```rbs
class PostRequiredSuperCallSite
  def omitted_optional: -> [Symbol, Symbol]
end

class PostRequiredSuperChild < PostRequiredSuperParent
  def value: (?Symbol value, Symbol required) -> [Symbol, Symbol]
end

class PostRequiredSuperParent
  def value: (?Symbol? value, Symbol required) -> Symbol
end
```

## Bare super expands rest arguments and keeps trailing required arguments

```yaml
known_issue: true
```

### update

```ruby
class RestDefaultParent
  def rest(value = :parent, *others) = [value, others]
  def post_rest(value = :parent, *others, required) = [value, others, required]
end

class RestDefaultChild < RestDefaultParent
  def rest(value = super, *others) = [:child, value, others]
  def post_rest(value = super, *others, required) = [:child, value, others, required]
end

class RestDefaultCallSite
  def empty_rest = RestDefaultChild.new.rest
  def post_required = RestDefaultChild.new.post_rest(:required)
end
```

### result

```rbs
class RestDefaultCallSite
  def empty_rest: -> [:child, [nil, Array[untyped]], Array[untyped]]
  def post_required: -> [:child, [nil, Array[untyped], Symbol], Array[untyped], Symbol]
end

class RestDefaultChild < RestDefaultParent
  def rest: (?[nil, Array[untyped]] value, *untyped others) -> [:child, [nil, Array[untyped]], Array[untyped]]
  def post_rest: (?[nil, Array[untyped], Symbol] value, *untyped others, Symbol required) -> [:child, [nil, Array[untyped], Symbol], Array[untyped], Symbol]
end

class RestDefaultParent
  def rest: (?Symbol? value, *untyped others) -> [:parent?, Array[untyped]]
  def post_rest: (?Symbol? value, *untyped others, Symbol required) -> [:parent?, Array[untyped], Symbol]
end
```

## Keyword defaults forward positional and keyword rest values

```yaml
known_issue: true
```

### update

```ruby
class KeywordRestDefaultParent
  def value(*values, option: :parent, **others) = [values, option, others]
end

class KeywordRestDefaultChild < KeywordRestDefaultParent
  def value(*values, option: super, **others) = option
end

class KeywordRestDefaultCallSite
  def value = KeywordRestDefaultChild.new.value(:first, :second, extra: 1)
end
```

### result

```rbs
class KeywordRestDefaultCallSite
  def value: -> [Array[Symbol], nil, Hash[Symbol, Integer]]
end

class KeywordRestDefaultChild < KeywordRestDefaultParent
  def value: (*Symbol values, ?option: [Array[Symbol], nil, Hash[Symbol, Integer]], **Integer others) -> [Array[Symbol], nil, Hash[Symbol, Integer]]
end

class KeywordRestDefaultParent
  def value: (*Symbol values, ?option: Symbol?, **Integer others) -> [Array[Symbol], :parent?, { extra: 1 }]
end
```

## A concrete prepend call keeps the nested `super` result

```yaml
known_issue: true
```

### update

```ruby
class PrependedCallParent
  def value(item) = [:parent, item]
end

module PrependedCallInner
  def value(item) = [:inner, super(item)]
end

module PrependedCallOuter
  def value(item) = [:outer, super(item)]
end

class PrependedCallHost < PrependedCallParent
  prepend PrependedCallInner
  prepend PrependedCallOuter
end

class PrependedCallSite
  def value = PrependedCallHost.new.value(1)
end
```

### result

```rbs
class PrependedCallHost < PrependedCallParent
  prepend PrependedCallInner
  prepend PrependedCallOuter
end

module PrependedCallInner
  def value: (Integer item) -> [:inner, [:parent, Integer]]
end

module PrependedCallOuter
  def value: (Integer item) -> [:outer, [:inner, [:parent, Integer]]]
end

class PrependedCallParent
  def value: (Integer item) -> [:parent, Integer]
end

class PrependedCallSite
  def value: -> [:outer, [:inner, [:parent, Integer]]]
end
```

## A nested expression keeps an included module `super` result

```yaml
known_issue: true
```

### update

```ruby
class NestedSuperParent
  def label(value) = value.to_s
end

module NestedSuperMethods
  def label(value) = [:nested, super(value)]
end

class NestedSuperChild < NestedSuperParent
  include NestedSuperMethods
end

def call_nested_super = NestedSuperChild.new.label(1)
```

### result

```rbs
class NestedSuperChild < NestedSuperParent
  include NestedSuperMethods
end

module NestedSuperMethods
  def label: (Integer value) -> [:nested, String]
end

class NestedSuperParent
  def label: (Integer value) -> String
end

class Object < BasicObject
  def call_nested_super: -> [:nested, String]
end
```

## Nested returns resolve across a chain of included modules

```yaml
known_issue: true
```

### update

```ruby
class NestedChainParent
  def label(value) = value.to_s
end

module NestedChainInner
  def label(value) = super(value)
end

module NestedChainOuter
  def label(value) = [:outer, super(value)]
end

class NestedChainChild < NestedChainParent
  include NestedChainInner
  include NestedChainOuter
end

def call_nested_chain = NestedChainChild.new.label(1)
```

### result

```rbs
class NestedChainChild < NestedChainParent
  include NestedChainInner
  include NestedChainOuter
end

module NestedChainInner
  def label: (Integer value) -> String
end

module NestedChainOuter
  def label: (Integer value) -> [:outer, String]
end

class NestedChainParent
  def label: (Integer value) -> String
end

class Object < BasicObject
  def call_nested_chain: -> [:outer, String]
end
```

## Separate `super` expressions resolve inside records and arrays

```yaml
known_issue: true
```

### update

```ruby
class CompositeSuperParent
  def value(item) = item.to_s
end

module CompositeSuperMethods
  def value(item) = { direct: super(item), nested: [super(item)] }
end

class CompositeSuperChild < CompositeSuperParent
  include CompositeSuperMethods
end

def call_composite_super = CompositeSuperChild.new.value(:item)
```

### result

```rbs
class CompositeSuperChild < CompositeSuperParent
  include CompositeSuperMethods
end

module CompositeSuperMethods
  def value: (Symbol item) -> { direct: String, nested: [String] }
end

class CompositeSuperParent
  def value: (Symbol item) -> String
end

class Object < BasicObject
  def call_composite_super: -> { direct: String, nested: [String] }
end
```

## A deferred `super` result resolves inside a Proc

```yaml
known_issue: true
```

### update

```ruby
class ProcSuperParent
  def value(item) = item.to_s
end

module ProcSuperMethods
  def value(item) = proc { super(item) }
end

class ProcSuperChild < ProcSuperParent
  include ProcSuperMethods
end

def call_proc_super = ProcSuperChild.new.value(1).call
```

### result

```rbs
class Object < BasicObject
  def call_proc_super: -> String
end

class ProcSuperChild < ProcSuperParent
  include ProcSuperMethods
end

module ProcSuperMethods
  def value: (Integer item) -> Proc
end

class ProcSuperParent
  def value: (Integer item) -> String
end
```

## `super` follows an included method before the superclass

```yaml
known_issue: true
```

### update

```ruby
class LayeredSuperParent
  def value = "parent"
end

module LayeredSuperMixin
  def value = super
end

class LayeredSuperChild < LayeredSuperParent
  include LayeredSuperMixin
  def value = super
end

def call_layered_super = LayeredSuperChild.new.value
```

### result

```rbs
class LayeredSuperChild < LayeredSuperParent
  include LayeredSuperMixin

  def value: -> "parent"
end

module LayeredSuperMixin
  def value: -> "parent"
end

class LayeredSuperParent
  def value: -> "parent"
end

class Object < BasicObject
  def call_layered_super: -> "parent"
end
```

## `super` in an extended module follows the singleton chain

```yaml
known_issue: true
```

### update

```ruby
class ExtendedSuperParent
  def self.value = :parent
end

module ExtendedSuperMethods
  def value = super
end

class ExtendedSuperChild < ExtendedSuperParent
  extend ExtendedSuperMethods
end

def read_extended_super = ExtendedSuperChild.value
```

### result

```rbs
class ExtendedSuperChild < ExtendedSuperParent
  extend ExtendedSuperMethods
end

module ExtendedSuperMethods
  def value: -> :parent
end

class ExtendedSuperParent
  def self.value: -> :parent
end

class Object < BasicObject
  def read_extended_super: -> :parent
end
```

## An included module forwards keyword rest through `super`

```yaml
known_issue: true
```

### update

```ruby
class KeywordSuperParent
  def render(item, label:, **options) = [item, label, options]
end

module KeywordSuperMethods
  def render(item, **kwargs) = super(item, **kwargs)
end

class KeywordSuperChild < KeywordSuperParent
  include KeywordSuperMethods
end

def call_keyword_super = KeywordSuperChild.new.render("value", label: :title, extra: 1)
```

### result

```rbs
class KeywordSuperChild < KeywordSuperParent
  include KeywordSuperMethods
end

module KeywordSuperMethods
  def render: (String item, **(Integer | Symbol) kwargs) -> [String, Symbol, Hash[Symbol, untyped]]
end

class KeywordSuperParent
  def render: (String item, label: Symbol, **Integer options) -> [String, Symbol, { extra: 1 }]
end

class Object < BasicObject
  def call_keyword_super: -> [String, Symbol, Hash[Symbol, untyped]]
end
```

## An included module forwards a block through `super`

```yaml
known_issue: true
```

### update

```ruby
class BlockSuperParent
  def render(item)
    yield item if block_given?
  end
end

module BlockSuperMethods
  def render(item, &block) = super(item, &block)
end

class BlockSuperChild < BlockSuperParent
  include BlockSuperMethods
end

def call_block_super = BlockSuperChild.new.render("value") { |item| item.to_sym }
```

### result

```rbs
class BlockSuperChild < BlockSuperParent
  include BlockSuperMethods
end

module BlockSuperMethods
  def render: (String item) { (String) -> Symbol } -> Symbol?
end

class BlockSuperParent
  def render: (String item) { (String) -> Symbol } -> Symbol?
end

class Object < BasicObject
  def call_block_super: -> Symbol?
end
```

## An included module joins block types from every `super` target

```yaml
known_issue: true
```

### update

```ruby
class MultiBlockStringParent
  def render(item)
    yield item if block_given?
  end
end

class MultiBlockSymbolParent
  def render(item)
    yield item.to_sym if block_given?
  end
end

module MultiBlockSuperMethods
  def render(item, &block) = super(item, &block)
  def call_render = render("value") { |item| item.to_s }
end

class MultiBlockStringChild < MultiBlockStringParent
  include MultiBlockSuperMethods
end

class MultiBlockSymbolChild < MultiBlockSymbolParent
  include MultiBlockSuperMethods
end

def call_multi_block_string = MultiBlockStringChild.new.render("value") { |item| item.to_s }
def call_multi_block_symbol = MultiBlockSymbolChild.new.render("value") { |item| item.to_s }
```

### result

```rbs
class MultiBlockStringChild < MultiBlockStringParent
  include MultiBlockSuperMethods
end

class MultiBlockStringParent
  def render: (String item) { (String) -> String } -> String?
end

module MultiBlockSuperMethods
  def render: (String item) { ((String | Symbol)) -> String } -> String?
  def call_render: -> String?
end

class MultiBlockSymbolChild < MultiBlockSymbolParent
  include MultiBlockSuperMethods
end

class MultiBlockSymbolParent
  def render: (String item) { (Symbol) -> String } -> String?
end

class Object < BasicObject
  def call_multi_block_string: -> String?
  def call_multi_block_symbol: -> String?
end
```

## A method call supplies yielded argument types to its block

```yaml
known_issue: true
```

### update

```ruby
class DirectBlockParent
  def render(item)
    yield item if block_given?
  end
end

def call_direct_render = DirectBlockParent.new.render("value") { |item| item.to_sym }
```

### result

```rbs
class DirectBlockParent
  def render: (String item) { (String) -> :value } -> :value?
end

class Object < BasicObject
  def call_direct_render: -> :value?
end
```

## Yield types resolve positional and keyword method parameters

```yaml
known_issue: true
```

### update

```ruby
class KeywordBlock
  def render(item:, marker: :default)
    yield item, marker
  end
end

def call_keyword_render = KeywordBlock.new.render(item: "value", marker: :tag) { |item, marker| :ok }
```

### result

```rbs
class KeywordBlock
  def render: (item: String, ?marker: Symbol) { (String, Symbol) -> :ok } -> :ok
end

class Object < BasicObject
  def call_keyword_render: -> :ok
end
```
