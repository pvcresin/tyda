## Narrow a right-hand condition assignment in the true branch

### update

```ruby
class IfAndAssignment
  def maybe_text(found)
    found ? "text" : nil
  end

  def assignment_on_right(guard, found)
    if guard && item = maybe_text(found)
      item.upcase
    else
      :no
    end
  end

  def skipped_assignment_when_left_is_false(guard, found)
    if guard && item = maybe_text(found)
      :matched
    else
      item
    end
  end

  def assignment_is_skipped_when_guard_is_false(found)
    value = :before
    guard = false
    if guard && value = maybe_text(found)
      :matched
    else
      value
    end
  end

  def reassignment_after_guard(guard, found)
    value = :before
    if guard && value = maybe_text(found)
      value.upcase
    else
      value
    end
  end

  def false_assignment_does_not_enter_body(guard)
    value = :before
    if guard && value = false
      :matched
    else
      value
    end
  end

  def nil_assignment_does_not_enter_body(guard)
    value = :before
    if guard && value = nil
      :matched
    else
      value
    end
  end

  def same_local_guard_and_assignment(value)
    if value && value = maybe_text(value)
      value.upcase
    else
      value
    end
  end

  def same_local_guard_and_assignment_examples
    same_local_guard_and_assignment(false)
    same_local_guard_and_assignment("old")
  end

  def same_local_guard_and_falsy_assignment(value)
    if value && value = nil
      :matched
    else
      value
    end
  end

  def same_local_guard_and_falsy_assignment_examples
    same_local_guard_and_falsy_assignment(false)
    same_local_guard_and_falsy_assignment("old")
  end
end
```

### result

```rbs
class IfAndAssignment
  def maybe_text: ((String | bool) found) -> "text"?
  def assignment_on_right: (untyped guard, untyped found) -> (String | :no)
  def skipped_assignment_when_left_is_false: (untyped guard, untyped found) -> :matched?
  def assignment_is_skipped_when_guard_is_false: (untyped found) -> :before
  def reassignment_after_guard: (untyped guard, untyped found) -> (String | :before)?
  def false_assignment_does_not_enter_body: (untyped guard) -> (false | :before)
  def nil_assignment_does_not_enter_body: (untyped guard) -> :before?
  def same_local_guard_and_assignment: ((String | bool) value) -> (String | false)?
  def same_local_guard_and_assignment_examples: -> (String | false)?
  def same_local_guard_and_falsy_assignment: ((String | bool) value) -> (false | nil)
  def same_local_guard_and_falsy_assignment_examples: -> (false | nil)
end
```
