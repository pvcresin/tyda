# Ruby / Control / If Else

## if else returns different types

### update

```ruby
def foo(x)
  if x
    1
  else
    "hello"
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> (1 | "hello")
end
```

## if else returns same type

### update

```ruby
def foo(x)
  if x
    1
  else
    2
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> (1 | 2)
end
```

## if without else unions with nil

### update

```ruby
def foo(x)
  if x
    1
  end
end
```

### result

```rbs
class Object < BasicObject
  def foo: (untyped x) -> 1?
end
```

## local assigned on one branch is nilable after the branch

### update

```ruby
def conditional_local(flag)
  if flag
    value = :present
  end
  value
end
```

### result

```rbs
class Object < BasicObject
  def conditional_local: (untyped flag) -> :present?
end
```

## local assigned on both branches joins branch types

### update

```ruby
def complete_branch_local(flag)
  if flag
    value = 1
  else
    value = "other"
  end
  value
end
```

### result

```rbs
class Object < BasicObject
  def complete_branch_local: (untyped flag) -> (1 | "other")
end
```

## branch merges retain mutable collection updates

```ruby
def hash_mutation_on_one_branch(flag)
  data = { base: 1 }
  if flag
    data[:extra] = "added"
  end
  data
end

def append_to_record_array(value)
  data = { values: [] }
  data[:values] << value
  data
end

def append_to_record_array_in_block(values)
  data = { values: [] }
  values.each do |value|
    data[:values] << value
  end
  data
end

def append_to_record_array_with_dynamic_key(flag, value)
  data = { left: [], right: [] }
  key = flag ? :left : :right
  data[key] << value
  data
end

def append_to_dynamic_record_array_with_distinct_elements(flag)
  data = { left: ["left"], right: [1], untouched: [:keep] }
  key = flag ? :left : :right
  data[key] << :added
  data
end

def write_to_finite_dynamic_record_key(flag)
  data = { left: :left, right: :right, untouched: :keep }
  key = flag ? :left : :right
  data[key] = :updated
  data
end

def write_with_16_dynamic_record_keys(index)
  data = {
    k0: :old, k1: :old, k2: :old, k3: :old,
    k4: :old, k5: :old, k6: :old, k7: :old,
    k8: :old, k9: :old, k10: :old, k11: :old,
    k12: :old, k13: :old, k14: :old, k15: :old
  }
  key = case index
  when 0 then :k0
  when 1 then :k1
  when 2 then :k2
  when 3 then :k3
  when 4 then :k4
  when 5 then :k5
  when 6 then :k6
  when 7 then :k7
  when 8 then :k8
  when 9 then :k9
  when 10 then :k10
  when 11 then :k11
  when 12 then :k12
  when 13 then :k13
  when 14 then :k14
  when 15 then :k15
  else :k0
  end
  data[key] = :updated
  data
end

def write_with_17_dynamic_record_keys(index)
  data = {
    k0: :old, k1: :old, k2: :old, k3: :old,
    k4: :old, k5: :old, k6: :old, k7: :old,
    k8: :old, k9: :old, k10: :old, k11: :old,
    k12: :old, k13: :old, k14: :old, k15: :old, k16: :old
  }
  key = case index
  when 0 then :k0
  when 1 then :k1
  when 2 then :k2
  when 3 then :k3
  when 4 then :k4
  when 5 then :k5
  when 6 then :k6
  when 7 then :k7
  when 8 then :k8
  when 9 then :k9
  when 10 then :k10
  when 11 then :k11
  when 12 then :k12
  when 13 then :k13
  when 14 then :k14
  when 15 then :k15
  when 16 then :k16
  else :k0
  end
  data[key] = :updated
  data
end

def write_with_unknown_dynamic_record_key(flag)
  data = { left: :left, right: :right }
  key = flag ? :left : :missing
  data[key] = :updated
  data
end

def write_with_optional_dynamic_record_key(flag)
  data = { left: :left }
  data[:right] = :right if flag
  key = flag ? :left : :right
  data[key] = :updated
  data
end

NAMESPACE_TYPE_MAPPING = { "Group" => :group_ids, "Project" => :project_namespace_ids }

class NamespaceRow
  def initialize(type)
    @type = type
  end

  def type
    @type
  end

  def id
    Object.new
  end
end

def mapping_lookup_with_union_key(flag)
  key = flag ? "Group" : "Project"
  NAMESPACE_TYPE_MAPPING[key]
end

def collect_namespace_ids_from_batch(iterator)
  buckets = { group_ids: [], project_namespace_ids: [] }
  iterator.each_batch do |ids|
    namespaces = [NamespaceRow.new("Group"), NamespaceRow.new("Project")]
    namespaces.each do |namespace|
      key = NAMESPACE_TYPE_MAPPING[namespace.type]
      buckets[key] << namespace.id if key
    end
  end
  buckets
end

def mapping_lookup_for_namespace_row(flag)
  key = flag ? "Group" : "Project"
  NAMESPACE_TYPE_MAPPING[NamespaceRow.new(key).type]
end
```

### result

```rbs
NAMESPACE_TYPE_MAPPING: { "Group" => :group_ids, "Project" => :project_namespace_ids }

class NamespaceRow
  def initialize: (String type) -> void
  def type: -> String
  def id: -> Object
end

class Object < BasicObject
  def hash_mutation_on_one_branch: (untyped flag) -> { base: 1, ?extra: "added" }
  def append_to_record_array: (untyped value) -> { values: [untyped] }
  def append_to_record_array_in_block: (untyped values) -> { values: Array[untyped] }
  def append_to_record_array_with_dynamic_key: (untyped flag, untyped value) -> { left: Array[untyped], right: Array[untyped] }
  def append_to_dynamic_record_array_with_distinct_elements: (untyped flag) -> { left: Array["left" | :added], right: Array[1 | :added], untouched: [:keep] }
  def write_to_finite_dynamic_record_key: (untyped flag) -> { left: :left | :updated, right: :right | :updated, untouched: :keep }
  def write_with_16_dynamic_record_keys: (untyped index) -> { k0: :old | :updated, k1: :old | :updated, k2: :old | :updated, k3: :old | :updated, k4: :old | :updated, k5: :old | :updated, k6: :old | :updated, k7: :old | :updated, k8: :old | :updated, k9: :old | :updated, k10: :old | :updated, k11: :old | :updated, k12: :old | :updated, k13: :old | :updated, k14: :old | :updated, k15: :old | :updated }
  def write_with_17_dynamic_record_keys: (untyped index) -> Hash[:k0 | :k1 | :k10 | :k11 | :k12 | :k13 | :k14 | :k15 | :k16 | :k2 | :k3 | :k4 | :k5 | :k6 | :k7 | :k8 | :k9, :old | :updated]
  def write_with_unknown_dynamic_record_key: (untyped flag) -> Hash[:left | :missing | :right, :left | :right | :updated]
  def write_with_optional_dynamic_record_key: (untyped flag) -> Hash[:left | :right, :left | :right | :updated]
  def mapping_lookup_with_union_key: (untyped flag) -> (:group_ids | :project_namespace_ids)
  def collect_namespace_ids_from_batch: (untyped iterator) -> { group_ids: Array[Object], project_namespace_ids: Array[Object] }
  def mapping_lookup_for_namespace_row: (untyped flag) -> (:group_ids | :project_namespace_ids)?
end
```

## Array include? narrows an optional value to direct literal elements

### update

```ruby
def included_prefix(input)
  prefix = input.to_s if input
  if %w[has is].include?(prefix)
    prefix
  end
end
```

### result

```rbs
class Object < BasicObject
  def included_prefix: (untyped input) -> ("has" | "is")?
end
```

## Array include? keeps values after an aliased array changes

### update

```ruby
def include_after_alias_mutation(flag)
  values = ["a"]
  alias_values = values
  alias_values << "b"
  candidate = flag ? "a" : "b"
  values.include?(candidate) ? candidate : nil
end
```

### result

```rbs
class Object < BasicObject
  def include_after_alias_mutation: (untyped flag) -> ("a" | "b")?
end
```

## Array include? respects an overridden element equality method

### update

```ruby
class String
  def ==(_other) = true
end

def include_with_overridden_string_equality(value)
  ["a"].include?(value) ? value : :missing
end
```

### result

```rbs
class Object < BasicObject
  def include_with_overridden_string_equality: (untyped value) -> (untyped | :missing)
end

class String
  def ==: (untyped _other) -> true
end
```

## Array include? respects refined element equality

### update

```ruby
module AlwaysMatchesString
  refine String do
    def ==(_other) = true
  end
end

using AlwaysMatchesString

def include_with_refined_string_equality(value)
  ["a"].include?(value) ? value : :missing
end
```

### result

```rbs
class Object < BasicObject
  def include_with_refined_string_equality: (untyped value) -> (untyped | :missing)
end
```

## Array include? preserves numerically equal float values

### update

```ruby
def include_integer_or_equal_float(flag)
  value = flag ? 1 : 1.0
  [1].include?(value) ? value : nil
end
```

### result

```rbs
class Object < BasicObject
  def include_integer_or_equal_float: (untyped flag) -> (1 | 1.0)?
end
```

## conditional instance data keeps fields from a truthy argument

### update

```ruby
class ConditionalLogData
  def initialize(query: nil, output: nil)
    @query = query
    @output = output
  end

  def data
    details = { base: :common }
    if query
      details[:query] = query
      details[:output] = output
    end
    details
  end

  private

  attr_reader :query, :output
end

ConditionalLogData.new(query: "query", output: "graphql").data

class DefaultNilLogData
  def initialize(query = nil)
    @query = query
  end

  def data
    details = { base: :common }
    details[:query] = query if query
    details
  end

  private

  attr_reader :query
end
```

### result

```rbs
class ConditionalLogData
  def initialize: (?query: String?, ?output: String?) -> void
  def data: -> { base: :common, ?query: String?, ?output: String? }
  private def query: -> String?
  private def output: -> String?
end

class DefaultNilLogData
  def initialize: (?nil query) -> void
  def data: -> { base: :common, ?query: nil }
  private def query: -> nil
end
```

## Proc values are always truthy

```ruby
def proc_truthiness
  callback = proc { :value }
  if callback
    :truthy
  else
    :falsy
  end
end

def captured_optional_block_does_not_make_proc_optional(&block)
  callback = proc { block.call }
  if callback
    :truthy
  else
    :falsy
  end
end
```

### result

```rbs
class Object < BasicObject
  def proc_truthiness: -> :truthy
  def captured_optional_block_does_not_make_proc_optional: (?untyped &block) -> :truthy
end
```

## Optional block parameters may be nil

```ruby
def optional_block_truthiness(&block)
  if block
    :provided
  else
    :missing
  end
end

def optional_block_alias_truthiness(&block)
  callback = block
  if callback
    :provided
  else
    :missing
  end
end

def optional_block_call(&block)
  return :missing unless block

  block.call
end
```

### result

```rbs
class Object < BasicObject
  def optional_block_truthiness: (?untyped &block) -> (:missing | :provided)
  def optional_block_alias_truthiness: (?untyped &block) -> (:missing | :provided)
  def optional_block_call: (?untyped &block) -> (untyped | :missing)
end
```

## Repeated literal checks preserve branch-local values

### update

```rbs
class FrameSource
  def self.next_frame: -> (180 | 3000)
end
```

```ruby
def frame_extent
  frames = FrameSource.next_frame
  if frames == 3000
    break_end = 20
  end

  if frames == 180
    10
  else
    break_end + 10
  end
end
```

### result

```rbs
class Object < BasicObject
  def frame_extent: -> Integer
end
```

## Aliased assignment invalidates branch-local correlations

### update

```ruby
def alias_write_does_not_reuse_branch_value(flag)
  if flag
    selector = :left
    payload = [:left]
  else
    selector = :right
    payload = [:right]
  end

  replacement = :left
  selector = replacement
  if selector == :left
    payload
  end
end
```

### result

```rbs
class Object < BasicObject
  def alias_write_does_not_reuse_branch_value: (untyped flag) -> [:left | :right]?
end

```

## Correlated alternatives preserve untyped payloads

### update

```rbs
module DynamicPayloadSource
  def self.value: -> untyped
end
```

```ruby
def selected_payload_keeps_untyped(flag, nested_flag)
  if flag
    selector = nested_flag ? :left : :middle
    payload = nested_flag ? DynamicPayloadSource.value : []
  else
    selector = :right
    payload = [:right]
  end

  if selector == :left || selector == :middle
    payload
  end
end
```

### result

```rbs
class Object < BasicObject
  def selected_payload_keeps_untyped: (untyped flag, untyped nested_flag) -> (nil | untyped | [ ])
end

```

## Correlations require standard scalar equality dispatch

### update

```ruby
module UnreliableSymbolEquality
  refine Symbol do
    def ==(_other) = true
  end
end

using UnreliableSymbolEquality

def refined_equality_does_not_select_payload(flag)
  if flag
    selector = :left
    payload = [:left]
  else
    selector = :right
    payload = [:right]
  end

  if selector == :left
    payload
  end
end
```

### result

```rbs
class Object < BasicObject
  def refined_equality_does_not_select_payload: (untyped flag) -> [:left | :right]?
end

```

## Frozen array constants retain safe include narrowing

### update

```rbs
module PrefixValueSource
  def self.value: -> String?
end
```

```ruby
class PrefixFilter
  PREFIXES = %w[pre tag].freeze
  MUTABLE_PREFIXES = %w[pre tag]

  def self.frozen_prefix
    value = PrefixValueSource.value
    if PREFIXES.include?(value)
      value
    else
      :miss
    end
  end

  def self.mutable_prefix
    value = PrefixValueSource.value
    if MUTABLE_PREFIXES.include?(value)
      value
    else
      :miss
    end
  end
end
```

### result

```rbs
class PrefixFilter
  PREFIXES: ["pre", "tag"]
  MUTABLE_PREFIXES: ["pre", "tag"]

  def self.frozen_prefix: -> String | :miss
  def self.mutable_prefix: -> (String | :miss)?
end

```

## assignment on an exiting branch does not reach the following read

### update

```ruby
def branch_local_after_return(flag)
  if flag
    value = :assigned
    return :returned
  end
  value
end
```

### result

```rbs
class Object < BasicObject
  def branch_local_after_return: (untyped flag) -> :returned?
end
```

## complete assignments across elsif exits stay non-nil

```yaml
known_issue: true
```

### update

```ruby
def body_after_range_branches(ranges, head)
  if ranges.nil?
    ranges = [:single]
  elsif ranges.empty?
    return :empty
  else
    partial = true
    body = :partial
  end

  if head
    body = :head
  elsif !partial
    body = :full
  end

  body
end

def maybe_ranges(flag)
  flag ? [0..10] : nil
end

class RangeResponseIterator
  def bytesize
    10
  end
end

def body_after_typed_range_branches(flag, head)
  ranges = maybe_ranges(flag)
  if ranges.nil?
    ranges = [:single]
  elsif ranges.empty?
    return :empty
  else
    partial = true
    body = :partial
  end

  if head
    body = :head
  elsif !partial
    body = :full
  end

  body
end

def range_response_after_nested_branch(flag, head, single_range)
  ranges = maybe_ranges(flag)
  status = 200
  headers = {}
  if ranges.nil?
    ranges = [0..10]
  elsif ranges.empty?
    return :empty
  else
    partial = true
    if single_range
      headers[:content_range] = :single
    else
      headers[:content_type] = :multiple
    end
    status = 206
    body = RangeResponseIterator.new
  end

  if head
    body = []
  elsif !partial
    body = :full
  end

  [status, headers, body]
end

def iterator_body_after_typed_range_branches(flag, head)
  ranges = maybe_ranges(flag)
  if ranges.nil?
    ranges = [0..10]
  elsif ranges.empty?
    return :empty
  else
    partial = true
    body = RangeResponseIterator.new
  end

  if head
    body = []
  elsif !partial
    body = :full
  end

  body
end

def first_range_after_empty_guard
  ranges = Array.new(1, 0..10)
  return :empty if ranges.empty?
  ranges[0]
end

def first_generic_array_after_clear
  values = [1].map { |value| value }
  return :empty if values.empty?
  values.clear
  values[0]
end

first_generic_array_after_clear
```

### result

```rbs
class Object < BasicObject
  def body_after_range_branches: (untyped ranges, untyped head) -> (:empty | :full | :head | :partial)
  def maybe_ranges: (untyped flag) -> [Range[Integer]]?
  def body_after_typed_range_branches: (untyped flag, untyped head) -> (:full | :head | :partial)
  def range_response_after_nested_branch: (untyped flag, untyped head, untyped single_range) -> ([200, Hash[untyped, untyped], :full] | [200, Hash[untyped, untyped], [ ]] | [206, Hash[:content_range, :single] | Hash[:content_type, :multiple], RangeResponseIterator] | [206, Hash[:content_range, :single] | Hash[:content_type, :multiple], [ ]])
  def iterator_body_after_typed_range_branches: (untyped flag, untyped head) -> (:full | RangeResponseIterator | [ ])
  def first_range_after_empty_guard: -> Range[Integer]
  def first_generic_array_after_clear: -> nil
end

class RangeResponseIterator
  def bytesize: -> 10
end
```
