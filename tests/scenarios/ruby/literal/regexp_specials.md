# Ruby / Literal / Regexp Specials

## Regexp backreferences and named captures

### update

```ruby
def numbered_ref(value)
  /(\w+)/ =~ value
  $1
end

def whole_match_ref(value)
  /(\w+)/ =~ value
  $&
end

def last_parenthesized_ref(value)
  /(\w+)/ =~ value
  $+
end

def second_numbered_ref(value)
  /(\w+):(\d+)/ =~ value
  $2
end

def missing_numbered_ref(value)
  /(\w+)/ =~ value
  $2
end

def pre_match_ref(value)
  /:/ =~ value
  $`
end

def post_match_ref(value)
  /:/ =~ value
  $'
end

def named_capture(value)
  /(?<word>\w+)/ =~ value
  word
end
```

### result

```rbs
class Object < BasicObject
  def numbered_ref: (untyped value) -> String?
  def whole_match_ref: (untyped value) -> String?
  def last_parenthesized_ref: (untyped value) -> String?
  def second_numbered_ref: (untyped value) -> String?
  def missing_numbered_ref: (untyped value) -> String?
  def pre_match_ref: (untyped value) -> String?
  def post_match_ref: (untyped value) -> String?
  def named_capture: (untyped value) -> String?
end
```

## Regexp.last_match captures

### update

```ruby
def last_match_index(value)
  /(\w+):(?<count>\d+)/ =~ value
  Regexp.last_match(1)
end

def last_match_zero(value)
  /(\w+):(?<count>\d+)/ =~ value
  Regexp.last_match(0)
end

def last_match_symbol(value)
  /(\w+):(?<count>\d+)/ =~ value
  Regexp.last_match(:count)
end

def last_match_string(value)
  /(\w+):(?<count>\d+)/ =~ value
  Regexp.last_match("count")
end

def last_match_dynamic(value, key)
  /(\w+):(?<count>\d+)/ =~ value
  Regexp.last_match(key)
end

def last_match_object(value)
  /(\w+):(?<count>\d+)/ =~ value
  Regexp.last_match
end

def last_match_capture_list(value)
  /(\w+):(?<count>\d+)/ =~ value
  Regexp.last_match&.captures
end
```

### result

```rbs
class Object < BasicObject
  def last_match_index: (untyped value) -> String?
  def last_match_zero: (untyped value) -> String?
  def last_match_symbol: (untyped value) -> String?
  def last_match_string: (untyped value) -> String?
  def last_match_dynamic: (untyped value, untyped key) -> String?
  def last_match_object: (untyped value) -> MatchData?
  def last_match_capture_list: (untyped value) -> Array[String?]?
end
```

## Match-last-line and flip-flop predicates do not erase branch types

### update

```ruby
def match_last_line_branch
  if /foo/
    1
  else
    "no"
  end
end

def flip_flop_branch(x)
  if x == 1 .. x == 3
    "inside"
  else
    "outside"
  end
end
```

### result

```rbs
class Object < BasicObject
  def match_last_line_branch: -> 1 | "no"
  def flip_flop_branch: (untyped x) -> ("inside" | "outside")
end
```

## case when regexp branch reads a numbered capture

### update

```ruby
def case_capture(s)
  case s
  when /a(b)c/
    $1
  end
end
```

### result

```rbs
class Object < BasicObject
  def case_capture: (untyped s) -> String?
end
```

## match? does not populate regexp globals

### update

```ruby
def probe(s)
  s.match?(/(z)/)
  $1
end
```

### result

```rbs
class Object < BasicObject
  def probe: (untyped s) -> untyped
end
```
