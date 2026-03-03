# Rails / DSL / Draper

## Decoratable model generates decorate

### update

```ruby
module Draper
  module Decoratable
  end

  class Decorator
  end
end

class A
  include Draper::Decoratable
end

class ADecorator < Draper::Decorator
end
```

### result

```rbs
class A
  include Draper::Decoratable

  def decorate: -> ADecorator
end

class ADecorator < Draper::Decorator
  def self.decorate: (A object, **untyped options) -> ADecorator
  def initialize: (A object, **untyped options) -> void
  def object: -> A
  def a: -> A
end
```

## Decorator applies explicit target and delegate_all

### update

```ruby
module Draper
  class Decorator
  end
end

class A
  #: () -> String
  def foo = "hello"
end

class ADecorator < Draper::Decorator
  decorates :a
  delegate_all
  decorates_finders
end
```

### result

```rbs
class A
  def foo: -> String
end

class ADecorator < Draper::Decorator
  extend Draper::Finders

  def self.decorate: (A object, **untyped options) -> ADecorator
  def initialize: (A object, **untyped options) -> void
  def object: -> A
  def a: -> A
  def foo: -> String
end
```
