# Ruby / Class / Prepend Order

## Prepended module method wins over class method

### update

```ruby
module M
  def hello = :module
end

class C
  prepend M

  def hello = :class
end

def f = C.new.hello
```

### result

```rbs
class C
  prepend M

  def hello: -> :class
end

module M
  def hello: -> :module
end

class Object < BasicObject
  def f: -> :module
end
```

## Use prepend class include order

### update

```ruby
module Pre
  def hello = :pre
end

module Inc
  def hello = :inc
end

class C
  prepend Pre
  include Inc

  def hello = :class
end

def f = C.new.hello
```

### result

```rbs
class C
  prepend Pre
  include Inc

  def hello: -> :class
end

module Inc
  def hello: -> :inc
end

class Object < BasicObject
  def f: -> :pre
end

module Pre
  def hello: -> :pre
end
```

## Prepended method is visible when class has no same-name method

### update

```ruby
module M
  def hello = :module
end

class C
  prepend M
end

def f = C.new.hello
```

### result

```rbs
class C
  prepend M
end

module M
  def hello: -> :module
end

class Object < BasicObject
  def f: -> :module
end
```

## Prepended date conversion overrides the built-in fallback

### update

```ruby
module DateConversion
  def to_date = :custom
end

class DateConversionReceiver
  prepend DateConversion
end

def converted_date = DateConversionReceiver.new.to_date
```

### result

```rbs
module DateConversion
  def to_date: -> :custom
end

class DateConversionReceiver
  prepend DateConversion
end

class Object < BasicObject
  def converted_date: -> :custom
end
```
