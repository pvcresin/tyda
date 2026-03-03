# Ruby / Class / Conditional Def

## Extend sibling module in scope to add singleton method

### update

```ruby
module App
  module Helpers
    def shout(s) = s.upcase
  end
  extend Helpers
end

def test = App.shout("hi")
```

### result

```rbs
module App
  extend Helpers
end

module App::Helpers
  def shout: (String s) -> String
end

class Object
  def test: -> String
end
```

## Resolve through mixin merged from another file

### update

```ruby
module Rake
  module FileUtilsExt
    def verbose(value = nil) = value
  end
  extend FileUtilsExt
end

def probe = Rake.verbose(true)
```

### result

```rbs
class Object
  def probe: -> bool?
end

module Rake
  extend FileUtilsExt
end

module Rake::FileUtilsExt
  def verbose: (?bool? value) -> bool?
end
```

## Collect `def self.` under if guard

### update

```ruby
module Foo
  if true
    def self.greet
      "hi"
    end
  end
end

def test = Foo.greet
```

### result

```rbs
module Foo
  def self.greet: -> "hi"
end

class Object
  def test: -> "hi"
end
```

## Collect `def` under unless guard

### update

```ruby
class Bar
  unless false
    def greet
      42
    end
  end
end

def test = Bar.new.greet
```

### result

```rbs
class Bar
  def greet: -> 42
end

class Object
  def test: -> 42
end
```

## Collect `def` under begin block

### update

```ruby
class Baz
  begin
    def shout
      "hi"
    end
  end
end

def test = Baz.new.shout
```

### result

```rbs
class Baz
  def shout: -> "hi"
end

class Object
  def test: -> "hi"
end
```

## defined? guard with same-name self method

### update

```ruby
module Utils
  if defined?(SOMETHING_UNAVAILABLE)
    def self.clock_time
      1.0
    end
  else
    def self.clock_time
      2.0
    end
  end
end

def elapsed = Utils.clock_time
```

### result

```rbs
class Object
  def elapsed: -> 2.0
end

module Utils
  def self.clock_time: -> 2.0
end
```

## Resolve same-scope superclass stored as short name

### update

```ruby
module Rake
  module DSL
    def task(name) = name
  end
  class TaskLib
    include Rake::DSL
  end
  class PackageTask < TaskLib
    def define
      task :package
    end
  end
end
```

### result

```rbs
module Rake::DSL
  def task: (Symbol name) -> Symbol
end

class Rake::PackageTask < TaskLib
  def define: -> Symbol
end

class Rake::TaskLib
  include Rake::DSL
end
```

## Resolve with enclosing class data across files

### update

```ruby
module App
  class Base
    def name = "app"
  end
  class Child < Base
    def call = name
  end
end
```

### result

```rbs
class App::Base
  def name: -> "app"
end

class App::Child < Base
  def call: -> "app"
end
```

## Follow parent scope across deep nesting

### update

```ruby
module Outer
  module Inner
    class Root
      def foo = 1
    end
  end
  class Leaf < Inner::Root
    def bar = foo
  end
end
```

### result

```rbs
class Outer::Inner::Root
  def foo: -> 1
end

class Outer::Leaf < Inner::Root
  def bar: -> 1
end
```
