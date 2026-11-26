# Rails / DSL / GraphQL

## GraphQL arguments do not define writers

### update

```ruby
module Types
  class BaseInputObject < GraphQL::Schema::InputObject; end
end

class Mutations::BaseMutation < GraphQL::Schema::RelayClassicMutation; end
class Resolvers::BaseResolver < GraphQL::Schema::Resolver; end

class Types::UserInput < Types::BaseInputObject
  argument :name, String, required: true
end

class Mutations::CreateUser < Mutations::BaseMutation
  argument :title, String, required: true
end

class Resolvers::Users < Resolvers::BaseResolver
  argument :limit, Integer, required: false
end
```

### result

```rbs
class Mutations::BaseMutation < GraphQL::Schema::RelayClassicMutation
end

class Mutations::CreateUser < Mutations::BaseMutation
end

class Resolvers::BaseResolver < GraphQL::Schema::Resolver
end

class Resolvers::Users < Resolvers::BaseResolver
end

module Types
end

class Types::BaseInputObject < GraphQL::Schema::InputObject
end

class Types::UserInput < Types::BaseInputObject
  def name: -> untyped
end
```

## GraphQL object fields do not define readers

### update

```ruby
module Types
  class BaseObject < GraphQL::Schema::Object; end
end

class Types::UserType < Types::BaseObject
  field :email, String, null: false

  def kind = :user
end
```

### result

```rbs
module Types
end

class Types::BaseObject < GraphQL::Schema::Object
end

class Types::UserType < Types::BaseObject
  def kind: -> :user
end
```

## Non-GraphQL field macros keep a reader

### update

```ruby
class Integration
  field :username
end
```

### result

```rbs
class Integration
  def username: -> untyped
end
```
