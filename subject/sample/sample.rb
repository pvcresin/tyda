class Sample
  CONST = 1

  def foo
    [1, 2]
  end

  def bar
    foo.map do |x|
      x
    end
  end

  def baz = self.class::CONST
end
