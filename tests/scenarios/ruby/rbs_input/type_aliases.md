# Ruby / RBS Input / Type Aliases

## Resolve top-level alias

### update

```rbs
type a = Integer

class A
  def foo: -> ::a
end
```

```ruby
class A
  def bar = foo
end
```

### result

```rbs
class A
  def bar: -> Integer
end
```

## Resolve builtin generic aliases

### update

```rbs
class RbsBuiltinAliases
  def names: -> array[string]
  def counts: -> hash[interned, int]
  def indexes: -> range[int]
  def predicate: -> boolish
end
```

```ruby
class RbsBuiltinAliases
  def list = names
  def table = counts
  def span = indexes
  def condition = predicate
end
```

### result

```rbs
class RbsBuiltinAliases
  def list: -> Array[String]
  def table: -> Hash[String | Symbol, Integer]
  def span: -> Range[Integer]
  def condition: -> top
end
```

## Resolve builtin generic alias through type alias

### update

```rbs
type rbs_value_list[T] = array[T]

class RbsBuiltinAliasType
  def values: -> rbs_value_list[string]
end
```

```ruby
class RbsBuiltinAliasType
  def fetch = values
end
```

### result

```rbs
class RbsBuiltinAliasType
  def fetch: -> Array[String]
end
```

## Resolve generic alias default type argument

### update

```rbs
type rbs_default_list[T = String] = Array[T]

class RbsDefaultAliasSource
  def values: -> rbs_default_list
end
```

```ruby
class RbsDefaultAliasSource
  def fetch = values
end
```

### result

```rbs
class RbsDefaultAliasSource
  def fetch: -> Array[String]
end
```

## Resolve generic alias upper bound type argument

### update

```rbs
type rbs_bounded_list[T < String] = Array[T]

class RbsBoundedAliasSource
  def values: -> rbs_bounded_list
end
```

```ruby
class RbsBoundedAliasSource
  def fetch = values
end
```

### result

```rbs
class RbsBoundedAliasSource
  def fetch: -> Array[String]
end
```

## Choose overload through alias default type argument

### update

```rbs
type rbs_default_hash[V = String] = Hash[Symbol, V]

class RbsDefaultAliasOverload
  def pick: (rbs_default_hash value) -> String
          | (untyped value) -> Integer
end
```

```ruby
def rbs_default_alias_overload_bad
  RbsDefaultAliasOverload.new.pick({ name: 1 })
end

def rbs_default_alias_overload_ok
  RbsDefaultAliasOverload.new.pick({ name: "x" })
end

rbs_default_alias_overload_bad
rbs_default_alias_overload_ok
```

### result

```rbs
class Object < BasicObject
  def rbs_default_alias_overload_bad: -> Integer
  def rbs_default_alias_overload_ok: -> String
end
```

## Choose overload through alias upper bound type argument

### update

```rbs
type rbs_bounded_hash[V < String] = Hash[Symbol, V]

class RbsBoundedAliasOverload
  def pick: (rbs_bounded_hash value) -> String
          | (untyped value) -> Integer
end
```

```ruby
def rbs_bounded_alias_overload_bad
  RbsBoundedAliasOverload.new.pick({ name: 1 })
end

def rbs_bounded_alias_overload_ok
  RbsBoundedAliasOverload.new.pick({ name: "x" })
end

rbs_bounded_alias_overload_bad
rbs_bounded_alias_overload_ok
```

### result

```rbs
class Object < BasicObject
  def rbs_bounded_alias_overload_bad: -> Integer
  def rbs_bounded_alias_overload_ok: -> String
end
```

## Resolve builtin alias method type parameters

### update

```rbs
class RbsBuiltinAliasGeneric
  def first: [T] (array[T] values) -> T
  def value: [K, V] (hash[K, V] values) -> V
  def range_value: [T] (range[T] values) -> T
end
```

```ruby
def rbs_builtin_alias_array_value
  RbsBuiltinAliasGeneric.new.first([1])
end

def rbs_builtin_alias_hash_value
  RbsBuiltinAliasGeneric.new.value({ name: "x" })
end

def rbs_builtin_alias_range_value
  RbsBuiltinAliasGeneric.new.range_value(1..3)
end

rbs_builtin_alias_array_value
rbs_builtin_alias_hash_value
rbs_builtin_alias_range_value
```

### result

```rbs
class Object < BasicObject
  def rbs_builtin_alias_array_value: -> 1
  def rbs_builtin_alias_hash_value: -> "x"
  def rbs_builtin_alias_range_value: -> Integer
end
```

## Choose overload through type alias

### update

```rbs
type rbs_label = String

class RbsAliasOverload
  def pick: (Integer value) -> Integer
          | (rbs_label value) -> String
end
```

```ruby
def rbs_alias_overload_string
  RbsAliasOverload.new.pick("name")
end

def rbs_alias_overload_integer
  RbsAliasOverload.new.pick(1)
end
```

### result

```rbs
class Object < BasicObject
  def rbs_alias_overload_string: -> String
  def rbs_alias_overload_integer: -> Integer
end
```

## Choose overload through builtin alias type arguments

### update

```rbs
class RbsBuiltinAliasOverload
  def pick_array: (array[string] values) -> String
                | (untyped values) -> Integer

  def pick_hash: (hash[Symbol, String] values) -> String
               | (untyped values) -> Integer

  def pick_range: (range[string] values) -> String
                | (untyped values) -> Integer
end
```

```ruby
def rbs_builtin_alias_array_overload
  RbsBuiltinAliasOverload.new.pick_array([1])
end

def rbs_builtin_alias_hash_overload
  RbsBuiltinAliasOverload.new.pick_hash({ name: 1 })
end

def rbs_builtin_alias_range_overload
  RbsBuiltinAliasOverload.new.pick_range(1..3)
end

rbs_builtin_alias_array_overload
rbs_builtin_alias_hash_overload
rbs_builtin_alias_range_overload
```

### result

```rbs
class Object < BasicObject
  def rbs_builtin_alias_array_overload: -> Integer
  def rbs_builtin_alias_hash_overload: -> Integer
  def rbs_builtin_alias_range_overload: -> Integer
end
```

## Resolve class-local alias

### update

```rbs
class A
  type a = Integer | String

  def foo: (a) -> a
end
```

```ruby
class A
  def bar = foo(1)
end
```

### result

```rbs
class A
  def bar: -> Integer | String
end
```

## Resolve namespaced alias

### update

```rbs
class B
  type a = Integer
end

class A
  def foo: -> B::a
end
```

```ruby
class A
  def bar = foo
end
```

### result

```rbs
class A
  def bar: -> Integer
end
```
