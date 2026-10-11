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
```

### result

```rbs
class Object < BasicObject
  def proc_truthiness: -> :truthy
end
```

## Keep nil-returning calls reachable for open dispatch

### update

```ruby
class DynamicFieldPayload
  def initialize(query: nil)
    @query = query
  end

  def query
    @query
  end

  def log_data
    data = { query_sha: "sha" }
    data[:query] = query if query
    data
  end

  def log_data_through_local
    query_value = query
    saved_query = query_value
    data = { query_sha: "sha" }
    data[:query] = saved_query if saved_query
    data
  end

  def log_data_after_conditional(flag)
    value = nil
    if flag
      value = query
    end
    data = { query_sha: "sha" }
    data[:query] = value if value
    data
  end

  def literal_nil_local
    value = nil
    if value
      :unreachable
    else
      :missing
    end
  end

  def literal_nil_after_dynamic_call
    value = query && nil
    if value
      :unreachable
    else
      :missing
    end
  end

  def literal_nil_from_dynamic_and
    if query && nil
      :unreachable
    else
      :missing
    end
  end

  def dynamic_and_value
    value = query && :present
    if value
      :present
    else
      :missing
    end
  end

  def literal_nil_from_dynamic_predicate
    value = if query
      nil
    else
      nil
    end
    if value
      :unreachable
    else
      :missing
    end
  end

  def falsey_and_branch_values(flag)
    value = query && (flag ? nil : false)
    if value
      :unreachable
    else
      :missing
    end
  end
end

class DynamicFieldPayloadChild < DynamicFieldPayload
  def query
    "query"
  end
end

```

### result

```rbs
class DynamicFieldPayload
  def initialize: (?query: nil) -> void
  def query: -> nil
  def log_data: -> { query_sha: "sha", ?query: nil }
  def log_data_through_local: -> { query_sha: "sha", ?query: nil }
  def log_data_after_conditional: (untyped flag) -> { query_sha: "sha", ?query: nil }
  def literal_nil_local: -> :missing
  def literal_nil_after_dynamic_call: -> :missing
  def literal_nil_from_dynamic_and: -> :missing
  def dynamic_and_value: -> :missing | :present
  def literal_nil_from_dynamic_predicate: -> :missing
  def falsey_and_branch_values: (untyped flag) -> :missing
end

class DynamicFieldPayloadChild < DynamicFieldPayload
  def query: -> "query"
end

```

## Preserve record shape for finite dynamic hash writes

```ruby
class FiniteDynamicHashKey
  KEY_TO_FIELD = {
    "project" => :project_ids,
    "group" => :group_ids
  }.freeze

  def append(id)
    data = { project_ids: [], group_ids: [] }
    key = KEY_TO_FIELD[id]
    data[key] << id if key
    data
  end
end
```

### result

```rbs
class FiniteDynamicHashKey
  KEY_TO_FIELD: { "project" => :project_ids, "group" => :group_ids }

  def append: (untyped id) -> { project_ids: Array[untyped], group_ids: Array[untyped] }
end
```

## Keep finite hash values after an open-dispatch key lookup

```ruby
class DynamicRecordLookupKey
  KEY_TO_FIELD = {
    "project" => :project_ids,
    "group" => :group_ids
  }.freeze

  def lookup_source
    nil
  end

  def append(id)
    data = { project_ids: [], group_ids: [] }
    key = KEY_TO_FIELD[lookup_source]
    data[key] << id if key
    data
  end

  def append_many(namespaces)
    data = { project_ids: [], group_ids: [] }
    namespaces.each do |namespace|
      key = KEY_TO_FIELD[namespace.type]
      data[key] << namespace.id if key
    end
    data
  end
end
```

### result

```rbs
class DynamicRecordLookupKey
  KEY_TO_FIELD: { "project" => :project_ids, "group" => :group_ids }

  def lookup_source: -> nil
  def append: (untyped id) -> { project_ids: Array[untyped], group_ids: Array[untyped] }
  def append_many: (untyped namespaces) -> { project_ids: Array[untyped], group_ids: Array[untyped] }
end
```

## Preserve optional fields from conditional hash splats

```ruby
class ConditionalRecordHashSplat
  def collect(flag)
    {
      **(flag ? {} : { failed_builds: %i[project metadata] }),
      manual_actions: []
    }
  end
end
```

### result

```rbs
class ConditionalRecordHashSplat
  def collect: (untyped flag) -> { ?failed_builds: [:project, :metadata], manual_actions: [ ] }
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

## Ignore dynamic calls in statically unreachable result branches

```ruby
class UnreachableDynamicTruthiness
  def query
    nil
  end

  def result
    value = if false
      query
    else
      nil
    end

    if value
      :unreachable
    else
      :missing
    end
  end
end

class UnreachableDynamicTruthinessChild < UnreachableDynamicTruthiness
  def query
    "present"
  end
end
```

### result

```rbs
class UnreachableDynamicTruthiness
  def query: -> nil
  def result: -> :missing
end

class UnreachableDynamicTruthinessChild < UnreachableDynamicTruthiness
  def query: -> "present"
end
```

## Preserve nil for finite record keys that may be absent

```ruby
class FiniteRecordLookupWithMissingKey
  def lookup_present(flag)
    values = { first: :first, second: :second }
    key = flag ? :first : :second
    values[key]
  end

  def lookup(flag)
    values = { present: :value }
    key = flag ? :present : :missing
    values[key]
  end
end
```

### result

```rbs
class FiniteRecordLookupWithMissingKey
  def lookup_present: (untyped flag) -> (:first | :second)
  def lookup: (untyped flag) -> :value?
end
```
