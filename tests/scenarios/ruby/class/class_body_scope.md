# Ruby / Class / Class Body Scope

## `class C` creates a new scope

### update

```ruby
outer_val = "x"

class A
  def f = outer_val
end
```

### result

```rbs
class A
  def f: -> untyped
end
```

## `def f` does not capture class body locals

### update

```ruby
class A
  body_local = "x"

  def f = body_local
end
```

### result

```rbs
class A
  def f: -> untyped
end
```

## Yielded block defines a method in the enclosing class body

### update

```ruby
class Module
  def install_generic(method)
    yield unless method_defined?(method)
  end

  def ignore_generic
    nil
  end
end

class ClassBodyYield
  install_generic(:generated) do
    def generated = "generated"
  end

  ignore_generic do
    def ignored = "ignored"
  end
end

def read_generated = ClassBodyYield.new.generated
```

### result

```rbs
class ClassBodyYield
  def generated: -> "generated"
end

class Module
  def install_generic: [T] (untyped method) { () -> T } -> T?
  def ignore_generic: -> nil
end

class Object < BasicObject
  def read_generated: -> "generated"
end
```

## Static iterator bindings define only unskipped attribute readers

### update

```ruby
class IteratorAttributeReaders
  DEFAULT_OPTIONS = {}
  options = {
    video: {
      frames: { default: 1 },
      shortcut: { shortcut: "--frames" },
    },
  }

  options.each_value do |group|
    group.each do |name, option|
      next if option[:shortcut]
      DEFAULT_OPTIONS[name] = option[:default] if option.key?(:default)
      attr_reader name
    end
  end

  def initialize
    @frames = 1
  end
end

def read_iterator_attribute = IteratorAttributeReaders.new.frames
```

### result

```rbs
class IteratorAttributeReaders
  DEFAULT_OPTIONS: Hash[:frames | :shortcut, untyped]

  def frames: -> 1
  def initialize: -> void
end

class Object < BasicObject
  def read_iterator_attribute: -> 1
end
```

## Do not expand an iterator overridden on its local receiver

```ruby
class IteratorOverride
  options = { fake: {} }
  options.define_singleton_method(:each) { nil }
  options.each { |name, _value| attr_reader name }

  def value = 1
end
```

### result

```rbs
class IteratorOverride
  def value: -> 1
end
```

## Preserve record fields in nested constant iterators

```ruby
module Optcarrot
class Config
  OPTIONS = {
    optimization: {
      opt_ppu: { type: :opts, candidates: Missing::OPTIONS, default: nil },
      opt: { shortcut: ["--opt-ppu=all", "--opt-cpu=all"] },
    },
    emulation: {
      sprite_limit: { type: :switch, default: false },
      frames: { type: :int, default: 0, aliases: [:f, :frame] },
    },
    profiling: {
      print_fps: { type: :switch, default: false },
      stackprof_mode: { type: "MODE", default: nil },
      stackprof: { shortcut: "--stackprof-mode=cpu" },
    },
    misc: {
      loglevel: { type: :int, default: 1 },
      quiet: { shortcut: "--loglevel=0", aliases: :q },
    },
  }
  DEFAULT_OPTIONS = {}

  OPTIONS.each_value do |group|
    group.each do |name, option|
      next if option[:shortcut]
      DEFAULT_OPTIONS[name] = option[:default] if option.key?(:default)
      attr_reader name
    end
  end

  def initialize
    @frames = 0
  end
end
end

def read_constant_iterator_attribute = Optcarrot::Config.new.frames
```

### result

```rbs
class Object < BasicObject
  def read_constant_iterator_attribute: -> 0
end

class Optcarrot::Config
  DEFAULT_OPTIONS: Hash[:frames | :loglevel | :opt | :opt_ppu | :print_fps | :quiet | :sprite_limit | :stackprof | :stackprof_mode, untyped]
  OPTIONS: { optimization: { opt_ppu: { type: :opts, candidates: untyped, default: nil }, opt: { shortcut: ["--opt-ppu=all", "--opt-cpu=all"] } }, emulation: { sprite_limit: { type: :switch, default: false }, frames: { type: :int, default: 0, aliases: [:f, :frame] } }, profiling: { print_fps: { type: :switch, default: false }, stackprof_mode: { type: "MODE", default: nil }, stackprof: { shortcut: "--stackprof-mode=cpu" } }, misc: { loglevel: { type: :int, default: 1 }, quiet: { shortcut: "--loglevel=0", aliases: :q } } }

  def opt_ppu: -> untyped
  def sprite_limit: -> untyped
  def frames: -> 0
  def print_fps: -> untyped
  def stackprof_mode: -> untyped
  def loglevel: -> untyped
  def initialize: -> void
end

```

## Skip iterator attributes after an unsupported side exit

### update

```ruby
class IteratorStopsOnBreak
  [:never].each do |name|
    break
    attr_reader name
  end
end

def read_after_iterator_break = IteratorStopsOnBreak.new.never
```

### result

```rbs
class Object < BasicObject
  def read_after_iterator_break: -> untyped
end
```

## Apply visibility changes inside a static iterator

### update

```ruby
class IteratorAttributeVisibility
  [:secret].each do |name|
    private
    attr_reader name
  end

  [:guarded].each do |name|
    protected
    attr_reader name
  end

  [:visible].each do |name|
    public
    attr_reader name
  end
end
```

### result

```rbs
class IteratorAttributeVisibility
  private def secret: -> untyped
  private def guarded: -> untyped
  def visible: -> untyped
end
```

## Preserve record writes while collecting iterator attributes

### update

```ruby
class IteratorRecordWrites
  DEFAULTS = {}
  OPTIONS = { frames: { default: 1 } }

  OPTIONS.each do |name, option|
    DEFAULTS[name] = option[:default]
    attr_reader name
  end

  def self.defaults = DEFAULTS
end

def read_iterator_defaults = IteratorRecordWrites.defaults
```

### result

```rbs
class IteratorRecordWrites
  DEFAULTS: Hash[:frames, 1]
  OPTIONS: { frames: { default: 1 } }

  def frames: -> untyped
  def self.defaults: -> Hash[:frames, 1]
end

class Object < BasicObject
  def read_iterator_defaults: -> Hash[:frames, 1]
end
```

## Skip static bindings for source-defined Hash iterators

### update

```ruby
class Hash
  def each = nil
  def each_value = nil
end

class IteratorWithOverriddenHashIterators
  { declared_each: {} }.each do |name, _option|
    attr_reader name
  end

  { declared_value: :declared_value }.each_value do |name|
    attr_reader name
  end
end
```

### result

```rbs
class Hash
  def each: -> nil
  def each_value: -> nil
end
```

## Skip static bindings for a local receiver's custom iterator

### update

```ruby
class IteratorWithLocalEachOverride
  options = { declared: {} }

  def options.each = nil

  options.each do |name, _option|
    attr_reader name
  end
end

def read_local_iterator_override = IteratorWithLocalEachOverride.new.declared
```

### result

```rbs
class Object < BasicObject
  def read_local_iterator_override: -> untyped
end
```

## Skip iterator collection when an unsupported case changes visibility

### update

```ruby
class IteratorVisibilityInCase
  [:secret].each do |name|
    case name
    when :secret
      private
    end
    attr_reader name
  end
end

def read_iterator_visibility_in_case = IteratorVisibilityInCase.new.secret
```

### result

```rbs
class Object < BasicObject
  def read_iterator_visibility_in_case: -> untyped
end
```
