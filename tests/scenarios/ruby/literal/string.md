# Ruby / Literal / String

## Single-quoted string

### update

```ruby
def single_quoted = 'hello'
```

### result

```rbs
class Object < BasicObject
  def single_quoted: -> "hello"
end
```

## Double-quoted string

### update

```ruby
def double_quoted = "hello"
```

### result

```rbs
class Object < BasicObject
  def double_quoted: -> "hello"
end
```

## %Q string

### update

```ruby
def percent_q = %Q(he said "hi")
```

### result

```rbs
class Object < BasicObject
  def percent_q: -> "he said \"hi\""
end
```

## %q string

### update

```ruby
def percent_lower_q = %q[hello world]
```

### result

```rbs
class Object < BasicObject
  def percent_lower_q: -> "hello world"
end
```

## % string notation

### update

```ruby
def percent_string = %(hello "world")
```

### result

```rbs
class Object < BasicObject
  def percent_string: -> "hello \"world\""
end
```

## String interpolation

### update

```ruby
def strings
  a = 'hello'
  b = "world"
  c = "#{a} #{b}"
  c
end
```

### result

```rbs
class Object < BasicObject
  def strings: -> "hello world"
end
```

## Adjacent string literals

### update

```ruby
def adjacent_strings = "foo" "bar"
```

### result

```rbs
class Object < BasicObject
  def adjacent_strings: -> "foobar"
end
```

## Character literal

### update

```ruby
def character_comma = ?,
def character_space = ?\s
def character_newline = ?\n
def character_multibyte = ?😀
```

### result

```rbs
class Object < BasicObject
  def character_comma: -> ","
  def character_space: -> " "
  def character_newline: -> "\n"
  def character_multibyte: -> "😀"
end
```

## Unary string prefix keeps literal value

### update

```ruby
def frozen_prefix = -"name"
def mutable_prefix = +"name"
```

### result

```rbs
class Object < BasicObject
  def frozen_prefix: -> "name"
  def mutable_prefix: -> "name"
end
```

## String repeat accepts literal float count

### update

```ruby
def repeated = -"x" * +2.9
```

### result

```rbs
class Object < BasicObject
  def repeated: -> "xx"
end
```

## String percent formats literal values

### update

```ruby
def format_char = "%c" % 44
def format_string = "%s" % "ok"
def format_percent = "%%" % []
def format_inspect = "_=%p;puts _%%_" % "_=%p;puts _%%_"
```

### result

```rbs
class Object < BasicObject
  def format_char: -> ","
  def format_string: -> "ok"
  def format_percent: -> "%"
  def format_inspect: -> "_=\"_=%p;puts _%%_\";puts _%_"
end
```

## String self representation helpers

### update

```ruby
def inspected_source = "_=%p;puts _%%_".inspect
def dumped_line = "a\nb".dump
def hex_number = "616263".hex
```

### result

```rbs
class Object < BasicObject
  def inspected_source: -> "\"_=%p;puts _%%_\""
  def dumped_line: -> "\"a\\nb\""
  def hex_number: -> 6382179
end
```

## String index accepts literal float positions

### update

```ruby
def float_index = "abcd"[1.9]
def float_range = "abcd"[1.9..2.1]
def float_byteslice = "abcd".byteslice(1.9, 2.1)
def float_getbyte = "abcd".getbyte(1.9)
```

### result

```rbs
class Object < BasicObject
  def float_index: -> "b"
  def float_range: -> "bc"
  def float_byteslice: -> "bc"
  def float_getbyte: -> 98
end
```

## String interpolation expands literal union

### update

```ruby
def interpolated_integer_union(cond)
  a = cond ? 1 : 2
  "v#{a}"
end
```

### result

```rbs
class Object < BasicObject
  def interpolated_integer_union: (untyped cond) -> ("v1" | "v2")
end
```

## String interpolation expands integer literal union from method condition

### update

```ruby
def interpolated_integer_from_comparison
  value = rand(100) > 50 ? 1 : 2
  "#{value}"
end
```

### result

```rbs
class Object < BasicObject
  def interpolated_integer_from_comparison: -> "1" | "2"
end
```

## Conditional inside string interpolation expands to integer literal union

### update

```ruby
def interpolated_direct_integer_comparison
  "#{rand(100) > 50 ? 1 : 2}"
end
```

### result

```rbs
class Object < BasicObject
  def interpolated_direct_integer_comparison: -> "1" | "2"
end
```

## String interpolation maps nil bool and symbol literals through to_s

### update

```ruby
def interpolated_mixed_literal(cond)
  a = cond ? nil : :ok
  b = cond ? true : false
  "#{a}:#{b}"
end
```

### result

```rbs
class Object < BasicObject
  def interpolated_mixed_literal: (untyped cond) -> (":false" | ":true" | "ok:false" | "ok:true")
end
```

## Symbol interpolation expands literal union

### update

```ruby
def interpolated_symbol_union(cond)
  a = cond ? 1 : :ok
  :"key_#{a}"
end
```

### result

```rbs
class Object < BasicObject
  def interpolated_symbol_union: (untyped cond) -> (:key_1 | :key_ok)
end
```

## Symbol interpolation falls back to base type when bare name is invalid

### update

```ruby
def interpolated_symbol_non_bare(cond)
  a = cond ? "" : "a-b"
  :"#{a}"
end
```

### result

```rbs
class Object < BasicObject
  def interpolated_symbol_non_bare: (untyped cond) -> Symbol
end
```

## Complex string interpolation

### update

```ruby
def interp_complex
  x = 42
  "value is #{x + 1}"
end
```

### result

```rbs
class Object < BasicObject
  def interp_complex: -> String
end
```

## String interpolation falls back when product is too large

### update

```ruby
def interpolation_product_cap(a, b)
  u = if a
    1
  elsif b
    2
  elsif a || b
    3
  else
    4
  end
  "#{u}#{u}#{u}#{u}"
end
```

### result

```rbs
class Object < BasicObject
  def interpolation_product_cap: (untyped a, untyped b) -> String
end
```

## Heredoc

### update

```ruby
def heredoc_test
  text = <<~HEREDOC
    Hello
    World
  HEREDOC
  text
end
```

### result

```rbs
class Object < BasicObject
  def heredoc_test: -> String
end
```

## Heredoc literal escapes newlines in RBS

### update

```ruby
def foo = <<'TEXT'
hello
world
TEXT
```

### result

```rbs
class Object < BasicObject
  def foo: -> "hello\nworld\n"
end
```

## Heredoc expression call

### update

```ruby
def heredoc_repeat = <<'TEXT' * 2
a
TEXT

def heredoc_numeric_label = <<'2'.chomp
value
2
```

### result

```rbs
class Object < BasicObject
  def heredoc_repeat: -> "a\na\n"
  def heredoc_numeric_label: -> String
end
```

## Backtick command string

### update

```ruby
def command_output = `echo hi`
```

### result

```rbs
class Object < BasicObject
  def command_output: -> String
end
```

## Literal string binary helpers

### update

```ruby
def delete_prefix_value = "path/name".delete_prefix("path/")
def delete_suffix_value = "name.rb".delete_suffix(".rb")
def starts_with_value = "name.rb".start_with?("name")
def ends_with_value = "name.rb".end_with?(".txt")
def includes_value = "name.rb".include?(".")
def split_value = "a,b,c".split(",")
def split_trailing = "a,b,".split(",")
def split_chars = "ab".split("")
```

### result

```rbs
class Object < BasicObject
  def delete_prefix_value: -> "name"
  def delete_suffix_value: -> "name"
  def starts_with_value: -> true
  def ends_with_value: -> false
  def includes_value: -> true
  def split_value: -> ["a", "b", "c"]
  def split_trailing: -> ["a", "b"]
  def split_chars: -> ["a", "b"]
end
```

## Literal string size helpers

### update

```ruby
def character_length = "\u{3042}".length
def byte_length = "\u{3042}".bytesize
```

### result

```rbs
class Object < BasicObject
  def character_length: -> 1
  def byte_length: -> 3
end
```

## Literal string symbol conversion

### update

```ruby
def string_to_symbol = "name".to_sym
def intern_symbol = "ready!".intern
def ivar_symbol = "@value".to_sym
def dynamic_symbol = "not ready".to_sym
```

### result

```rbs
class Object < BasicObject
  def string_to_symbol: -> :name
  def intern_symbol: -> :ready!
  def ivar_symbol: -> :@value
  def dynamic_symbol: -> Symbol
end
```

## Literal string partition helpers

### update

```ruby
def partition_value = "key=value".partition("=")
def partition_missing = "key".partition("=")
def rpartition_value = "key=value=tail".rpartition("=")
def rpartition_missing = "key".rpartition("=")
```

### result

```rbs
class Object < BasicObject
  def partition_value: -> ["key", "=", "value"]
  def partition_missing: -> ["key", "", ""]
  def rpartition_value: -> ["key=value", "=", "tail"]
  def rpartition_missing: -> ["", "", "key"]
end
```

## Literal string sequence helpers

### update

```ruby
def line_values = "name".lines
def char_values = "ab".chars
def byte_values = "AZ".bytes
def chomped_line_values = "name\ncount\n".lines(chomp: true)
def separated_line_values = "name--count--flag".lines("--", chomp: true)
def separated_line_keep_values = "name--count".lines("--")

def line_enum_hash
  "name".each_line.with_index.to_h
end

def line_enum_chomp_hash
  "name\ncount\n".each_line(chomp: true).with_index.to_h
end

def line_enum_separator_hash
  "name--count--flag".each_line("--", chomp: true).with_index.to_h
end

def char_enum_hash
  "ab".each_char.with_index.to_h
end

def byte_enum_hash
  "AZ".each_byte.with_index.to_h
end

def binary_line_enum_hash
  "name".b.each_line.with_index.to_h
end

def line_block_values
  values = []
  "name".each_line { |line| values << line }
  values
end

def line_block_chomp_values
  values = []
  "name\ncount\n".each_line(chomp: true) { |line| values << line }
  values
end

def line_block_separator_values
  values = []
  "name--count--flag".each_line("--", chomp: true) { |line| values << line }
  values
end

def byte_block_values
  values = []
  "AZ".each_byte { |byte| values << byte }
  values
end
```

### result

```rbs
class Object < BasicObject
  def line_values: -> ["name"]
  def char_values: -> ["a", "b"]
  def byte_values: -> [65, 90]
  def chomped_line_values: -> ["name", "count"]
  def separated_line_values: -> ["name", "count", "flag"]
  def separated_line_keep_values: -> ["name--", "count"]
  def line_enum_hash: -> Hash["name", Integer]
  def line_enum_chomp_hash: -> Hash["count" | "name", Integer]
  def line_enum_separator_hash: -> Hash["count" | "flag" | "name", Integer]
  def char_enum_hash: -> Hash["a" | "b", Integer]
  def byte_enum_hash: -> Hash[65 | 90, Integer]
  def binary_line_enum_hash: -> Hash["name", Integer]
  def line_block_values: -> Array["name"]
  def line_block_chomp_values: -> Array["count" | "name"]
  def line_block_separator_values: -> Array["count" | "flag" | "name"]
  def byte_block_values: -> Array[65 | 90]
end
```

## Literal string slice helpers

### update

```ruby
def first_char = "name"[0]
def last_char = "name"[-1]
def middle_chars = "name"[1, 2]
def range_chars = "name"[1..2]
def exclusive_chars = "name"[1...-1]
def slice_chars = "name".slice(1, 2)
def pattern_chars = "name"["am"]
def missing_pattern = "name"["zz"]
def missing_char = "name"[9]
def empty_range = "name"[4..3]
def missing_range = "name"[5..6]
def byte_prefix = "name".byteslice(0, 2)
def byte_suffix = "name".byteslice(-2, 2)
def byte_range = "name".byteslice(1..2)
def empty_byte_slice = "name".byteslice(4, 1)
def missing_byte_slice = "name".byteslice(5, 0)
def first_byte = "name".getbyte(0)
def last_byte = "name".getbyte(-1)
def missing_byte = "name".getbyte(9)
def first_codepoint = "A".ord
def first_character = "name".chr
```

### result

```rbs
class Object < BasicObject
  def first_char: -> "n"
  def last_char: -> "e"
  def middle_chars: -> "am"
  def range_chars: -> "am"
  def exclusive_chars: -> "am"
  def slice_chars: -> "am"
  def pattern_chars: -> "am"
  def missing_pattern: -> nil
  def missing_char: -> nil
  def empty_range: -> ""
  def missing_range: -> nil
  def byte_prefix: -> "na"
  def byte_suffix: -> "me"
  def byte_range: -> "am"
  def empty_byte_slice: -> ""
  def missing_byte_slice: -> nil
  def first_byte: -> 110
  def last_byte: -> 101
  def missing_byte: -> nil
  def first_codepoint: -> 65
  def first_character: -> "n"
end
```

## %x command string

### update

```ruby
def percent_x_output = %x[echo hi]
```

### result

```rbs
class Object < BasicObject
  def percent_x_output: -> String
end
```

## %w word array

### update

```ruby
def word_array = %w[foo bar baz]
def escaped_word_join = %w[one two\ three].join
def star_join = %w[a b] * ""
def dashed_join = %w[a b] * "-"
```

### result

```rbs
class Object < BasicObject
  def word_array: -> ["foo", "bar", "baz"]
  def escaped_word_join: -> "onetwo three"
  def star_join: -> "ab"
  def dashed_join: -> "a-b"
end
```

## %w word array stays exact after freeze

### update

```ruby
def frozen_word_array = %w[foo bar baz].freeze
```

### result

```rbs
class Object < BasicObject
  def frozen_word_array: -> ["foo", "bar", "baz"]
end
```

## %W word array widens only interpolated items

### update

```ruby
def interpolated_word_array(host) = %W[https #{host}].freeze
```

### result

```rbs
class Object < BasicObject
  def interpolated_word_array: (untyped host) -> ["https", String]
end
```

## %w assigned to constant stays exact in return

### update

```ruby
class A
  CONST = %w[https].freeze

  def foo = CONST
end
```

### result

```rbs
class A
  CONST: ["https"]

  def foo: -> ["https"]
end
```

## %i symbol array

### update

```ruby
def symbol_array = %i[foo bar baz]
```

### result

```rbs
class Object < BasicObject
  def symbol_array: -> [:foo, :bar, :baz]
end
```

## %s symbol

### update

```ruby
def percent_s = %s[hello_world]
```

### result

```rbs
class Object < BasicObject
  def percent_s: -> :hello_world
end
```

## %I symbol array widens only interpolated items

### update

```ruby
def interpolated_symbol_array(host) = %I[foo #{host}]
```

### result

```rbs
class Object < BasicObject
  def interpolated_symbol_array: (untyped host) -> [:foo, Symbol]
end
```

## String#unpack1 narrows static template result

### update

```ruby
def hex_payload = File.binread("data.bin").unpack1("H*")
def byte_code = "a".unpack1("C")
def network_size = "abcd".unpack1("N")
def double_value = "abcdefgh".unpack1("G")
def skipped_byte = "ab".unpack1("xC")
def no_value = "a".unpack1("x")

def union_template(flag)
  template = flag ? "H*" : "C"
  "a".unpack1(template)
end
```

### result

```rbs
class Object < BasicObject
  def hex_payload: -> String
  def byte_code: -> Integer?
  def network_size: -> Integer?
  def double_value: -> Float?
  def skipped_byte: -> Integer?
  def no_value: -> nil
  def union_template: (untyped flag) -> (Integer | String)?
end
```

## String#unpack narrows static template values

### update

```ruby
def byte_values = File.binread("data.bin").unpack("C*")
def fixed_header = File.binread("data.bin").unpack("nnN")
def text_payload = File.binread("data.bin").unpack("H32")[0]
def decoded_payload = File.binread("data.bin").unpack("m").join
def skipped_value = "ab".unpack("@1C")
def no_values = "ab".unpack("x")
def two_bytes = "ab".unpack("C2")
def large_bytes = File.binread("data.bin").unpack("C64")

def union_values(flag)
  template = flag ? "H*" : "C*"
  File.binread("data.bin").unpack(template)
end
```

### result

```rbs
class Object < BasicObject
  def byte_values: -> Array[Integer]
  def fixed_header: -> [Integer?, Integer?, Integer?]
  def text_payload: -> String
  def decoded_payload: -> String
  def skipped_value: -> [Integer?]
  def no_values: -> [ ]
  def two_bytes: -> [Integer?, Integer?]
  def large_bytes: -> Array[Integer?]
  def union_values: (untyped flag) -> Array[Integer | String]
end
```

## Numeric code string round trip helpers

### update

```ruby
def encoded_integer = "abc".unpack("H*")[0].hex
def decoded_string = ["616263"].pack("H*")
def hex_template_value = "abc".unpack1("H*").hex
```

### result

```rbs
class Object < BasicObject
  def encoded_integer: -> Integer
  def decoded_string: -> String
  def hex_template_value: -> Integer
end
```

## Frozen string literal pragma

### update

```ruby
# frozen_string_literal: true

def frozen_test
  str = "hello"
  str
end
```

### result

```rbs
class Object < BasicObject
  def frozen_test: -> "hello"
end
```

## Mutating a string widens its literal type

### update

```ruby
class Builder
  def build
    s = ""
    s << "hello"
    s << "world"
    s
  end

  def with_concat
    s = +""
    s.concat("a", "b")
    s
  end
end
```

### result

```rbs
class Builder
  def build: -> String
  def with_concat: -> String
end
```

## gsub with a hash replacement returns a string

### update

```ruby
class Replacer
  def implicit_hash = "abc".gsub(/[ab]/, "a" => "X", "b" => "Y")
  def braced_hash   = "abc".gsub(/[ab]/, { "a" => "X" })
  def sub_hash      = "abc".sub(/a/, "a" => "X")
  def no_replacement = "abc".gsub(/[ab]/)
end
```

### result

```rbs
class Replacer
  def implicit_hash: -> String
  def braced_hash: -> String
  def sub_hash: -> String
  def no_replacement: -> Enumerator[String, String]
end
```

## partition of a literal string keeps a three-element tuple

### update

```ruby
def parts = "a-b-c".partition("-")
```

### result

```rbs
class Object < BasicObject
  def parts: -> ["a", "-", "b-c"]
end
```
