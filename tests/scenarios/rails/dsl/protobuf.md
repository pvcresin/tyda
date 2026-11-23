# Rails / DSL / Protobuf

## Name protobuf accessors from the field name argument

### update

```ruby
class Protobuf::Message; end

class UserMessage < Protobuf::Message
  optional :string, :name, 1
  required ::Protobuf::Field::Int32Field, :age, 2
end
```

### result

```rbs
class Protobuf::Message
end

class UserMessage < Protobuf::Message
  def name: -> untyped
  def name=: (untyped name) -> untyped
  def name!: -> untyped
  def age: -> untyped
  def age=: (untyped age) -> untyped
  def age!: -> untyped
end
```

## Type repeated protobuf fields as FieldArray

### update

```ruby
class Protobuf::Message; end

class UserMessage < Protobuf::Message
  repeated ::Protobuf::Field::StringField, :tags, 1
end
```

### result

```rbs
class Protobuf::Message
end

class UserMessage < Protobuf::Message
  def tags: -> Protobuf::Field::FieldArray
  def tags=: (untyped tags) -> untyped
  def tags!: -> Protobuf::Field::FieldArray?
end
```
