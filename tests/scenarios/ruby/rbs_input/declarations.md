# Ruby / RBS Input / Declarations

## RBS attr declarations provide reader and writer method types

```rbs
class RbsProfile
  attr_reader name: String
  attr_accessor age: Integer
end
```

```ruby
def rbs_profile_name(profile)
  profile.name
end

def rbs_profile_age(profile)
  profile.age
end

rbs_profile_name(RbsProfile.new)
rbs_profile_age(RbsProfile.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_profile_name: (RbsProfile profile) -> String
  def rbs_profile_age: (RbsProfile profile) -> Integer
end
```

## RBS interface and include declarations participate in method lookup

```rbs
interface _RbsRenderable
  def render: -> String
end

class RbsWidget
  include _RbsRenderable
end
```

```ruby
def rbs_render(widget)
  widget.render
end

rbs_render(RbsWidget.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_render: (RbsWidget widget) -> String
end
```

## RBS interface parameter selects matching overload

```rbs
interface _RbsPayload
  def token: -> String
end

class RbsSource
  def take: (_RbsPayload payload) -> String
          | (untyped payload) -> Integer
end
```

```ruby
class RbsPayload
  def token
    "x"
  end
end

def rbs_take(source, payload)
  source.take(payload)
end

rbs_take(RbsSource.new, RbsPayload.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_take: (RbsSource source, RbsPayload payload) -> String
end

class RbsPayload
  def token: -> "x"
end
```

## RBS interface parameter matches inside array overload

```rbs
interface _RbsItem
  def code: -> String
end

class RbsBatch
  def collect: (Array[_RbsItem] items) -> Array[String]
             | (untyped items) -> Integer
end
```

```ruby
class RbsItem
  def code
    "x"
  end
end

def rbs_collect(batch, items)
  batch.collect(items)
end

rbs_collect(RbsBatch.new, [RbsItem.new])
```

### result

```rbs
class Object < BasicObject
  def rbs_collect: (RbsBatch batch, Array[RbsItem] items) -> Array[String]
end

class RbsItem
  def code: -> "x"
end
```

## RBS interface parameter checks method arity

```rbs
interface _RbsFormatter
  def format: (String value) -> String
end

class RbsPrinter
  def print: (_RbsFormatter formatter) -> String
           | (untyped formatter) -> Integer
end
```

```ruby
class RbsFormatter
  def format
    "x"
  end
end

def rbs_print(printer, formatter)
  printer.print(formatter)
end

rbs_print(RbsPrinter.new, RbsFormatter.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_print: (RbsPrinter printer, RbsFormatter formatter) -> Integer
end

class RbsFormatter
  def format: -> "x"
end
```

## RBS interface parameter accepts required keyword method

```rbs
interface _RbsKeywordFormatter
  def format: (value: String) -> String
end

class RbsKeywordPrinter
  def print: (_RbsKeywordFormatter formatter) -> String
           | (untyped formatter) -> Integer
end
```

```ruby
class RbsKeywordFormatter
  def format(value:)
    "x"
  end
end

def rbs_keyword_print(printer, formatter)
  printer.print(formatter)
end

rbs_keyword_print(RbsKeywordPrinter.new, RbsKeywordFormatter.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_keyword_print: (RbsKeywordPrinter printer, RbsKeywordFormatter formatter) -> String
end

class RbsKeywordFormatter
  def format: (value: untyped) -> "x"
end
```

## RBS interface parameter checks required keyword

```rbs
interface _RbsRequiredKeywordFormatter
  def format: (value: String) -> String
end

class RbsRequiredKeywordPrinter
  def print: (_RbsRequiredKeywordFormatter formatter) -> String
           | (untyped formatter) -> Integer
end
```

```ruby
class RbsRequiredKeywordFormatter
  def format
    "x"
  end
end

def rbs_required_keyword_print(printer, formatter)
  printer.print(formatter)
end

rbs_required_keyword_print(RbsRequiredKeywordPrinter.new, RbsRequiredKeywordFormatter.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_required_keyword_print: (RbsRequiredKeywordPrinter printer, RbsRequiredKeywordFormatter formatter) -> Integer
end

class RbsRequiredKeywordFormatter
  def format: -> "x"
end
```

## RBS interface parameter checks method return type

```rbs
interface _RbsStringSource
  def token: -> String
end

class RbsStringConsumer
  def take: (_RbsStringSource source) -> String
          | (untyped source) -> Integer
end
```

```ruby
class RbsNumberSource
  def token
    1
  end
end

def rbs_string_take(consumer, source)
  consumer.take(source)
end

rbs_string_take(RbsStringConsumer.new, RbsNumberSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_string_take: (RbsStringConsumer consumer, RbsNumberSource source) -> Integer
end

class RbsNumberSource
  def token: -> 1
end
```

## RBS interface parameter accepts wider method parameter type

```rbs
interface _RbsStringFormatterWider
  def format: (String value) -> String
end

class RbsWiderFormatter
  def format: (String | Integer value) -> String
end

class RbsWiderPrinter
  def print: (_RbsStringFormatterWider formatter) -> String
           | (untyped formatter) -> Integer
end
```

```ruby
def rbs_wider_print(printer, formatter)
  printer.print(formatter)
end

rbs_wider_print(RbsWiderPrinter.new, RbsWiderFormatter.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_wider_print: (RbsWiderPrinter printer, RbsWiderFormatter formatter) -> String
end
```

## RBS interface parameter checks method parameter type

```rbs
interface _RbsStringFormatterParam
  def format: (String value) -> String
end

class RbsIntegerFormatterParam
  def format: (Integer value) -> String
end

class RbsParamPrinter
  def print: (_RbsStringFormatterParam formatter) -> String
           | (untyped formatter) -> Integer
end
```

```ruby
def rbs_param_print(printer, formatter)
  printer.print(formatter)
end

rbs_param_print(RbsParamPrinter.new, RbsIntegerFormatterParam.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_param_print: (RbsParamPrinter printer, RbsIntegerFormatterParam formatter) -> Integer
end
```

## RBS interface parameter checks keyword parameter type

```rbs
interface _RbsKeywordStringFormatter
  def format: (value: String) -> String
end

class RbsKeywordIntegerFormatter
  def format: (value: Integer) -> String
end

class RbsKeywordParamPrinter
  def print: (_RbsKeywordStringFormatter formatter) -> String
           | (untyped formatter) -> Integer
end
```

```ruby
def rbs_keyword_param_print(printer, formatter)
  printer.print(formatter)
end

rbs_keyword_param_print(RbsKeywordParamPrinter.new, RbsKeywordIntegerFormatter.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_keyword_param_print: (RbsKeywordParamPrinter printer, RbsKeywordIntegerFormatter formatter) -> Integer
end
```

## RBS interface parameter checks required block presence

```rbs
interface _RbsBlockIterable
  def each: () { (String value) -> void } -> void
end

class RbsNoBlockIterable
  def each: () -> void
end

class RbsBlockConsumer
  def take: (_RbsBlockIterable iterable) -> String
          | (untyped iterable) -> Integer
end
```

```ruby
def rbs_block_iterable_take(consumer, iterable)
  consumer.take(iterable)
end

rbs_block_iterable_take(RbsBlockConsumer.new, RbsNoBlockIterable.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_block_iterable_take: (RbsBlockConsumer consumer, RbsNoBlockIterable iterable) -> Integer
end
```

## RBS interface parameter checks block yield type

```rbs
interface _RbsStringIterable
  def each: () { (String value) -> void } -> void
end

class RbsIntegerIterable
  def each: () { (Integer value) -> void } -> void
end

class RbsStringIterableConsumer
  def take: (_RbsStringIterable iterable) -> String
          | (untyped iterable) -> Integer
end
```

```ruby
def rbs_string_iterable_take(consumer, iterable)
  consumer.take(iterable)
end

rbs_string_iterable_take(RbsStringIterableConsumer.new, RbsIntegerIterable.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_string_iterable_take: (RbsStringIterableConsumer consumer, RbsIntegerIterable iterable) -> Integer
end
```

## RBS interface include contributes required methods

```rbs
interface _RbsNamedParent
  def name: -> String
end

interface _RbsNamedChild
  include _RbsNamedParent
end

class RbsNamedChildConsumer
  def take: (_RbsNamedChild value) -> String
          | (untyped value) -> Integer
end

class RbsNamedOnly
  def name: -> String
end
```

```ruby
def rbs_interface_include_take(consumer, value)
  consumer.take(value)
end

rbs_interface_include_take(RbsNamedChildConsumer.new, RbsNamedOnly.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_interface_include_take: (RbsNamedChildConsumer consumer, RbsNamedOnly value) -> String
end
```

## RBS generic interface include resolves type parameters

```rbs
interface _RbsParentValue[E]
  def value: -> E
end

interface _RbsChildValue[T]
  include _RbsParentValue[T]
end

class RbsStringValue
  def value: -> String
end

class RbsGenericInterfaceConsumer
  def collect: [T] (_RbsChildValue[T] value) -> Array[T]
end
```

```ruby
def rbs_generic_interface_include_value
  RbsGenericInterfaceConsumer.new.collect(RbsStringValue.new)
end

rbs_generic_interface_include_value
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_interface_include_value: -> Array[String]
end
```

## RBS builtin interface checks each type argument

```rbs
class RbsEachStringConsumer
  def take: (_Each[String, void] values) -> String
          | (untyped values) -> Integer
end
```

```ruby
def rbs_each_string_take(consumer, values)
  consumer.take(values)
end

rbs_each_string_take(RbsEachStringConsumer.new, [1])
```

### result

```rbs
class Object < BasicObject
  def rbs_each_string_take: (RbsEachStringConsumer consumer, Array[Integer] values) -> Integer
end
```

## RBS builtin interface resolves each return type argument

```rbs
class RbsEachReturnConsumer
  def take: [E, R] (_Each[E, R] values) -> [E, R]
end
```

```ruby
def rbs_each_return_value
  RbsEachReturnConsumer.new.take([1])
end

rbs_each_return_value
```

### result

```rbs
class Object < BasicObject
  def rbs_each_return_value: -> [1, [1]]
end
```

## RBS builtin interface checks array type argument

```rbs
class RbsToArrayStringConsumer
  def take: (_ToA[String] values) -> String
          | (untyped values) -> Integer
end
```

```ruby
def rbs_to_array_string_take(consumer, values)
  consumer.take(values)
end

rbs_to_array_string_take(RbsToArrayStringConsumer.new, [1])
```

### result

```rbs
class Object < BasicObject
  def rbs_to_array_string_take: (RbsToArrayStringConsumer consumer, Array[Integer] values) -> Integer
end
```

## RBS builtin interface checks hash type arguments

```rbs
class RbsToHashStringConsumer
  def take: (_ToHash[Symbol, String] values) -> String
          | (untyped values) -> Integer
end
```

```ruby
def rbs_to_hash_string_take(consumer, values)
  consumer.take(values)
end

rbs_to_hash_string_take(RbsToHashStringConsumer.new, { count: 1 })
```

### result

```rbs
class Object < BasicObject
  def rbs_to_hash_string_take: (RbsToHashStringConsumer consumer, { count: Integer } values) -> Integer
end
```

## RBS builtin interface resolves hash type arguments

```rbs
class RbsToHashGenericConsumer
  def build: [K, V] (_ToHash[K, V] values) -> Hash[K, V]
end
```

```ruby
def rbs_to_hash_generic_value
  RbsToHashGenericConsumer.new.build({ name: "x" })
end

rbs_to_hash_generic_value
```

### result

```rbs
class Object < BasicObject
  def rbs_to_hash_generic_value: -> Hash[Symbol, "x"]
end
```

## RBS builtin interface checks range type argument

```rbs
class RbsRangeStringConsumer
  def take: (_Range[String] range) -> String
          | (untyped range) -> Integer
end
```

```ruby
def rbs_range_string_take(consumer, range)
  consumer.take(range)
end

rbs_range_string_take(RbsRangeStringConsumer.new, 1..3)
```

### result

```rbs
class Object < BasicObject
  def rbs_range_string_take: (RbsRangeStringConsumer consumer, Range[Integer] range) -> Integer
end
```

## RBS builtin interface resolves range type argument

```rbs
class RbsRangeGenericConsumer
  def pick: [T] (_Range[T] range) -> T
end
```

```ruby
def rbs_range_generic_value
  RbsRangeGenericConsumer.new.pick(1..3)
end

rbs_range_generic_value
```

### result

```rbs
class Object < BasicObject
  def rbs_range_generic_value: -> Integer
end
```

## RBS method type parameter checks upper bound

```rbs
class RbsBoundedGenericConsumer
  def take: [T < _ToHash[Symbol, String]] (T values) -> String
          | (untyped values) -> Integer
end
```

```ruby
def rbs_bounded_generic_take(consumer, values)
  consumer.take(values)
end

def rbs_bounded_generic_take_ok(consumer, values)
  consumer.take(values)
end

rbs_bounded_generic_take(RbsBoundedGenericConsumer.new, { name: 1 })
rbs_bounded_generic_take_ok(RbsBoundedGenericConsumer.new, { name: "x" })
```

### result

```rbs
class Object < BasicObject
  def rbs_bounded_generic_take: (RbsBoundedGenericConsumer consumer, { name: Integer } values) -> Integer
  def rbs_bounded_generic_take_ok: (RbsBoundedGenericConsumer consumer, { name: String } values) -> String
end
```

## RBS method type parameter upper bound fills unresolved return

```rbs
class RbsBoundedGenericFactory
  def label: [T < String] () -> T
end
```

```ruby
def rbs_bounded_generic_label
  RbsBoundedGenericFactory.new.label
end

rbs_bounded_generic_label
```

### result

```rbs
class Object < BasicObject
  def rbs_bounded_generic_label: -> String
end
```

## RBS method type parameter checks lower bound

```rbs
class RbsLowerBoundedGenericConsumer
  def take: [T > String] (T value) -> String
          | (untyped value) -> Integer
end
```

```ruby
def rbs_lower_bounded_generic_take(consumer, value)
  consumer.take(value)
end

def rbs_lower_bounded_generic_take_ok(consumer, value)
  consumer.take(value)
end

rbs_lower_bounded_generic_take(RbsLowerBoundedGenericConsumer.new, :name)
rbs_lower_bounded_generic_take_ok(RbsLowerBoundedGenericConsumer.new, "x")
```

### result

```rbs
class Object < BasicObject
  def rbs_lower_bounded_generic_take: (RbsLowerBoundedGenericConsumer consumer, Symbol value) -> Integer
  def rbs_lower_bounded_generic_take_ok: (RbsLowerBoundedGenericConsumer consumer, String value) -> String
end
```

## RBS method type parameter lower bound widens literal return

```rbs
class RbsLowerBoundedGenericIdentity
  def take: [T > String] (T value) -> T
end
```

```ruby
def rbs_lower_bounded_generic_value
  RbsLowerBoundedGenericIdentity.new.take("x")
end

rbs_lower_bounded_generic_value
```

### result

```rbs
class Object < BasicObject
  def rbs_lower_bounded_generic_value: -> String
end
```

## RBS method type parameter checks upper bound from block return

```rbs
class RbsBoundedBlockGenericFactory
  def label: [T < String] () { () -> T } -> T
end
```

```ruby
def rbs_bounded_block_generic_label
  RbsBoundedBlockGenericFactory.new.label { "x" }
end

def rbs_bounded_block_generic_invalid_label
  RbsBoundedBlockGenericFactory.new.label { 1 }
end

rbs_bounded_block_generic_label
rbs_bounded_block_generic_invalid_label
```

### result

```rbs
class Object < BasicObject
  def rbs_bounded_block_generic_label: -> "x"
  def rbs_bounded_block_generic_invalid_label: -> untyped
end
```

## RBS class type parameter upper bound fills omitted receiver argument

```rbs
class RbsBoundedValue[T < String]
  def value: -> T
end

class RbsBoundedValueChild < RbsBoundedValue
end

class RbsBoundedValueSource
  def value: -> RbsBoundedValue
end
```

```ruby
def rbs_bounded_value_results(source)
  [
    source.value.value,
    RbsBoundedValueChild.new.value
  ]
end

rbs_bounded_value_results(RbsBoundedValueSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_bounded_value_results: (RbsBoundedValueSource source) -> [String, String]
end
```

## RBS interface type parameter upper bound fills omitted parameter argument

```rbs
interface _RbsBoundedNamed[T < String]
  def name: -> T
end

class RbsBoundedNamedConsumer
  def take: (_RbsBoundedNamed value) -> String
          | (untyped value) -> Integer
end

class RbsBoundedNamedValue
  def name: -> String
end
```

```ruby
def rbs_bounded_named_take(consumer, value)
  consumer.take(value)
end

rbs_bounded_named_take(RbsBoundedNamedConsumer.new, RbsBoundedNamedValue.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_bounded_named_take: (RbsBoundedNamedConsumer consumer, RbsBoundedNamedValue value) -> String
end
```

## RBS class type parameter default fills omitted receiver argument

```rbs
class RbsDefaultPair[T, U = String]
  def second: -> U
end

class RbsDefaultPairSource
  def pair: -> RbsDefaultPair[Integer]
end
```

```ruby
def rbs_default_pair_second
  RbsDefaultPairSource.new.pair.second
end

rbs_default_pair_second
```

### result

```rbs
class Object < BasicObject
  def rbs_default_pair_second: -> String
end
```

## RBS module type parameter default fills omitted mixin argument

```rbs
module RbsDefaultReadable[Elem = String]
  def value: -> Elem
end

class RbsDefaultReadableUser
  include RbsDefaultReadable
end
```

```ruby
def rbs_default_readable_value
  RbsDefaultReadableUser.new.value
end

rbs_default_readable_value
```

### result

```rbs
class Object < BasicObject
  def rbs_default_readable_value: -> String
end
```

## RBS module type parameter upper bound fills omitted mixin argument

```rbs
module RbsBoundedReadable[Elem < String]
  def value: -> Elem
end

class RbsBoundedReadableUser
  include RbsBoundedReadable
end
```

```ruby
def rbs_bounded_readable_value
  RbsBoundedReadableUser.new.value
end

rbs_bounded_readable_value
```

### result

```rbs
class Object < BasicObject
  def rbs_bounded_readable_value: -> String
end
```

## RBS core class parameters match runtime primitives

```rbs
class RbsCoreClassConsumer
  def object_value: (Object value) -> String
                  | (untyped value) -> Integer

  def nil_value: (NilClass value) -> String
               | (untyped value) -> Integer

  def true_value: (TrueClass value) -> String
                | (untyped value) -> Integer

  def false_value: (FalseClass value) -> String
                 | (untyped value) -> Integer

  def class_value: (Class value) -> String
                 | (untyped value) -> Integer

  def proc_value: (Proc value) -> String
                | (untyped value) -> Integer
end
```

```ruby
def rbs_core_class_values(consumer)
  [
    consumer.object_value(:name),
    consumer.nil_value(nil),
    consumer.true_value(true),
    consumer.false_value(false),
    consumer.class_value(String),
    consumer.proc_value(-> { :ok })
  ]
end

rbs_core_class_values(RbsCoreClassConsumer.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_core_class_values: (RbsCoreClassConsumer consumer) -> [String, String, String, String, String, String]
end
```

## RBS primitive class returns normalize to literal runtime types

```rbs
class RbsPrimitiveClassReturnSource
  def value: -> (TrueClass | FalseClass | NilClass)
end
```

```ruby
def rbs_primitive_class_return(source)
  source.value
end

rbs_primitive_class_return(RbsPrimitiveClassReturnSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_primitive_class_return: (RbsPrimitiveClassReturnSource source) -> bool?
end
```

## RBS nominal parameter accepts registry ancestors

```rbs
class RbsNominalAnimal
end

class RbsNominalDog < RbsNominalAnimal
end

module RbsNominalNamed
end

class RbsNominalUser
  include RbsNominalNamed
end

class RbsNominalConsumer
  def animal: (RbsNominalAnimal value) -> String
            | (untyped value) -> Integer

  def named: (RbsNominalNamed value) -> String
           | (untyped value) -> Integer
end
```

```ruby
def rbs_nominal_registry_values(consumer)
  [
    consumer.animal(RbsNominalDog.new),
    consumer.named(RbsNominalUser.new)
  ]
end

rbs_nominal_registry_values(RbsNominalConsumer.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_nominal_registry_values: (RbsNominalConsumer consumer) -> [String, String]
end
```

## RBS generic nominal parameter checks inherited type arguments

```rbs
class RbsNominalBox[Item]
end

class RbsNominalStringBox < RbsNominalBox[String]
end

class RbsNominalIntegerBox < RbsNominalBox[Integer]
end

class RbsNominalBoxConsumer
  def take: (RbsNominalBox[String] value) -> String
          | (untyped value) -> Integer
end
```

```ruby
def rbs_generic_nominal_values(consumer)
  [
    consumer.take(RbsNominalStringBox.new),
    consumer.take(RbsNominalIntegerBox.new)
  ]
end

rbs_generic_nominal_values(RbsNominalBoxConsumer.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_nominal_values: (RbsNominalBoxConsumer consumer) -> [String, Integer]
end
```

## RBS generic nominal parameter resolves inherited type argument

```rbs
class RbsNominalValueBox[Item]
end

class RbsNominalStringValueBox < RbsNominalValueBox[String]
end

class RbsNominalValueConsumer
  def take: [T] (RbsNominalValueBox[T] value) -> T
end
```

```ruby
def rbs_generic_nominal_value(consumer)
  consumer.take(RbsNominalStringValueBox.new)
end

rbs_generic_nominal_value(RbsNominalValueConsumer.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_nominal_value: (RbsNominalValueConsumer consumer) -> String
end
```

## RBS singleton parameter accepts subclass constants

```rbs
class RbsSingletonParent
end

class RbsSingletonChild < RbsSingletonParent
end

class RbsSingletonConsumer
  def take: (singleton(RbsSingletonParent) value) -> String
          | (untyped value) -> Integer
end
```

```ruby
def rbs_singleton_subclass_value(consumer)
  consumer.take(RbsSingletonChild)
end

rbs_singleton_subclass_value(RbsSingletonConsumer.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_singleton_subclass_value: (RbsSingletonConsumer consumer) -> String
end
```

## RBS generic superclass arguments feed inherited methods

```rbs
class RbsGenericParent[T]
  def value: -> T
end

class RbsGenericStringChild < RbsGenericParent[String]
end

class RbsGenericChild[T] < RbsGenericParent[T]
end

class RbsGenericChildSource
  def child: -> RbsGenericChild[Integer]
end
```

```ruby
def rbs_generic_superclass_values(source)
  [
    RbsGenericStringChild.new.value,
    source.child.value
  ]
end

rbs_generic_superclass_values(RbsGenericChildSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_superclass_values: (RbsGenericChildSource source) -> [String, Integer]
end
```

## RBS generic ancestor overloads use owner arguments

```rbs
class RbsGenericOverloadParent[T]
  def pick: (String key) -> T
          | (Integer key) -> Symbol
end

class RbsGenericOverloadStringChild < RbsGenericOverloadParent[String]
end

class RbsGenericOverloadChild[T] < RbsGenericOverloadParent[T]
end

class RbsGenericOverloadChildSource
  def child: -> RbsGenericOverloadChild[Integer]
end
```

```ruby
def rbs_generic_ancestor_overload_values(source)
  [
    RbsGenericOverloadStringChild.new.pick("x"),
    RbsGenericOverloadStringChild.new.pick(1),
    source.child.pick("x"),
    source.child.pick(1)
  ]
end

rbs_generic_ancestor_overload_values(RbsGenericOverloadChildSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_ancestor_overload_values: (RbsGenericOverloadChildSource source) -> [String, Symbol, Integer, Symbol]
end
```

## RBS generic ancestor binary overloads feed symbol reduce

```rbs
class RbsGenericAdderParent[T]
  def +: (RbsGenericAdderParent[T] other) -> T
end

class RbsGenericIntegerAdder < RbsGenericAdderParent[Integer]
end

class RbsGenericAdderSource
  def adders: -> Array[RbsGenericIntegerAdder]
end
```

```ruby
def rbs_generic_ancestor_symbol_reduce_value(source)
  source.adders.inject(:+)
end

rbs_generic_ancestor_symbol_reduce_value(RbsGenericAdderSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_ancestor_symbol_reduce_value: (RbsGenericAdderSource source) -> Integer
end
```

## RBS generic ancestor arguments feed method objects

```rbs
class RbsGenericMethodParent[T]
  def value: -> T
end

class RbsGenericMethodStringChild < RbsGenericMethodParent[String]
end

class RbsGenericMethodChild[T] < RbsGenericMethodParent[T]
end

class RbsGenericMethodChildSource
  def child: -> RbsGenericMethodChild[Integer]
end

module RbsGenericMethodReadable[T]
  def read: -> T
end

class RbsGenericMethodStringReader
  include RbsGenericMethodReadable[String]
end

class RbsGenericMethodReader[T]
  include RbsGenericMethodReadable[T]
end

class RbsGenericMethodReaderSource
  def reader: -> RbsGenericMethodReader[Integer]
end
```

```ruby
def rbs_generic_ancestor_method_object_values(child_source, reader_source)
  [
    RbsGenericMethodStringChild.new.method(:value).call,
    child_source.child.method(:value).call,
    RbsGenericMethodStringChild.instance_method(:value).bind_call(RbsGenericMethodStringChild.new),
    RbsGenericMethodStringReader.new.method(:read).call,
    reader_source.reader.method(:read).call
  ]
end

rbs_generic_ancestor_method_object_values(
  RbsGenericMethodChildSource.new,
  RbsGenericMethodReaderSource.new
)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_ancestor_method_object_values: (RbsGenericMethodChildSource child_source, RbsGenericMethodReaderSource reader_source) -> [String, Integer, String, String, Integer]
end
```

## RBS generic ancestor block arguments feed symbol procs

```rbs
class RbsGenericSymbolParent[T]
  def map: [U] () { (T value) -> U } -> Array[U]
end

class RbsGenericSymbolStringChild < RbsGenericSymbolParent[String]
end

class RbsGenericSymbolChild[T] < RbsGenericSymbolParent[T]
end

class RbsGenericSymbolChildSource
  def child: -> RbsGenericSymbolChild[Integer]
end

module RbsGenericSymbolReadable[T]
  def map: [U] () { (T value) -> U } -> Array[U]
end

class RbsGenericSymbolStringReader
  include RbsGenericSymbolReadable[String]
end

class RbsGenericSymbolReader[T]
  include RbsGenericSymbolReadable[T]
end

class RbsGenericSymbolReaderSource
  def reader: -> RbsGenericSymbolReader[Integer]
end
```

```ruby
def rbs_generic_ancestor_symbol_proc_values(child_source, reader_source)
  [
    RbsGenericSymbolStringChild.new.map(&:itself),
    child_source.child.map(&:itself),
    RbsGenericSymbolStringReader.new.map(&:itself),
    reader_source.reader.map(&:itself)
  ]
end

rbs_generic_ancestor_symbol_proc_values(
  RbsGenericSymbolChildSource.new,
  RbsGenericSymbolReaderSource.new
)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_ancestor_symbol_proc_values: (RbsGenericSymbolChildSource child_source, RbsGenericSymbolReaderSource reader_source) -> [Array[String], Array[Integer], Array[String], Array[Integer]]
end
```

## RBS generic receiver method return feeds symbol procs

```rbs
class RbsSymbolItem[T]
  def value: -> T
end

class RbsSymbolItemSource
  def strings: -> Array[RbsSymbolItem[String]]
  def integers: -> Array[RbsSymbolItem[Integer]]
  def string_item: -> RbsSymbolItem[String]
end
```

```ruby
def rbs_symbol_proc_generic_receiver_values(source)
  [
    source.strings.map(&:value),
    source.integers.map(&:value),
    source.string_item.then(&:value)
  ]
end

rbs_symbol_proc_generic_receiver_values(RbsSymbolItemSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_symbol_proc_generic_receiver_values: (RbsSymbolItemSource source) -> [Array[String], Array[Integer], String]
end
```

## RBS generic mixin arguments feed included methods

```rbs
module RbsGenericReadable[T]
  def read: -> T
end

class RbsGenericStringReader
  include RbsGenericReadable[String]
end

class RbsGenericReader[T]
  include RbsGenericReadable[T]
end

class RbsGenericReaderSource
  def reader: -> RbsGenericReader[Integer]
end
```

```ruby
def rbs_generic_mixin_values(source)
  [
    RbsGenericStringReader.new.read,
    source.reader.read
  ]
end

rbs_generic_mixin_values(RbsGenericReaderSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_mixin_values: (RbsGenericReaderSource source) -> [String, Integer]
end
```

## RBS generic extend arguments feed singleton methods

```rbs
module RbsGenericFactory[T]
  def build: -> T
end

class RbsGenericStringFactory
  extend RbsGenericFactory[String]
end
```

```ruby
def rbs_generic_extend_value
  RbsGenericStringFactory.build
end

rbs_generic_extend_value
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_extend_value: -> String
end
```

## RBS generic prepend arguments feed prepended methods

```rbs
module RbsGenericPrependedReadable[T]
  def read: -> T
end

class RbsGenericPrependedStringReader
  prepend RbsGenericPrependedReadable[String]
end

class RbsGenericPrependedReader[T]
  prepend RbsGenericPrependedReadable[T]
end

class RbsGenericPrependedReaderSource
  def reader: -> RbsGenericPrependedReader[Integer]
end
```

```ruby
def rbs_generic_prepend_values(source)
  [
    RbsGenericPrependedStringReader.new.read,
    source.reader.read
  ]
end

rbs_generic_prepend_values(RbsGenericPrependedReaderSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_prepend_values: (RbsGenericPrependedReaderSource source) -> [String, Integer]
end
```

## RBS generic mixin block arguments feed side effects

```rbs
module RbsGenericIterable[T]
  def each: () { (T value) -> void } -> self
end

class RbsGenericStringIterable
  include RbsGenericIterable[String]
end

class RbsGenericIterableBox[T]
  include RbsGenericIterable[T]
end

class RbsGenericIterableSource
  def box: -> RbsGenericIterableBox[Integer]
end
```

```ruby
def rbs_generic_mixin_block_string
  values = []
  RbsGenericStringIterable.new.each { |value| values << value }
  values
end

def rbs_generic_mixin_block_integer(source)
  values = []
  source.box.each { |value| values << value }
  values
end

rbs_generic_mixin_block_string
rbs_generic_mixin_block_integer(RbsGenericIterableSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_mixin_block_string: -> Array[String]
  def rbs_generic_mixin_block_integer: (RbsGenericIterableSource source) -> Array[Integer]
end
```

## RBS generic module self type arguments feed required ancestor methods

```rbs
interface _RbsRequiredReadable[T]
  def read: -> T
end

module RbsRequiredReader : _RbsRequiredReadable[String]
end
```

```ruby
module RbsRequiredReader
  def rbs_required_read
    read
  end
end
```

### result

```rbs
module RbsRequiredReader
  def rbs_required_read: -> String
end
```

## RBS generic superclass block arguments feed side effects

```rbs
class RbsGenericIterableParent[T]
  def each: () { (T value) -> void } -> self
end

class RbsGenericStringIterableChild < RbsGenericIterableParent[String]
end

class RbsGenericIterableChild[T] < RbsGenericIterableParent[T]
end

class RbsGenericIterableChildSource
  def child: -> RbsGenericIterableChild[Integer]
end
```

```ruby
def rbs_generic_superclass_block_string
  values = []
  RbsGenericStringIterableChild.new.each { |value| values << value }
  values
end

def rbs_generic_superclass_block_integer(source)
  values = []
  source.child.each { |value| values << value }
  values
end

rbs_generic_superclass_block_string
rbs_generic_superclass_block_integer(RbsGenericIterableChildSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_generic_superclass_block_string: -> Array[String]
  def rbs_generic_superclass_block_integer: (RbsGenericIterableChildSource source) -> Array[Integer]
end
```

## RBS interface resolves type parameters from method return

```rbs
interface _RbsToHashPair[K, V]
  def to_hash: () -> Hash[K, V]
end

class RbsPairHash
  def to_hash: () -> Hash[Symbol, String]
end

class RbsHashPairConsumer
  def build: [K, V] (_RbsToHashPair[K, V] value) -> Hash[K, V]
end
```

```ruby
def rbs_hash_pair_value
  RbsHashPairConsumer.new.build(RbsPairHash.new)
end

rbs_hash_pair_value
```

### result

```rbs
class Object < BasicObject
  def rbs_hash_pair_value: -> Hash[Symbol, String]
end
```

## RBS interface resolves type parameters from block yield

```rbs
interface _RbsEachValue[E]
  def each: () { (E value) -> void } -> void
end

class RbsStringEachValue
  def each: () { (String value) -> void } -> void
end

class RbsEachValueConsumer
  def collect: [T] (_RbsEachValue[T] value) -> Array[T]
end
```

```ruby
def rbs_each_value_result
  RbsEachValueConsumer.new.collect(RbsStringEachValue.new)
end

rbs_each_value_result
```

### result

```rbs
class Object < BasicObject
  def rbs_each_value_result: -> Array[String]
end
```

## RBS interface resolves type parameters from keyword block yield

```rbs
interface _RbsEachNamedValue[E]
  def each: () { (name: E) -> void } -> void
end

class RbsStringEachNamedValue
  def each: () { (name: String) -> void } -> void
end

class RbsEachNamedValueConsumer
  def collect: [T] (_RbsEachNamedValue[T] value) -> Array[T]
end
```

```ruby
def rbs_each_named_value_result
  RbsEachNamedValueConsumer.new.collect(RbsStringEachNamedValue.new)
end

rbs_each_named_value_result
```

### result

```rbs
class Object < BasicObject
  def rbs_each_named_value_result: -> Array[String]
end
```

## RBS nested lexical type names resolve inside class body

```rbs
class RbsLexicalOuter
  class Item
  end

  class Inner
    class Item
    end
  end

  class Source
    DEFAULT: Item
    def item: -> Item
    def nested_item: -> Inner::Item
  end
end
```

```ruby
def rbs_lexical_nested_values(source)
  [
    source.item,
    source.nested_item,
    RbsLexicalOuter::Source::DEFAULT
  ]
end

rbs_lexical_nested_values(RbsLexicalOuter::Source.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_lexical_nested_values: (RbsLexicalOuter::Source source) -> [RbsLexicalOuter::Item, RbsLexicalOuter::Inner::Item, RbsLexicalOuter::Item]
end
```

## RBS mixin declarations expose module methods to including classes

```rbs
module RbsNamed
  def name: -> String
end

class RbsUser
  include RbsNamed
end
```

```ruby
def rbs_user_name(user)
  user.name
end

rbs_user_name(RbsUser.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_user_name: (RbsUser user) -> String
end
```

## RBS ivar declarations feed Ruby method body inference

```rbs
class RbsBox
  @value: String
end
```

```ruby
class RbsBox
  def value
    @value
  end
end
```

### result

```rbs
class RbsBox
  def value: -> String
end
```

## RBS global variable declarations feed Ruby global reads

```rbs
$rbs_config: String
```

```ruby
def rbs_global_config
  $rbs_config
end
```

### result

```rbs
class Object < BasicObject
  def rbs_global_config: -> String
end
```

## Generic RBS type aliases substitute type arguments

```rbs
type rbs_boxed[T] = Array[T]

class RbsAliasSource
  def names: -> rbs_boxed[String]
end
```

```ruby
def rbs_alias_names(source)
  source.names
end

rbs_alias_names(RbsAliasSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_alias_names: (RbsAliasSource source) -> Array[String]
end
```

## Generic RBS type aliases keep custom class type arguments

```rbs
class RbsAliasWrapper[T]
end

type rbs_wrapped[T] = RbsAliasWrapper[T]

class RbsCustomAliasSource
  def wrapped: -> rbs_wrapped[String]
end
```

```ruby
def rbs_custom_alias(source)
  source.wrapped
end

rbs_custom_alias(RbsCustomAliasSource.new)
```

### result

```rbs
class Object < BasicObject
  def rbs_custom_alias: (RbsCustomAliasSource source) -> RbsAliasWrapper[String]
end
```

## RBS class instance variables feed singleton method body inference

```rbs
class RbsSingletonStore
  self.@label: String
end
```

```ruby
class RbsSingletonStore
  def self.label
    @label
  end
end
```

### result

```rbs
class RbsSingletonStore
  def self.label: -> String
end
```

## RBS class variables feed Ruby class variable reads

```rbs
class RbsClassState
  @@state: Symbol
end
```

```ruby
class RbsClassState
  def state
    @@state
  end
end
```

### result

```rbs
class RbsClassState
  def state: -> Symbol
end
```
