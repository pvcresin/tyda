# Ruby / Runtime / Builtins

## Built-in conversion through ARGV element

### update

```ruby
def foo(n) = n.to_s

foo(ARGV[0].to_i)
```

### result

```rbs
class Object < BasicObject
  def foo: (Integer n) -> String
end
```

## `__FILE__` and `__LINE__`

### update

```ruby
def file_name = __FILE__

def line_no = __LINE__
```

### result

```rbs
class Object < BasicObject
  def file_name: -> String
  def line_no: -> Integer
end
```

## `__ENCODING__`

### update

```ruby
def enc = __ENCODING__
```

### result

```rbs
class Object < BasicObject
  def enc: -> Encoding
end
```

## `__method__` / `__callee__` / `__dir__`

### update

```ruby
def current_method_name = __method__

def current_callee_name = __callee__

def current_dir = __dir__
```

### result

```rbs
class Object < BasicObject
  def current_method_name: -> :current_method_name
  def current_callee_name: -> :current_callee_name
  def current_dir: -> String
end
```

## `self.class` and value `.class`

### update

```ruby
def self_class = self.class

def int_class = 1.class

def array_class = [1].class

def hash_class = ({ 1 => "x" }).class

def class_class = Object.class

def unknown_class(x) = x.class
```

### result

```rbs
class Object < BasicObject
  def self_class: -> singleton(Object)
  def int_class: -> singleton(Integer)
  def array_class: -> singleton(Array)
  def hash_class: -> singleton(Hash)
  def class_class: -> singleton(Class)
  def unknown_class: (untyped x) -> Class
end
```

## Runtime constants and final data stream

### update

```ruby
def stdout_const = STDOUT
def argf_const = ARGF
def env_const = ENV
def data_const = DATA
def data_read = DATA.read
def gc_disable = GC.disable
__END__
payload
```

### result

```rbs
class Object < BasicObject
  def stdout_const: -> IO
  def argf_const: -> RBS::Unnamed::ARGFClass
  def env_const: -> RBS::Unnamed::ENVClass
  def data_const: -> IO
  def data_read: -> String
  def gc_disable: -> bool
end
```

## Core class aliases resolve to their target

### update

```ruby
def make_mutex = Mutex.new
def make_queue = Queue.new
def make_cv = ConditionVariable.new
def make_sq = SizedQueue.new(2)
```

### result

```rbs
class Object < BasicObject
  def make_mutex: -> Thread::Mutex
  def make_queue: -> Thread::Queue
  def make_cv: -> Thread::ConditionVariable
  def make_sq: -> Thread::SizedQueue
end
```

## File path helpers return String

### update

```ruby
def base = File.basename("/x/y.rb")
def ext = File.extname("/x/y.rb")
def dir = File.dirname("/x/y.rb")
```

### result

```rbs
class Object < BasicObject
  def base: -> String
  def ext: -> String
  def dir: -> String
end
```
