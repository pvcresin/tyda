# Ruby / Variable / Assignment Nodes

## Global variable writes are available to later reads in the same file

### update

```ruby
def global_assign
  $g_value = "value"
  $g_value
end

def global_or_write
  $g_or ||= 1
  $g_or
end

def global_and_write
  $g_and = true
  $g_and &&= "yes"
  $g_and
end

def global_operator_write
  $g_number = 1
  $g_number += 2
  $g_number
end
```

### result

```rbs
class Object
  def global_assign: -> "value"
  def global_or_write: -> 1
  def global_and_write: -> "yes"
  def global_operator_write: -> Integer
end
```

## Built-in global variables keep their Ruby core types

### update

```ruby
def input = $stdin
def output = $stdout
def error = $stderr
def load_path = $LOAD_PATH
def program_name = $PROGRAM_NAME
def verbose = $VERBOSE
def process_id = $$
def status = $?
def error_info = $!
def backtrace = $@
def match_data = $~
def matched_text = $&
def left_text = $`
def right_text = $'
def output_separator = $\
```

### result

```rbs
class Object
  def input: -> IO
  def output: -> IO
  def error: -> IO
  def load_path: -> Array[String]
  def program_name: -> String
  def verbose: -> bool?
  def process_id: -> Integer
  def status: -> Process::Status?
  def error_info: -> Exception?
  def backtrace: -> Array[String]?
  def match_data: -> MatchData?
  def matched_text: -> String?
  def left_text: -> String?
  def right_text: -> String?
  def output_separator: -> String?
end
```

## Constant path and shareable constant writes keep their RHS type

### update

```ruby
module AssignNodes
end

AssignNodes::VALUE = 1
AssignNodes::OTHER ||= "x"
AssignNodes::COUNT = 1
AssignNodes::COUNT += 2

# shareable_constant_value: literal
SHARED_VALUE = "shared"

def const_path_value = AssignNodes::VALUE
def const_path_or = AssignNodes::OTHER
def const_path_operator = AssignNodes::COUNT
def shareable_value = SHARED_VALUE
```

### result

```rbs
SHARED_VALUE: "shared"

module AssignNodes
  VALUE: 1
  OTHER: "x"
  COUNT: Integer
end

class Object
  def const_path_value: -> 1
  def const_path_or: -> "x"
  def const_path_operator: -> Integer
  def shareable_value: -> "shared"
end
```

## Index conditional writes update exact local collection shapes

### update

```ruby
def hash_or_write
  h = {}
  h[:name] ||= "ruby"
  h[:name]
end

def hash_and_write
  h = {enabled: true}
  h[:enabled] &&= "yes"
  h[:enabled]
end

def hash_operator_write
  h = {count: 1}
  h[:count] += 2
  h[:count]
end
```

### result

```rbs
class Object
  def hash_or_write: -> "ruby"?
  def hash_and_write: -> "yes"
  def hash_operator_write: -> Integer
end
```
