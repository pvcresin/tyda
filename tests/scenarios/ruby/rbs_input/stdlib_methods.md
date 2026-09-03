# Ruby / RBS Input / Stdlib Methods

## String#to_i

### update

```ruby
def test = "42".to_i
```

### result

```rbs
class Object < BasicObject
  def test: -> Integer
end
```

## String#to_f

### update

```ruby
def test = "3.14".to_f
```

### result

```rbs
class Object < BasicObject
  def test: -> Float
end
```

## String#length

### update

```ruby
def test = "hello".length
```

### result

```rbs
class Object < BasicObject
  def test: -> 5
end
```

## String#upcase

### update

```ruby
def test = "hello".upcase
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## String#downcase

### update

```ruby
def test = "HELLO".downcase
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## String#strip

### update

```ruby
def test = "  hello  ".strip
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## String#reverse

### update

```ruby
def test = "hello".reverse
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## String#chomp

### update

```ruby
def test = "hello\n".chomp
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## String#chop

### update

```ruby
def test = "hello".chop
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## String#split

### update

```ruby
def test = "a,b,c".split
```

### result

```rbs
class Object < BasicObject
  def test: -> Array[String]
end
```

## String#chars

### update

```ruby
def test = "hello".chars
```

### result

```rbs
class Object < BasicObject
  def test: -> ["h", "e", "l", "l", "o"]
end
```

## Integer#to_s

### update

```ruby
def test = 42.to_s
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## Integer#to_f

### update

```ruby
def test = 42.to_f
```

### result

```rbs
class Object < BasicObject
  def test: -> Float
end
```

## Integer#abs

### update

```ruby
def test
  n = -5
  n.abs
end
```

### result

```rbs
class Object < BasicObject
  def test: -> Integer
end
```

## Integer#zero?

### update

```ruby
def test = 0.zero?
```

### result

```rbs
class Object < BasicObject
  def test: -> bool
end
```

## Float#to_i

### update

```ruby
def test = 3.14.to_i
```

### result

```rbs
class Object < BasicObject
  def test: -> Integer
end
```

## Float#to_s

### update

```ruby
def test = 3.14.to_s
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## Float#round

### update

```ruby
def test = 3.14.round
```

### result

```rbs
class Object < BasicObject
  def test: -> Integer
end
```

## Float#ceil

### update

```ruby
def test = 3.14.ceil
```

### result

```rbs
class Object < BasicObject
  def test: -> Integer
end
```

## Float#floor

### update

```ruby
def test = 3.14.floor
```

### result

```rbs
class Object < BasicObject
  def test: -> Integer
end
```

## Float#abs

### update

```ruby
def test
  f = -3.14
  f.abs
end
```

### result

```rbs
class Object < BasicObject
  def test: -> Float
end
```

## Array#length

### update

```ruby
def test = [1, 2, 3].length
```

### result

```rbs
class Object < BasicObject
  def test: -> 3
end
```

## Array#empty?

### update

```ruby
def test = [].empty?
```

### result

```rbs
class Object < BasicObject
  def test: -> true
end
```

## Array#reverse

### update

```ruby
def test = [1, 2, 3].reverse
```

### result

```rbs
class Object < BasicObject
  def test: -> Array[1 | 2 | 3]
end
```

## Array#count

### update

```ruby
def test = [1, 2, 3].count
```

### result

```rbs
class Object < BasicObject
  def test: -> 3
end
```

## Array#compact

### update

```ruby
def test = [1, nil, 3].compact
```

### result

```rbs
class Object < BasicObject
  def test: -> [1, 3]
end
```

## Array#flatten

### update

```ruby
def test = [[1, 2], [3]].flatten
```

### result

```rbs
class Object < BasicObject
  def test: -> Array[1 | 2 | 3]
end
```

## Hash#keys

### update

```ruby
def test = { a: 1, b: 2 }.keys
```

### result

```rbs
class Object < BasicObject
  def test: -> Array[:a | :b]
end
```

## Hash#values

### update

```ruby
def test = { a: 1, b: 2 }.values
```

### result

```rbs
class Object < BasicObject
  def test: -> Array[1 | 2]
end
```

## Hash#empty?

### update

```ruby
def test = {}.empty?
```

### result

```rbs
class Object < BasicObject
  def test: -> bool
end
```

## Hash#length

### update

```ruby
def test = { a: 1 }.length
```

### result

```rbs
class Object < BasicObject
  def test: -> 1
end
```

## Method chain String to Integer to String

### update

```ruby
def test = "42".to_i.to_s
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## Method chain String to Integer

### update

```ruby
def test = "hello".upcase.length
```

### result

```rbs
class Object < BasicObject
  def test: -> Integer
end
```

## Method chain Integer to Float to Integer

### update

```ruby
def test = 42.to_f.to_i
```

### result

```rbs
class Object < BasicObject
  def test: -> Integer
end
```

## Method chain String to String

### update

```ruby
def test = "hello".reverse.upcase
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## Call stdlib method through local variable

### update

```ruby
def test
  s = "hello"
  s.upcase
end
```

### result

```rbs
class Object < BasicObject
  def test: -> String
end
```

## Method chain through local variable

### update

```ruby
def test
  s = "42"
  n = s.to_i
  n.to_f
end
```

### result

```rbs
class Object < BasicObject
  def test: -> Float
end
```

## Record#keys keeps string keys

### update

```ruby
def test = { "a" => 1, "b" => 2 }.keys
```

### result

```rbs
class Object < BasicObject
  def test: -> Array["a" | "b"]
end
```

## Record#values returns union of value types

### update

```ruby
def test = { foo: "x", bar: "y" }.values
```

### result

```rbs
class Object < BasicObject
  def test: -> Array["x" | "y"]
end
```

## File.open instance type resolves as File

### update

```ruby
def test = File.open("a", "r", &@blk)
```

### result

```rbs
class Object < BasicObject
  def test: -> File
end
```

## RBS return constrains static stdlib refinements

### update

```rbs
class Array
  def join: (?String separator) -> Integer
end

class String
  def unpack: (String format) -> Symbol
  def unpack1: (String format) -> Symbol
  def scan: (Regexp pattern) -> Symbol
          | (Regexp pattern) { (String match) -> void } -> Symbol
  def each_line: () { (String line) -> void } -> Symbol
  def match: (Regexp pattern) -> Integer
end

class Integer
  def step: (Integer limit) { (Integer value) -> void } -> Symbol
end
```

```ruby
def joined_value = ["a", "b"].join("/")
def unpacked_values = "a".unpack("C")
def unpacked_value = "a".unpack1("C")
def scanned_value = "ab".scan(/a/)
def scanned_block_value = "ab".scan(/a/) { |match| match }
def line_block_value = "a\n".each_line { |line| line }
def matched_value = "ab".match(/(a)/)
def stepped_block_value = 1.step(3) { |value| value }
```

### result

```rbs
class Object < BasicObject
  def joined_value: -> Integer
  def unpacked_values: -> Symbol
  def unpacked_value: -> Symbol
  def scanned_value: -> Symbol
  def scanned_block_value: -> Symbol
  def line_block_value: -> Symbol
  def matched_value: -> Integer
  def stepped_block_value: -> Symbol
end
```

## RBS return constrains static Array refinements

### update

```rbs
class Array
  def first: -> Symbol
  def take: (Integer count) -> Symbol
  def to_h: -> Symbol
  def product: (*Array[untyped] others) -> Symbol
  def combination: (Integer count) { (Array[untyped] tuple) -> void } -> Symbol
  def sort!: () { (untyped left, untyped right) -> Integer } -> Symbol
  def grep: (untyped pattern) -> Symbol
  def union: (*Array[untyped] others) -> Symbol
  def transpose: -> Symbol
  def zip: (Array[untyped] other, *Array[untyped] others) -> Symbol
  def compact: -> Symbol
  def flatten: (?Integer level) -> Symbol
end
```

```ruby
def first_value = [1, 2].first
def taken_values = [1, 2].take(1)
def pair_hash = [[:a, 1]].to_h
def product_values = [1].product(["a"])
def combination_block_values = [1, 2].combination(1) { |tuple| tuple }
def sorted_block_values
  values = [2, 1]
  values.sort! { |left, right| left <=> right }
end
def grep_values = [1, "a"].grep(String)
def union_values = [1].union([2])
def transposed_values = [[1, 2], [3, 4]].transpose
def zipped_values = [1].zip([2])
def compacted_values = [1, nil].compact
def flattened_values = [[1], [2]].flatten
```

### result

```rbs
class Object < BasicObject
  def first_value: -> Symbol
  def taken_values: -> Symbol
  def pair_hash: -> Symbol
  def product_values: -> Symbol
  def combination_block_values: -> Symbol
  def sorted_block_values: -> Symbol
  def grep_values: -> Symbol
  def union_values: -> Symbol
  def transposed_values: -> Symbol
  def zipped_values: -> Symbol
  def compacted_values: -> Symbol
  def flattened_values: -> Symbol
end
```

## RBS return constrains literal String refinements

### update

```rbs
class String
  def []: (Integer index) -> Symbol
end
```

```ruby
def indexed_value = "abc"[0]
```

### result

```rbs
class Object < BasicObject
  def indexed_value: -> Symbol
end
```

## RBS return constrains static collection helpers

### update

```rbs
class Array
  def []: (Integer index) -> Symbol
  def sample: -> Symbol
  def include?: (untyped item) -> Symbol
  def size: -> Symbol
  def assoc: (untyped item) -> Symbol
end

class Hash
  def []: (Symbol key) -> Symbol
  def key?: (Symbol key) -> Symbol
end

module Enumerable
  def chain: (*untyped sources) -> Symbol
  def to_set: -> Symbol
end
```

```ruby
def array_item = [1, 2][0]
def sampled_item = [1, 2].sample
def included_item = [1, 2].include?(1)
def array_size = [1, 2].size
def assoc_item = [[:a, 1]].assoc(:a)
def chain_values = [1].chain([2])
def set_values = [1, 2].to_set
def hash_item = { a: 1 }[:a]
def hash_key = { a: 1 }.key?(:a)
```

### result

```rbs
class Object < BasicObject
  def array_item: -> Symbol
  def sampled_item: -> Symbol
  def included_item: -> Symbol
  def array_size: -> Symbol
  def assoc_item: -> Symbol
  def chain_values: -> Symbol
  def set_values: -> Symbol
  def hash_item: -> Symbol
  def hash_key: -> Symbol
end
```

## RBS return constrains regexp helper refinements

### update

```rbs
class Regexp
  def self.last_match: -> Symbol
end

class MatchData
  def captures: -> Symbol
  def named_captures: -> Symbol
  def names: -> Symbol
  def to_a: -> Symbol
  def values_at: (*untyped keys) -> Symbol
  def []: (untyped key) -> Symbol
end
```

```ruby
def last_match_value
  /(?<name>a)/.match("a")
  Regexp.last_match
end

def capture_values = /(?<name>a)/.match("a")&.captures
def named_capture_values = /(?<name>a)/.match("a")&.named_captures
def capture_names = /(?<name>a)/.match("a")&.names
def capture_array = /(?<name>a)/.match("a")&.to_a
def selected_captures = /(?<name>a)/.match("a")&.values_at(:name)
def indexed_capture = /(?<name>a)/.match("a")&.[](:name)
```

### result

```rbs
class Object < BasicObject
  def last_match_value: -> Symbol
  def capture_values: -> Symbol?
  def named_capture_values: -> Symbol?
  def capture_names: -> Symbol?
  def capture_array: -> Symbol?
  def selected_captures: -> Symbol?
  def indexed_capture: -> Symbol?
end
```

## RBS return constrains static Hash refinements

### update

```rbs
class Hash
  def fetch: (Symbol key) -> Symbol
  def fetch_values: (*Symbol keys) -> Symbol
  def dig: (*untyped keys) -> Symbol
  def slice: (*Symbol keys) -> Symbol
  def except: (*Symbol keys) -> Symbol
  def values_at: (*Symbol keys) -> Symbol
  def key: (untyped value) -> Integer
  def compact: -> Symbol
  def flatten: -> Symbol
end

module Enumerable
  def entries: -> Symbol
  def sort: -> Symbol
  def tally: -> Symbol
  def min: -> Symbol
  def max: -> Symbol
  def minmax: -> Symbol
end
```

```ruby
def fetched_value = { a: 1 }.fetch(:a)
def fetched_values = { a: 1, b: 2 }.fetch_values(:a, :b)
def dug_value = { a: { b: 1 } }.dig(:a, :b)
def sliced_hash = { a: 1, b: 2 }.slice(:a)
def excepted_hash = { a: 1, b: 2 }.except(:b)
def values_at_hash = { a: 1, b: 2 }.values_at(:a, :missing)
def key_value = { a: 1 }.key(1)
def compacted_hash = { a: 1, b: nil }.compact
def flattened_hash = { a: 1 }.flatten
def hash_entries = { a: 1 }.entries
def hash_sort = { a: 1 }.sort
def hash_tally = { a: 1 }.tally
def hash_min = { a: 1 }.min
def hash_max = { a: 1 }.max
def hash_minmax = { a: 1 }.minmax
```

### result

```rbs
class Object < BasicObject
  def fetched_value: -> Symbol
  def fetched_values: -> Symbol
  def dug_value: -> Symbol
  def sliced_hash: -> Symbol
  def excepted_hash: -> Symbol
  def values_at_hash: -> Symbol
  def key_value: -> Integer
  def compacted_hash: -> Symbol
  def flattened_hash: -> Symbol
  def hash_entries: -> Symbol
  def hash_sort: -> Symbol
  def hash_tally: -> Symbol
  def hash_min: -> Symbol
  def hash_max: -> Symbol
  def hash_minmax: -> Symbol
end
```

## Resolve stdlib extension files

### update

```rbs
class CGI
  def self.escapeHTML: (String str) -> String
end

class Dir
  def self.tmpdir: -> String
end

class Time
  def self.now: -> Time
  def httpdate: -> String
end

module URI
  def self.decode_www_form_component: (String str) -> String
end

module Zlib
  class Deflate
    def self.deflate: (String string) -> String
  end
end
```

```ruby
require "cgi/escape"
require "time"
require "tmpdir"
require "uri"
require "zlib"

def cgi_html = CGI.escapeHTML("<a>")
def time_header = Time.now.httpdate
def tmpdir_path = Dir.tmpdir
def uri_component = URI.decode_www_form_component("a+b")
def zlib_data = Zlib::Deflate.deflate("a")
```

### result

```rbs
class Object < BasicObject
  def cgi_html: -> String
  def time_header: -> String
  def tmpdir_path: -> String
  def uri_component: -> String
  def zlib_data: -> String
end
```
