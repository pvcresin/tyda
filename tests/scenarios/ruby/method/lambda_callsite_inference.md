# Ruby / Method / Lambda Callsite Inference

## Infer class body lambda args from call site

### update

```ruby
class Config
  env_int = lambda do |key, val|
    val
  end

  FOO = env_int.call("LIMIT_A", 4096)
  BAR = env_int.call("LIMIT_B", 8192)
end
```

### result

```rbs
class Config
  FOO: 4096
  BAR: 8192
end
```

## Infer class body proc args from call site

### update

```ruby
class Config
  say = proc do |name, count|
    count
  end

  ANSWER = say.call("hello", 42)
end
```

### result

```rbs
class Config
  ANSWER: 42
end
```

## Infer method body lambda args from call site

### update

```ruby
class Runner
  def run
    doubler = lambda do |n|
      n * 2
    end
    doubler.call(21)
  end
end
```

### result

```rbs
class Runner
  def run: -> Integer
end
```

## Infer class body lambda through `.()` call

### update

```ruby
class Config
  env_int = lambda do |k, v|
    v
  end
  LIMIT = env_int.("A", 123)
end
```

### result

```rbs
class Config
  LIMIT: 123
end
```

## Show call-site arg as block arg type in hover

### update

```ruby
class Config
  say_val = lambda do |key, val|
    val
  end
  OUT = say_val.call("A", 123)
end
```

### result

```rbs
class Config
  OUT: 123
end
```

## Resolve an RBS method inside a stored lambda

### update

```ruby
class DirectoryBody
  def initialize(files)
    @files = files
  end

  def each
    @files.call("entry")
  end
end

class Directory
  def list_directory
    DirectoryBody.new(->(path) do
      stat = stat(path)
      next unless stat

      filesize_format(stat.size)
    end)
  end

  def stat(path)
    File.stat(path)
  rescue SystemCallError
    nil
  end

  def filesize_format(size) = size
end

module Rack
  class Directory
    FILESIZE_FORMAT = [
      ["%.1fT", 1 << 40],
      ["%.1fG", 1 << 30],
      ["%.1fM", 1 << 20],
      ["%.1fK", 1 << 10],
    ]

    class DirectoryBody < Struct.new(:root, :path, :files)
      def each
        files.call("..") unless path.chomp("/") == root

        Dir.foreach(path) do |basename|
          next if basename.start_with?(".")
          next unless entry = files.call(basename)

          entry
        end
      end
    end

    def initialize(root, app = nil)
      @root = root
      @app = app
    end

    def list_directory(path_info, path, script_name)
      url_head = (script_name.split("/") + path_info.split("/")).map do |part|
        part
      end

      DirectoryBody.new(@root, path, ->(basename) do
        stat = stat(File.join(path, basename))
        next unless stat

        url = File.join(*url_head + [basename])
        mtime = stat.mtime
        if stat.directory?
          size = "-"
          type = "directory"
          url << "/"
        else
          type = File.extname(basename)
          size = filesize_format(stat.size)
        end

        [url, basename, size, type, mtime]
      end)
    end

    def stat(path)
      File.stat(path)
    rescue SystemCallError
      nil
    end

    def filesize_format(int)
      FILESIZE_FORMAT.each do |format, size|
        return format % (int.to_f / size) if int >= size
      end

      "#{int}B"
    end
  end
end
```

### result

```rbs
class Directory
  def list_directory: -> DirectoryBody
  def stat: (untyped path) -> File::Stat?
  def filesize_format: (Integer size) -> Integer
end

class DirectoryBody
  def initialize: (Proc files) -> void
  def each: -> untyped
end

class Rack::Directory
  FILESIZE_FORMAT: [["%.1fT", Integer], ["%.1fG", Integer], ["%.1fM", Integer], ["%.1fK", Integer]]

  def initialize: (untyped root, ?nil app) -> void
  def list_directory: (untyped path_info, untyped path, untyped script_name) -> Rack::Directory::DirectoryBody
  def stat: (String path) -> File::Stat?
  def filesize_format: (Integer int) -> String
end

class Rack::Directory::DirectoryBody
  def root: -> untyped
  def root=: (untyped root) -> untyped
  def path: -> untyped
  def path=: (untyped path) -> untyped
  def files: -> untyped
  def files=: (untyped files) -> untyped
  def initialize: (untyped root, untyped path, untyped files) -> void
  def self.members: -> Array[:files | :path | :root]
  def each: -> nil
end
```

## Do not infer method params from unbound lambda params

### update

```ruby
class Headers
  def [](key)
    key
  end

  def to_proc
    lambda { |x| self[x] }
  end

  def to_block
    proc { |x| self[x] }
  end
end
```

### result

```rbs
class Headers
  def []: (untyped key) -> untyped
  def to_proc: -> Proc
  def to_block: -> Proc
end
```
