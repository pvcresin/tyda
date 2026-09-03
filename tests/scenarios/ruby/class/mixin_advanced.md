# Ruby / Class / Mixin Advanced

## Use include and extend together

### update

```ruby
module InstanceMethods
  def greet = "hello"
end

module ClassMethods
  def create = "new"
end

class User
  include InstanceMethods
  extend ClassMethods

  def name = "Alice"
end
```

### result

```rbs
module ClassMethods
  def create: -> "new"
end

module InstanceMethods
  def greet: -> "hello"
end

class User
  include InstanceMethods
  extend ClassMethods

  def name: -> "Alice"
end
```

## Resolve included method with RBS annotation

### update

```ruby
module Formatter
  #: (Integer) -> String
  def format_number(n) = n.to_s
end

class Report
  include Formatter

  def summary = format_number(42)
end
```

### result

```rbs
module Formatter
  def format_number: (Integer n) -> String
end

class Report
  include Formatter

  def summary: -> String
end
```

## Prepend overrides method while keeping original type

### update

```ruby
module Logging
  def process = "logged"
end

class Worker
  prepend Logging

  def process = 42
end
```

### result

```rbs
module Logging
  def process: -> "logged"
end

class Worker
  prepend Logging

  def process: -> 42
end
```

## include with multiple args

### update

```ruby
module A
  def method_a = "a"
end

module B
  def method_b = "b"
end

class C
  include A, B

  def method_c = "c"
end
```

### result

```rbs
module A
  def method_a: -> "a"
end

module B
  def method_b: -> "b"
end

class C
  include A
  include B

  def method_c: -> "c"
end
```

## Combine RBS include and Ruby include

### update

```rbs
module Validatable
  def valid?: () -> bool
end
```

```ruby
class Form
  include Validatable

  def submit = "submitted"
end
```

### result

```rbs
class Form
  include Validatable

  def submit: -> "submitted"
end
```

## Resolve singleton method through extend

### update

```ruby
module Findable
  #: (Integer) -> String
  def find(id) = "found"
end

class Record
  extend Findable
end
```

### result

```rbs
module Findable
  def find: (Integer id) -> String
end

class Record
  extend Findable
end
```

## Plain included hook extends nested class methods

### update

```ruby
module Attachable
  def self.included(base)
    base.extend(ClassMethods)
  end

  module ClassMethods
    def build(value) = value.to_s
  end
end

class Item
  include Attachable
end

def result = Item.build(1)
```

### result

```rbs
module Attachable
  def self.included: (untyped base) -> untyped
end

module Attachable::ClassMethods
  def build: (Integer value) -> String
end

class Item
  include Attachable
  extend Attachable::ClassMethods
end

class Object < BasicObject
  def result: -> String
end
```

## Apply extend inside plain included hook branch

### update

```ruby
module Configurable
  def self.included(base)
    if defined?(base)
      base.extend(BuilderMethods)
    end
  end

  module BuilderMethods
    def enabled? = true
  end
end

class Entry
  include Configurable
end

def check = Entry.enabled?
```

### result

```rbs
module Configurable
  def self.included: (untyped base) -> (nil | untyped)
end

module Configurable::BuilderMethods
  def enabled?: -> true
end

class Entry
  include Configurable
  extend Configurable::BuilderMethods
end

class Object < BasicObject
  def check: -> true
end
```

## Resolve nested ClassMethods convention without hook or Concern

### update

```ruby
module Worker
  module ClassMethods
    def perform_async = :queued
  end
end

class Job
  include Worker
end

def result = Job.perform_async
```

### result

```rbs
class Job
  include Worker
end

class Object < BasicObject
  def result: -> :queued
end

module Worker::ClassMethods
  def perform_async: -> :queued
end
```

## Resolve cross-file nested ClassMethods convention

### update

`lib/worker.rb`

```ruby
module Worker
  module ClassMethods
    def perform_in(interval) = interval
  end
end
```

```ruby
class Job
  include Worker
end

def result = Job.perform_in(5)
```

### result

```rbs
class Job
  include Worker
end

class Object < BasicObject
  def result: -> Integer
end
```

## Apply plain included hook extend across files

### update

`lib/feature.rb`

```ruby
module Feature
  def self.included(base)
    base.extend(ClassMethods)
  end

  module ClassMethods
    def label = "label"
  end
end
```

```ruby
class Item
  include Feature
end

def label = Item.label
```

### result

```rbs
class Item
  include Feature
  extend Feature::ClassMethods
end

class Object < BasicObject
  def label: -> "label"
end
```

## Static receiver include links project module

### update

```ruby
module Shared
  def label = "label"
end

class Item
end

Item.include Shared

def label = Item.new.label
```

### result

```rbs
class Item
  include Shared
end

class Object < BasicObject
  def label: -> "label"
end

module Shared
  def label: -> "label"
end
```

## Static dispatch mixin calls link project modules

### update

```ruby
module Builder
  def build = :built
end

module Wrapper
  def name = "wrapped"
end

class Entry
  def name = "entry"
end

Entry.public_send(:extend, Builder)
Entry.send(:prepend, Wrapper)

def built = Entry.build
def name = Entry.new.name
```

### result

```rbs
module Builder
  def build: -> :built
end

class Entry
  extend Builder
  prepend Wrapper

  def name: -> "entry"
end

class Object < BasicObject
  def built: -> :built
  def name: -> "wrapped"
end

module Wrapper
  def name: -> "wrapped"
end
```

## Plain extended hook includes nested instance methods

### update

```ruby
module Usable
  def self.extended(base)
    base.include(InstanceMethods)
  end

  module InstanceMethods
    def tag = :tag
  end
end

class Record
  extend Usable
end

def tag = Record.new.tag
```

### result

```rbs
class Object < BasicObject
  def tag: -> :tag
end

class Record
  extend Usable
  include Usable::InstanceMethods
end

module Usable
  def self.extended: (untyped base) -> untyped
end

module Usable::InstanceMethods
  def tag: -> :tag
end
```

## Plain prepended hook extends nested class methods

### update

```ruby
module Wrapped
  def self.prepended(base)
    base.extend(ClassMethods)
  end

  def name = "wrapped"

  module ClassMethods
    def build = :built
  end
end

class Entry
  prepend Wrapped

  def name = "entry"
end

def built = Entry.build
def name = Entry.new.name
```

### result

```rbs
class Entry
  prepend Wrapped
  extend Wrapped::ClassMethods

  def name: -> "entry"
end

class Object < BasicObject
  def built: -> :built
  def name: -> "wrapped"
end

module Wrapped
  def self.prepended: (untyped base) -> untyped
  def name: -> "wrapped"
end

module Wrapped::ClassMethods
  def build: -> :built
end
```
