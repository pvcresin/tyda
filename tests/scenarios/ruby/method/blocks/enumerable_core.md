# Ruby / Method / Blocks / Enumerable Core

## map with type transformation

### update

```ruby
def int_to_string = [1, 2, 3].map { |x| x.to_s }
```

### result

```rbs
class Object < BasicObject
  def int_to_string: -> Array[String]
end
```

## Integer array repetition remains an array

### update

```ruby
def join_repeated_values = (["entry"] * [1, 2].size).join(" ")
```

### result

```rbs
class Object < BasicObject
  def join_repeated_values: -> String
end
```

## Untyped repetition count does not choose the String overload

### update

```ruby
def join_repeated_values_with_untyped_count(tokens) = (["entry"] * tokens.size).join(" ")
```

### result

```rbs
class Object < BasicObject
  def join_repeated_values_with_untyped_count: (untyped tokens) -> untyped
end
```

## Deferred repetition counts do not choose the String overload

### update

```ruby
class DeferredRepeatCount
  def from_param(n) = ["entry"] * n
  def from_method(n) = ["entry"] * n.size
  def local_result(n)
    repeated = ["entry"] * n.to_i
    repeated.join(",")
  end
  def literal_count = ["entry"] * 2
  def literal_separator = ["entry"] * ","
end
```

### result

```rbs
class DeferredRepeatCount
  def from_param: (untyped n) -> untyped
  def from_method: (untyped n) -> untyped
  def local_result: (untyped n) -> untyped
  def literal_count: -> Array["entry"]
  def literal_separator: -> "entry"
end
```

## Re-infer a block call after its receiver's source return is resolved

### update

```ruby
class DeferredMapReceiver
  def self.headers_with_index
    index = -1
    headers.map { |header| [header, index += 1] }
  end

  def self.headers
    ["id".to_s]
  end
end
```

### result

```rbs
class DeferredMapReceiver
  def self.headers_with_index: -> Array[[String, Integer]]
  def self.headers: -> [String]
end
```

## Preserve the array result when a block return is deferred

### update

```ruby
class DeferredMapResult
  def self.load_raw(value) = value

  def self.map_after_guard(value)
    values = [value].compact
    return [] if values.empty?

    values.map { load_raw(_1) }
  end
end
```

### result

```rbs
class DeferredMapResult
  def self.load_raw: (untyped value) -> untyped
  def self.map_after_guard: (untyped value) -> Array[untyped]
end
```

## Source methods distinguish yielded blocks from unused blocks

### update

```ruby
class ExplicitBlockInvocation
  def run(&block)
    block.call
  end

  def forward(&block)
    run(&block)
  end

  def run_via_alias(&block)
    callback = block
    callback.call
  end

  def alias_and_ignore(&block)
    callback = block
    :ignored
  end

  def overwrite_alias(&block)
    callback = block
    callback = -> { :other }
    callback.call
    :ignored
  end

  def conditionally_overwrite_alias(flag, &block)
    callback = block
    if flag
      callback = -> { :other }
    end
    callback.call
    :ignored
  end

  def invoke_from_case(flag, &block)
    callback = block
    case flag
    when :overwrite
      callback = -> { :other }
    when :invoke
      callback.call
    end
    :ignored
  end

  def invoke_after_loop(flag, &block)
    callback = block
    while flag
      callback = -> { :other }
      break
    end
    callback.call
    :ignored
  end

  def invoke_from_rescue(flag, &block)
    callback = block
    begin
      raise if flag
      callback = -> { :other }
    rescue
      callback.call
    end
    :ignored
  end

  def invoke_after_short_circuit(flag, &block)
    callback = block
    flag || callback = -> { :other }
    callback.call
    :ignored
  end

  def invoke_before_overwrite(&block)
    callback = block
    callback = (callback.call; -> { :other })
    :ignored
  end

  def ignore(&block)
    :ignored
  end

  def block_call_updates_outer_local
    value = :before
    run { value = :after }
    value
  end

  def forwarded_block_updates_outer_local
    value = :before
    forward { value = :after }
    value
  end

  def aliased_block_updates_outer_local
    value = :before
    run_via_alias { value = :after }
    value
  end

  def unused_block_alias_keeps_outer_local
    value = :before
    alias_and_ignore { value = :after }
    value
  end

  def unused_block_keeps_outer_local
    value = :before
    ignore { value = :after }
    value
  end

  def overwritten_block_alias_keeps_outer_local
    value = :before
    overwrite_alias { value = :after }
    value
  end

  def maybe_overwritten_block_alias_updates_outer_local(flag)
    value = :before
    conditionally_overwrite_alias(flag) { value = :after }
    value
  end

  def case_block_alias_updates_outer_local(flag)
    value = :before
    invoke_from_case(flag) { value = :after }
    value
  end

  def loop_block_alias_updates_outer_local(flag)
    value = :before
    invoke_after_loop(flag) { value = :after }
    value
  end

  def rescue_block_alias_updates_outer_local(flag)
    value = :before
    invoke_from_rescue(flag) { value = :after }
    value
  end

  def short_circuit_block_alias_updates_outer_local(flag)
    value = :before
    invoke_after_short_circuit(flag) { value = :after }
    value
  end

  def mutation_in_repeated_block_widens
    items = []
    ExplicitBlockReceiver.new.repeat { items << 1 }
    items
  end

  def guarded_yield(&block)
    yield if block_given?
    :done
  end

  def guarded_yield_updates_outer_local
    value = :before
    guarded_yield { value = :after }
    value
  end

  def early_return_before_yield(flag)
    return :early if flag
    yield
  end

  def early_return_joins_outer_local(flag)
    value = :before
    early_return_before_yield(flag) { value = :after }
    value
  end

  def escaped_block(&block)
    @callback = block
    :stored
  end

  def escaped_block_joins_outer_local
    value = :before
    escaped_block { value = :after }
    value
  end

  def receiver_ignored_block
    value = :before
    ExplicitBlockReceiver.new.ignore { value = :after }
    value
  end

  def receiver_yielded_block
    value = :before
    ExplicitBlockReceiver.new.run { value = :after }
    value
  end

  def block_alias_runs_before_overwrite
    value = :before
    invoke_before_overwrite { value = :after }
    value
  end
end

class ExplicitBlockReceiver
  def ignore(&block) = :ignored
  def run = yield

  def repeat
    yield
    yield
  end
end
```

### result

```rbs
class ExplicitBlockInvocation
  def run: (?untyped &block) -> :after
  def forward: (?untyped &block) -> :after
  def run_via_alias: (?untyped &block) -> :after
  def alias_and_ignore: (?untyped &block) -> :ignored
  def overwrite_alias: (?untyped &block) -> :ignored
  def conditionally_overwrite_alias: (untyped flag, ?untyped &block) -> :ignored
  def invoke_from_case: (untyped flag, ?untyped &block) -> :ignored
  def invoke_after_loop: (untyped flag, ?untyped &block) -> :ignored
  def invoke_from_rescue: (untyped flag, ?untyped &block) -> :ignored
  def invoke_after_short_circuit: (untyped flag, ?untyped &block) -> :ignored
  def invoke_before_overwrite: (?untyped &block) -> :ignored
  def ignore: (?untyped &block) -> :ignored
  def block_call_updates_outer_local: -> :after
  def forwarded_block_updates_outer_local: -> :after
  def aliased_block_updates_outer_local: -> :after
  def unused_block_alias_keeps_outer_local: -> :before
  def unused_block_keeps_outer_local: -> :before
  def overwritten_block_alias_keeps_outer_local: -> :before
  def maybe_overwritten_block_alias_updates_outer_local: (untyped flag) -> (:after | :before)
  def case_block_alias_updates_outer_local: (untyped flag) -> (:after | :before)
  def loop_block_alias_updates_outer_local: (untyped flag) -> (:after | :before)
  def rescue_block_alias_updates_outer_local: (untyped flag) -> (:after | :before)
  def short_circuit_block_alias_updates_outer_local: (untyped flag) -> (:after | :before)
  def mutation_in_repeated_block_widens: -> Array[1]
  def guarded_yield: (?untyped &block) -> :done
  def guarded_yield_updates_outer_local: -> :after
  def early_return_before_yield: (untyped flag) { () -> :after } -> (:after | :early)
  def early_return_joins_outer_local: (untyped flag) -> (:after | :before)
  def escaped_block: (?untyped &block) -> :stored
  def escaped_block_joins_outer_local: -> :after | :before
  def receiver_ignored_block: -> :before
  def receiver_yielded_block: -> :after
  def block_alias_runs_before_overwrite: -> :after
end

class ExplicitBlockReceiver
  def ignore: (?untyped &block) -> :ignored
  def run: { () -> :after } -> :after
  def repeat: { () -> [1] } -> [1]
end
```

## Literal arguments select the block path through a source method

### update

```ruby
class LiteralArgumentBlockPath
  def conditionally_overwrite_alias(flag, &block)
    callback = block
    if flag
      callback = -> { :other }
    end
    callback.call
    :ignored
  end

  def invoke_from_case(flag, &block)
    callback = block
    case flag
    when :overwrite
      callback = -> { :other }
    when :invoke
      callback.call
    end
    :ignored
  end

  def invoke_after_loop(flag, &block)
    callback = block
    while flag
      callback = -> { :other }
      break
    end
    callback.call
    :ignored
  end

  def invoke_from_rescue(flag, &block)
    callback = block
    begin
      raise if flag
      callback = -> { :other }
    rescue
      callback.call
    end
    :ignored
  end

  def invoke_after_short_circuit(flag, &block)
    callback = block
    flag || callback = -> { :other }
    callback.call
    :ignored
  end

  def maybe_overwritten
    value = :before
    conditionally_overwrite_alias(:unknown) { value = :after }
    value
  end

  def from_case
    value = :before
    invoke_from_case(:invoke) { value = :after }
    value
  end

  def after_loop
    value = :before
    invoke_after_loop(:unknown) { value = :after }
    value
  end

  def from_rescue
    value = :before
    invoke_from_rescue(:unknown) { value = :after }
    value
  end

  def after_short_circuit
    value = :before
    invoke_after_short_circuit(:unknown) { value = :after }
    value
  end
end
```

### result

```rbs
class LiteralArgumentBlockPath
  def conditionally_overwrite_alias: (Symbol flag, ?untyped &block) -> :ignored
  def invoke_from_case: (Symbol flag, ?untyped &block) -> :ignored
  def invoke_after_loop: (Symbol flag, ?untyped &block) -> :ignored
  def invoke_from_rescue: (Symbol flag, ?untyped &block) -> :ignored
  def invoke_after_short_circuit: (Symbol flag, ?untyped &block) -> :ignored
  def maybe_overwritten: -> :before
  def from_case: -> :after
  def after_loop: -> :before
  def from_rescue: -> :after
  def after_short_circuit: -> :after
end
```

## Literal arguments decide only what the callee reads unchanged

### update

```ruby
class LiteralBlockEdges
  def yield_if(flag)
    yield if flag
    :done
  end

  def yield_unless_not(flag)
    yield unless !flag
    :done
  end

  def overwritten_flag(flag)
    flag = !flag
    yield if flag
    :done
  end

  def shadowed_in_closure(flag)
    [1].each { |flag| yield if flag }
    :done
  end

  def case_undecided(flag)
    case flag
    when Integer then yield
    when :other then :no
    end
    :done
  end

  def case_multi(flag)
    case flag
    when :a, :b then yield
    else :no
    end
    :done
  end

  def case_float(flag)
    case flag
    when 1.0 then yield
    end
    :done
  end

  def do_while(flag)
    begin
      yield
    end while flag
    :done
  end

  def until_loop(flag)
    until flag
      yield
      break
    end
    :done
  end

  def optional(flag = nil)
    yield if flag
    :done
  end

  def trailing(first = 1, flag)
    yield if flag
    :done
  end

  def retried(flag, &block)
    callback = block
    begin
      raise if flag
    rescue
      callback.call
      callback = -> { :other }
      retry if false
    end
    :done
  end

  def forwarded(flag, &block)
    return :none unless flag
    target(&block)
  end

  def target = yield

  def nil_skips = (v = :before; yield_if(nil) { v = :after }; v)
  def symbol_runs = (v = :before; yield_if(:x) { v = :after }; v)
  def negated_runs = (v = :before; yield_unless_not(:x) { v = :after }; v)
  def overwritten_is_unknown = (v = :before; overwritten_flag(true) { v = :after }; v)
  def shadowed_is_unknown = (v = :before; shadowed_in_closure(nil) { v = :after }; v)
  def undecided_pattern_is_unknown = (v = :before; case_undecided(:other) { v = :after }; v)
  def multi_pattern_runs = (v = :before; case_multi(:b) { v = :after }; v)
  def no_pattern_skips = (v = :before; case_multi(:c) { v = :after }; v)
  def float_pattern_is_unknown = (v = :before; case_float(1) { v = :after }; v)
  def do_while_is_unknown = (v = :before; do_while(false) { v = :after }; v)
  def until_enters = (v = :before; until_loop(nil) { v = :after }; v)
  def until_skips = (v = :before; until_loop(true) { v = :after }; v)
  def optional_given_runs = (v = :before; optional(true) { v = :after }; v)
  def optional_omitted_is_unknown = (v = :before; optional { v = :after }; v)
  def trailing_is_unknown = (v = :before; trailing(nil, 1) { v = :after }; v)
  def retry_is_unknown = (v = :before; retried(true) { v = :after }; v)
  def forward_runs = (v = :before; forwarded(1) { v = :after }; v)
  def forward_skips = (v = :before; forwarded(nil) { v = :after }; v)
  def splat_is_unknown = (v = :before; yield_if(*[nil]) { v = :after }; v)
  def non_literal_is_unknown(flag) = (v = :before; yield_if(flag) { v = :after }; v)
end
```

### result

```rbs
class LiteralBlockEdges
  def yield_if: (Symbol? flag) { () -> :after } -> :done
  def yield_unless_not: (Symbol flag) { () -> :after } -> :done
  def overwritten_flag: (bool flag) { () -> :after } -> :done
  def shadowed_in_closure: (nil flag) { () -> :after } -> :done
  def case_undecided: (Symbol flag) { () -> :after } -> :done
  def case_multi: (Symbol flag) { () -> :after } -> :done
  def case_float: (Integer flag) { () -> :after } -> :done
  def do_while: (bool flag) { () -> :after } -> :done
  def until_loop: (bool? flag) { () -> :after } -> :done
  def optional: (?bool? flag) { () -> :after } -> :done
  def trailing: (?Integer? first, Integer flag) { () -> :after } -> :done
  def retried: (bool flag, ?untyped &block) -> :done
  def forwarded: (Integer? flag) { () -> untyped } -> untyped
  def target: { () -> untyped } -> untyped
  def nil_skips: -> :before
  def symbol_runs: -> :after
  def negated_runs: -> :after
  def overwritten_is_unknown: -> :after | :before
  def shadowed_is_unknown: -> :after | :before
  def undecided_pattern_is_unknown: -> :after | :before
  def multi_pattern_runs: -> :after
  def no_pattern_skips: -> :before
  def float_pattern_is_unknown: -> :after | :before
  def do_while_is_unknown: -> :after | :before
  def until_enters: -> :after
  def until_skips: -> :before
  def optional_given_runs: -> :after
  def optional_omitted_is_unknown: -> :after | :before
  def trailing_is_unknown: -> :after | :before
  def retry_is_unknown: -> :after | :before
  def forward_runs: -> :after
  def forward_skips: -> :before
  def splat_is_unknown: -> :after | :before
  def non_literal_is_unknown: (untyped flag) -> (:after | :before)
end
```

## A redefined literal case equality keeps every when arm

### update

```ruby
class Symbol
  def ===(_other) = true
end

class RedefinedLiteralCase
  def run(flag)
    case flag
    when :a then yield
    end
    :done
  end

  def still_unknown
    value = :before
    run(:b) { value = :after }
    value
  end
end
```

### result

```rbs
class RedefinedLiteralCase
  def run: (Symbol flag) { () -> :after } -> :done
  def still_unknown: -> :after | :before
end

class Symbol
  def ===: (untyped _other) -> true
end
```

## Record direct block calls without changing unused block returns

### update

```ruby
class DirectBlockCall
  def invoke(&block)
    block.call
  end

  def ignore(&block)
    :ignored
  end
end
```

### result

```rbs
class DirectBlockCall
  def invoke: (?untyped &block) -> untyped
  def ignore: (?untyped &block) -> :ignored
end
```

## Hash iteration keeps the receiver result across block control flow

### update

```ruby
class HashEachResult
  #: () -> untyped
  def self.config
  end

  def self.defaults = { enabled: config.enabled }

  def each_with_next
    @init_attributes = {}
    self.class.defaults.each do |attribute, value|
      next unless @init_attributes[:"#{attribute}_enabled"].nil? &&
        @init_attributes[:"#{attribute}_access_level"].nil?

      public_send("#{attribute}_enabled=", value)
    end
  end
end
```

### result

```rbs
class HashEachResult
  def self.config: -> untyped
  def self.defaults: -> { enabled: untyped }
  def each_with_next: -> { enabled: untyped }
end
```

## Map distributes literal operations across mixed tuple elements

### update

```ruby
def mixed_literal_map = [1, 2, "3"].map { |n| n * 2 }
```

### result

```rbs
class Object < BasicObject
  def mixed_literal_map: -> Array[Integer | String]
end
```

## Chained array transformations keep the result of symbol-proc sort_by

### update

```ruby
module Redmine
  module FieldFormat
    def self.all
      @formats ||= Hash.new(Base.instance)
    end

    def self.as_select(class_name = nil)
      formats = all.values.select do |format|
        format.class.customized_class_names.nil? ||
          format.class.customized_class_names.include?(class_name)
      end
      formats.map do |format|
        [::I18n.t(format.label), format.name]
      end.sort_by(&:first)
    end
  end
end
```

### result

```rbs
module Redmine::FieldFormat
  def self.all: -> Hash[untyped, untyped]
  def self.as_select: (?nil class_name) -> Array[[String, untyped]]
end
```

## select preserving element type

### update

```ruby
def filter_ints = [1, 2, 3].select { |x| x > 1 }
```

### result

```rbs
class Object < BasicObject
  def filter_ints: -> Array[1 | 2 | 3]
end
```

## Use map and select block signatures for side effects

### update

```ruby
def map_side_effect_values
  values = []
  [1, 2].map { |value| values << value }
  values
end

def numbered_map_side_effect_values
  values = []
  [1, 2].map { values << _1 }
  values
end

def select_side_effect_values
  values = []
  [1, 2].select { |value| values << value; true }
  values
end
```

### result

```rbs
class Object < BasicObject
  def map_side_effect_values: -> Array[1 | 2]
  def numbered_map_side_effect_values: -> Array[1 | 2]
  def select_side_effect_values: -> Array[1 | 2]
end
```

## Use Range and Set block signatures for side effects

### update

```ruby
def range_each_values
  values = []
  (1..3).each { |value| values << value }
  values
end

def range_reverse_values
  values = []
  (1..3).reverse_each { |value| values << value }
  values
end

def string_range_values
  values = []
  ("a".."c").each { |value| values << value }
  values
end

def set_each_values
  values = []
  Set[1, 2].each { |value| values << value }
  values
end

def set_with_object_map
  Set[:a, :b].each_with_object({}) { |key, map| map[key] = key.to_s }
end
```

### result

```rbs
class Object < BasicObject
  def range_each_values: -> Array[Integer]
  def range_reverse_values: -> Array[Integer]
  def string_range_values: -> Array[String]
  def set_each_values: -> Array[1 | 2]
  def set_with_object_map: -> Hash[:a | :b, String]
end
```

## Use Hash iterator block signatures for side effects

### update

```ruby

def hash_key_side_effect_values
  keys = []
  { name: "one", count: 2 }.each_key { |key| keys << key }
  keys
end

def hash_value_side_effect_values
  values = []
  { name: "one", count: 2 }.each_value { |value| values << value }
  values
end

def fetch_key_side_effect_values
  keys = []
  { name: "one" }.fetch(:count) { |key| keys << key }
  keys
end

def fetch_existing_key_side_effect_values
  keys = []
  { name: "one" }.fetch(:name) { |key| keys << key }
  keys
end

def fetch_values_key_side_effect_values
  keys = []
  { name: "one" }.fetch_values(:count) { |key| keys << key }
  keys
end

def fetch_values_existing_key_side_effect_values
  keys = []
  { name: "one" }.fetch_values(:name) { |key| keys << key }
  keys
end
```

### result

```rbs
class Object < BasicObject
  def hash_key_side_effect_values: -> Array[:count | :name]
  def hash_value_side_effect_values: -> Array[2 | "one"]
  def fetch_key_side_effect_values: -> Array[:count]
  def fetch_existing_key_side_effect_values: -> [ ]
  def fetch_values_key_side_effect_values: -> Array[:count]
  def fetch_values_existing_key_side_effect_values: -> [ ]
end
```

## each returning self

### update

```ruby
def each_returns_self = [1, 2, 3].each { |x| puts x }
```

### result

```rbs
class Object < BasicObject
  def each_returns_self: -> [1, 2, 3]
end
```

## find returning element | nil

### update

```ruby
def find_int = [1, 2, 3].find { |x| x > 1 }
```

### result

```rbs
class Object < BasicObject
  def find_int: -> (1 | 2 | 3)?
end
```

## find with ifnone proc

### update

```ruby
class F
  def find_ifnone
    [1, 2].find(-> { "x" }) { |n| n > 10 }
  end
end
```

### result

```rbs
class F
  def find_ifnone: -> 1 | 2 | "x"
end
```

## find and detect narrow with class predicate

### update

```ruby
class Entry
end

class Note < Entry
end

class Flag < Entry
end

class Other
end

def find_entry
  [Note.new, Other.new, Flag.new].find { |value| value.is_a?(Entry) }
end

def detect_non_entry
  [Note.new, Other.new, Flag.new].detect { |value| !value.kind_of?(Entry) }
end

def find_entry_or_default
  [Note.new, Other.new].find(-> { :none }) { |value| value.is_a?(Entry) }
end
```

### result

```rbs
class Object < BasicObject
  def find_entry: -> (Flag | Note)?
  def detect_non_entry: -> Other?
  def find_entry_or_default: -> :none | Note
end
```

## find narrows with module predicate

### update

```ruby
module Taggable
end

class Article
  include Taggable
end

class Event
  include Taggable
end

class Plain
end

def find_taggable_value
  [Article.new, Plain.new, Event.new].find { |value| value.kind_of?(Taggable) }
end
```

### result

```rbs
class Article
  include Taggable
end

class Event
  include Taggable
end

class Object < BasicObject
  def find_taggable_value: -> (Article | Event)?
end
```

## detect narrows with constant alias predicate

```yaml
known_issue: true
```

### update

```ruby
module Group
  class Base
  end

  class Item < Base
  end

  ItemAlias = Item
end

def detect_project_item
  [Group::Item.new, Object.new].detect { |value| value.is_a?(Group::ItemAlias) }
end
```

### result

```rbs
module Group
  class ItemAlias = Group::Item
end

class Object < BasicObject
  def detect_project_item: -> Group::Item?
end
```

## find and detect narrow with nil and truthy predicate

### update

```ruby
def find_non_nil_value
  [nil, "a", :b].find { |value| !value.nil? }
end

def detect_nil_value
  [1, nil, 2].detect(&:nil?)
end

def find_truthy_value
  [false, nil, "x"].find(&:itself)
end
```

### result

```rbs
class Object < BasicObject
  def find_non_nil_value: -> ("a" | :b)?
  def detect_nil_value: -> nil
  def find_truthy_value: -> "x"?
end
```

## Resolve block return for bsearch and find_index

### update

```ruby
def range_bsearch
  (0...10).bsearch { |index| index >= 3 }
end

def array_bsearch
  [1, 2, 3].bsearch { |value| value >= 2 }
end

def array_bsearch_index
  [1, 2, 3].bsearch_index { |value| value >= 2 }
end

def line_find_index
  ["one", "two", "three"].find_index { |line| line.length > 3 }
end

def entry_find_index
  { one: 1, two: 2 }.find_index { |key, value| key == :two && value > 1 }
end
```

### result

```rbs
class Object < BasicObject
  def range_bsearch: -> Integer?
  def array_bsearch: -> (1 | 2 | 3)?
  def array_bsearch_index: -> Integer?
  def line_find_index: -> Integer?
  def entry_find_index: -> Integer?
end
```

## Resolve window type for slice and chunk iterators

### update

```ruby

def slice_before_groups
  ["a", "bb", "ccc"].slice_before { |line| line.length > 1 }.to_a
end

def slice_after_heads
  ["a", "bb", "ccc"].slice_after { |line| line.length > 1 }.map(&:first)
end

def slice_when_groups
  [1, 2, 4, 5].slice_when { |left, right| right - left > 1 }.to_a
end

def chunk_while_groups
  [1, 2, 4, 5].chunk_while { |left, right| right - left == 1 }.to_a
end
```

### result

```rbs
class Object < BasicObject
  def slice_before_groups: -> Array[Array["a" | "bb" | "ccc"]]
  def slice_after_heads: -> Array[("a" | "bb" | "ccc")?]
  def slice_when_groups: -> Array[Array[1 | 2 | 4 | 5]]
  def chunk_while_groups: -> Array[Array[1 | 2 | 4 | 5]]
end
```

## reduce with initial value

### update

```ruby
def sum_reduce = [1, 2, 3].reduce(0) { |sum, x| sum + x }
```

### result

```rbs
class Object < BasicObject
  def sum_reduce: -> Integer
end
```

## Recover Array accumulator in inject from block return

### update

```ruby
def collect_values
  [1, 2, 3].inject([]) do |list, value|
    list << value.to_s
    list
  end
end
```

### result

```rbs
class Object < BasicObject
  def collect_values: -> Array[String]
end
```

## concat into Array accumulator in reduce

### update

```ruby
def collect_nested_values
  [[1, 2], [3]].reduce([]) do |list, values|
    list.concat(values)
    list
  end
end
```

### result

```rbs
class Object < BasicObject
  def collect_nested_values: -> Array[1 | 2 | 3]
end
```

## Write dynamic key to Hash accumulator in inject

### update

```ruby
def build_table
  ["a", "bb"].inject({}) do |table, name|
    table[name] = name.length
    table
  end
end
```

### result

```rbs
class Object < BasicObject
  def build_table: -> Hash["a" | "bb", 1 | 2]
end
```

## Keep nested block control flow out of a Hash accumulator

### update

```ruby
def map_hashes_after_next
  [1, 2].map do |value|
    next if value.zero?
    [value].inject({}) do |hash, item|
      hash[item.to_s] = item.to_s
      hash
    end.merge("value" => value.to_s)
  end
end
```

### result

```rbs
class Object < BasicObject
  def map_hashes_after_next: -> Array[Hash[String, String]?]
end
```

## Update existing accumulator alias in inject

### update

```ruby
def collect_into_result
  result = []
  [1, 2, 3].inject(result) do |list, value|
    list << value.to_s
    list
  end
  result
end
```

### result

```rbs
class Object < BasicObject
  def collect_into_result: -> Array[String]
end
```

## any?/all?/none?

### update

```ruby
def check_any = [1, 2, 3].any? { |x| x > 2 }

def check_all = [1, 2, 3].all? { |x| x > 0 }

def check_none = [1, 2, 3].none? { |x| x > 5 }
```

### result

```rbs
class Object < BasicObject
  def check_any: -> bool
  def check_all: -> bool
  def check_none: -> bool
end
```

## Method chaining: select then map

### update

```ruby
def chain_select_map = [1, 2, 3].select { |x| x > 1 }.map { |x| x.to_s }
```

### result

```rbs
class Object < BasicObject
  def chain_select_map: -> Array[String]
end
```

## tap preserves receiver type

### update

```ruby
def use_tap = "hello".tap { |s| puts s }
```

### result

```rbs
class Object < BasicObject
  def use_tap: -> "hello"
end
```

## tap propagates mutable receiver side effects

### update

```ruby
def build_hash_with_tap
  {}.tap do |hash|
    hash[:name] = "item"
    hash[:count] = 1
  end
end

def extend_record_with_tap
  { name: "item" }.tap do |hash|
    hash[:count] = 1
  end
end

def build_array_with_tap
  [].tap do |values|
    values << "item"
    values << 1
  end
end

def update_local_with_tap
  values = []
  values.tap { |list| list << :item }
  values
end
```

### result

```rbs
class Object < BasicObject
  def build_hash_with_tap: -> Hash[:count | :name, 1 | "item"]
  def extend_record_with_tap: -> { name: "item", count: 1 }
  def build_array_with_tap: -> Array[1 | "item"]
  def update_local_with_tap: -> Array[:item]
end
```

## then/yield_self transforms type

### update

```ruby
def use_then = "hello".then { |s| s.length }
```

### result

```rbs
class Object < BasicObject
  def use_then: -> 5
end
```

## count with block

### update

```ruby
def count_positives = [1, -2, 3].count { |x| x > 0 }
```

### result

```rbs
class Object < BasicObject
  def count_positives: -> Integer
end
```

## group_by

### update

```ruby
def group_by_parity = [1, 2, 3, 4].group_by { |x| x > 2 }
```

### result

```rbs
class Object < BasicObject
  def group_by_parity: -> Hash[bool, Array[1 | 2 | 3 | 4]]
end
```

## sort_by

### update

```ruby
def sort_desc = [3, 1, 2].sort_by { |x| x }
```

### result

```rbs
class Object < BasicObject
  def sort_desc: -> Array[1 | 2 | 3]
end
```

## sum without block

### update

```ruby
def sum_ints = [1, 2, 3].sum
```

### result

```rbs
class Object < BasicObject
  def sum_ints: -> Integer
end
```

## sum of float array is Float

### update

```ruby
def float_sum = [1.5, 2.5].sum
def mixed_sum = [1, 2.5].sum
def empty_sum = [].sum
```

### result

```rbs
class Object < BasicObject
  def float_sum: -> Float
  def mixed_sum: -> Float
  def empty_sum: -> Integer
end
```

## flat_map

### update

```ruby
def flat_map_example = [[1, 2], [3, 4]].flat_map { |a| a }
```

### result

```rbs
class Object < BasicObject
  def flat_map_example: -> Array[1 | 2 | 3 | 4]
end
```

## tuple union block destructuring

### update

```ruby
def map_entry_pairs
  [[1, "a"], [2, "b"]].map { |id, name| [name, id] }
end

def flat_map_entry_pairs
  [[1, "a"], [2, "b"]].flat_map { |id, name| [id, name] }
end

def missing_entry_tail
  [[1], [2, 3]].map { |id, value| [id, value] }
end
```

### result

```rbs
class Object < BasicObject
  def map_entry_pairs: -> Array[["a" | "b", 1 | 2]]
  def flat_map_entry_pairs: -> Array[1 | 2 | "a" | "b"]
  def missing_entry_tail: -> Array[[1 | 2, 3?]]
end
```

## min_by/max_by

### update

```ruby
def find_min_by = [3, 1, 2].min_by { |x| x }
```

### result

```rbs
class Object < BasicObject
  def find_min_by: -> (1 | 2 | 3)?
end
```

## reject

### update

```ruby
def reject_small = [1, 2, 3].reject { |x| x < 2 }
```

### result

```rbs
class Object < BasicObject
  def reject_small: -> [1, 2, 3]
end
```

## each_entry

### update

```ruby
def each_entry_rows
  rows = []
  { name: "one", count: 2 }.each_entry { |row| rows << row }
  rows
end

def each_entry_pairs
  { name: "one", count: 2 }.each_entry.map { |key, value| [key, value] }
end

def array_each_entry
  ["a", "bb"].each_entry.map(&:length)
end
```

### result

```rbs
class Object < BasicObject
  def each_entry_rows: -> Array[[:count | :name, 2 | "one"]]
  def each_entry_pairs: -> Array[[:count | :name, 2 | "one"]]
  def array_each_entry: -> Array[1 | 2]
end
```

## map over a literal tuple widens to Array of String

### update

```ruby
def stringified = [1, 2, 3].map { |n| n.to_s }
```

### result

```rbs
class Object < BasicObject
  def stringified: -> Array[String]
end
```

## map preserves a literal tuple's element union

### update

```ruby
def literal_map = [1, 2, 3].map { |n| n }
```

### result

```rbs
class Object < BasicObject
  def literal_map: -> Array[1 | 2 | 3]
end
```

## numbered-parameter map widens integer increments

### update

```ruby
def shifted = [1, 2, 3].map { _1 + 1 }
```

### result

```rbs
class Object < BasicObject
  def shifted: -> Array[Integer]
end
```

## filter_map with even? keeps stringified survivors

### update

```ruby
def evens = [1, 2, 3].filter_map { |n| n.even? ? n.to_s : nil }
```

### result

```rbs
class Object < BasicObject
  def evens: -> Array[String]
end
```

## flat_map with branch-dependent sizes unions the elements

### update

```ruby
def flat_varied = [1, 2].flat_map { |n| n.even? ? [n, n] : [n] }
```

### result

```rbs
class Object < BasicObject
  def flat_varied: -> Array[1 | 2]
end
```

## flat_map of a scalar block widens to Array of String

### update

```ruby
def flat_strings = [1, 2, 3].flat_map { |n| n.to_s }
```

### result

```rbs
class Object < BasicObject
  def flat_strings: -> Array[String]
end
```

## Block accumulator from each push

### update

```ruby
def foo(arr)
  result = []
  arr.each { |x| result << x * 2 }
  result
end

foo([1, 2, 3])
```

### result

```rbs
class Object < BasicObject
  def foo: (Array[Integer] arr) -> Array[Integer]
end
```

## find on a literal tuple returns a nilable element union

### update

```ruby
def first_even = [1, 2, 3, 4].find { |n| n.even? }
```

### result

```rbs
class Object < BasicObject
  def first_even: -> (1 | 2 | 3 | 4)?
end
```

## find_index on a literal tuple returns a nilable Integer

### update

```ruby
def idx_first_even = [1, 2, 3, 4].find_index { |n| n.even? }
```

### result

```rbs
class Object < BasicObject
  def idx_first_even: -> Integer?
end
```

## Same block passed twice to each

### update

```ruby
def foo(&blk)
  obj = [1]
  obj.each(&blk)
  obj.each(&blk)
end

foo { |x| x }
```

### result

```rbs
class Object < BasicObject
  def foo: (?untyped &blk) -> [1]
end
```

## Block next excludes a dead until-true

### update

```ruby
def bar
  yield
  nil
end

def foo
  bar do
    next :a
    until true
      next :b
    end
    next :c
  end
end
```

### result

```rbs
class Object < BasicObject
  def bar: { () -> :a } -> nil
  def foo: -> nil
end
```

## Unknown reverse_each receivers keep block arguments untyped

### update

```ruby
def inspect_reverse_each_value(value)
  (value ? ["address"] : []).reverse_each do |address|
    observed_address(address)
  end
end

def observed_address(address)
  address
end

def inspect_split_header(value)
  split_header(value).reverse_each do |address|
    observed_split_header(address)
  end
  :done
end

def split_header(value)
  value ? value.strip.split(/[, \t]+/) : []
end

def observed_split_header(address)
  address
end

```

### result

```rbs
class Object < BasicObject
  def inspect_reverse_each_value: (untyped value) -> Array["address"]
  def observed_address: (String address) -> String
  def inspect_split_header: (untyped value) -> :done
  def split_header: (untyped value) -> untyped
  def observed_split_header: (untyped address) -> untyped
end
```

## Nested block calls bind the declared parameter at each level

```ruby
class NestedBlockSink
  def take(value)
    value
  end
end

class NestedBlockCaller
  def run
    lines = File.readlines("input")
    lines.select { |outer| lines.select { |inner| NestedBlockSink.new.take(outer) } }
  end
end
```

### result

```rbs
class NestedBlockCaller
  def run: -> Array[String]
end

class NestedBlockSink
  def take: (String value) -> String
end
```

## A yield in a loop reads the value assigned earlier in the iteration

```ruby
class LoopYieldReader
  def each_part(path)
    file = File.open(path)
    while true
      part = file.read(1)
      break unless part
      yield part
    end
  end
end
```

### result

```rbs
class LoopYieldReader
  def each_part: (untyped path) { (String) -> untyped } -> nil
end
```
