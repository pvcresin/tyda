# Sorbet / Sig / Multi Sig

## Two sigs

### update

```ruby
class Converter
  sig { params(x: Integer).returns(String) }
  sig { params(x: String).returns(Integer) }
  def convert(x) = x.to_s
end
```

### result

```rbs
class Converter
  def convert: (Integer x) -> String
             | (String x) -> Integer
end
```

## Three sigs

### update

```ruby
class Parser
  sig { params(input: String).returns(Integer) }
  sig { params(input: Integer).returns(String) }
  sig { params(input: Float).returns(String) }
  def parse(input) = input.to_s
end
```

### result

```rbs
class Parser
  def parse: (String input) -> Integer
           | (Integer input) -> String
           | (Float input) -> String
end
```

## Same-name singleton and instance sigs keep separate param types

### update

```ruby
class Tag
  sig { params(owner_person_id: T.untyped, person_ids: T::Array[T.untyped]).returns(T::Array[T.untyped]) }
  def self.reindex_collected_person_ids(owner_person_id, person_ids) = person_ids

  sig { params(person_ids: T::Array[T.untyped]).returns(T::Array[T.untyped]) }
  def reindex_collected_person_ids(person_ids) = self.class.reindex_collected_person_ids(0, person_ids)
end
```

### result

```rbs
class Tag
  def self.reindex_collected_person_ids: (untyped owner_person_id, Array[untyped] person_ids) -> Array[untyped]
  def reindex_collected_person_ids: (Array[untyped] person_ids) -> Array[untyped]
end
```
