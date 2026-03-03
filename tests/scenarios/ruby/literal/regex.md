# Ruby / Literal / Regex

## Regexp literal

### update

```ruby
def regex_test = /pattern/
```

### result

```rbs
class Object
  def regex_test: -> Regexp
end
```

## Regexp.new

### update

```ruby
def regex_new = Regexp.new("pattern")
```

### result

```rbs
class Object
  def regex_new: -> Regexp
end
```

## Match operator =~

### update

```ruby
def match_test = "hello" =~ /ell/
```

### result

```rbs
class Object
  def match_test: -> Integer?
end
```

## Regexp with interpolation

### update

```ruby
def interp_regex
  name = "world"
  /hello #{name}/
end
```

### result

```rbs
class Object
  def interp_regex: -> Regexp
end
```

## %r notation

### update

```ruby
def percent_r_regex = %r{a+b}
```

### result

```rbs
class Object
  def percent_r_regex: -> Regexp
end
```

## Regexp.union

### update

```ruby
def regex_union = Regexp.union(["0"] + ["1"])
```

### result

```rbs
class Object
  def regex_union: -> Regexp
end
```

## String#scan changes element type by capture groups

### update

```ruby
def scan_words
  "one two".scan(/\w+/)
end

def scan_pair_captures
  "a1 b2".scan(/(\w)(\d)/)
end

def scan_one_capture
  "a1 b2".scan(/(\w)\d/)
end

def scan_named_capture
  "one two".scan(/(?<word>\w+)/)
end

def scan_without_counting_non_captures
  "a1 b2".scan(/(?:\w)(?=\d)[(\w)]/)
end

def scan_string_pattern
  "one two".scan("o")
end
```

### result

```rbs
class Object
  def scan_words: -> Array[String]
  def scan_pair_captures: -> Array[[String?, String?]]
  def scan_one_capture: -> Array[[String?]]
  def scan_named_capture: -> Array[[String?]]
  def scan_without_counting_non_captures: -> Array[String]
  def scan_string_pattern: -> Array[String]
end
```

## String#scan block receives captures

### update

```ruby
def scan_block_words
  values = []
  "one two".scan(/\w+/) { |word| values << word }
  values
end

def scan_block_captures
  values = []
  "a1 b2".scan(/(\w)(\d)/) { |name, count| values << [name, count] }
  values
end

def scan_block_capture_tuple
  values = []
  "a1 b2".scan(/(\w)(\d)/) { |match| values << match }
  values
end
```

### result

```rbs
class Object
  def scan_block_words: -> Array[String]
  def scan_block_captures: -> Array[[String?, String?]]
  def scan_block_capture_tuple: -> Array[[String?, String?]]
end
```

## String#match keeps capture shape

### update

```ruby
def match_capture_list
  text = "path:12"
  text.match(/^(.+?):(\d+).*$/, &:captures)
end

def match_block_capture
  text = "name:12"
  text.match(/^(\w+):(\d+)$/) { |match| match[1] }
end

def match_capture_slice
  text = "name:12"
  text.match(/^(\w+):(\d+)$/)&.[](1, 2)
end

def match_capture_values(value)
  /^(\w+):(\d+)$/.match(value)&.values_at(1, 2)
end

def match_object
  "name".match(/^(\w+)$/)
end
```

### result

```rbs
class Object
  def match_capture_list: -> [String?, String?]?
  def match_block_capture: -> String?
  def match_capture_slice: -> [String?, String?]?
  def match_capture_values: (untyped value) -> [String?, String?]?
  def match_object: -> MatchData?
end
```

## MatchData keeps named capture shape

### update

```ruby
def match_named_symbol(text)
  match = /(?<host>[^:]+)(?::(?<port>\d+))?/.match(text)
  match&.[](:host)
end

def match_named_string(text)
  match = /(?<host>[^:]+)(?::(?<port>\d+))?/.match(text)
  match&.[]("port")
end

def match_named_values(text)
  match = /(?<host>[^:]+)(?::(?<port>\d+))?/.match(text)
  match&.values_at(:host, "port")
end

def match_named_hash(text)
  match = /(?<host>[^:]+)(?::(?<port>\d+))?/.match(text)
  match&.named_captures
end

def match_named_names(text)
  match = /(?<host>[^:]+)(?::(?<port>\d+))?/.match(text)
  match&.names
end

def regexp_receiver_named_capture(text)
  /(?<name>\w+):(?<count>\d+)/.match(text) { |match| [match[:name], match.values_at("count")] }
end
```

### result

```rbs
class Object
  def match_named_symbol: (untyped text) -> String?
  def match_named_string: (untyped text) -> String?
  def match_named_values: (untyped text) -> [String?, String?]?
  def match_named_hash: (untyped text) -> { "host" => String?, "port" => String? }?
  def match_named_names: (untyped text) -> ["host", "port"]?
  def regexp_receiver_named_capture: (untyped text) -> [String, [String?]]?
end
```

## Regexp literal in if condition

### update

```ruby
def check1
  if /foo/
    :then
  else
    :else
  end
end

def check2
  if /foo#{1}/
    :then
  else
    :else
  end
end
```

### result

```rbs
class Object
  def check1: -> :else | :then
  def check2: -> :else | :then
end
```
