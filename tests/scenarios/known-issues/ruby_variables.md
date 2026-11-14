# Known Issues / Ruby Variables

## A multiple assignment qualifies a constant defined later in the class

```ruby
module Outer
  class Apu
    def initialize
      @p0, @p1 = Pulse.new(self), Pulse.new(self)
      @single = Pulse.new(self)
      @mixer = Mixer.new(@p0, @single)
    end

    def p0 = @p0

    class Pulse
      def initialize(x) = @x = x
    end

    class Mixer
      def initialize(a, b)
        @a = a
        @b = b
      end
    end
  end
end
```

### result

```rbs
class Outer::Apu
  def initialize: -> void
  def p0: -> Outer::Apu::Pulse
end

class Outer::Apu::Mixer
  def initialize: (Outer::Apu::Pulse a, Outer::Apu::Pulse b) -> void
end

class Outer::Apu::Pulse
  def initialize: (Outer::Apu x) -> void
end
```
