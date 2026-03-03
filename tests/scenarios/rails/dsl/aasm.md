# Rails / DSL / AASM

## Generate predicate and event methods from state and event

### update

```ruby
class Order
  aasm do
    state :draft, initial: true
    state :paid

    event :pay do
    end
  end
end
```

### result

```rbs
class Order
  def draft?: -> bool
  def paid?: -> bool
  def may_pay?: -> bool
  def pay: -> bool
  def pay!: -> bool
end
```

## aasm column option does not change event or state method names

### update

```ruby
class Invoice
  aasm column: :payment_state do
    state :unpaid, initial: true
    state :paid

    event :charge do
    end
  end
end
```

### result

```rbs
class Invoice
  def unpaid?: -> bool
  def paid?: -> bool
  def may_charge?: -> bool
  def charge: -> bool
  def charge!: -> bool
end
```

## Class with multiple state machines

### update

```ruby
class Task
  aasm :status do
    state :pending, initial: true
    state :done

    event :complete do
    end
  end

  aasm :review_state do
    state :open
    state :approved

    event :approve do
    end
  end
end
```

### result

```rbs
class Task
  def pending?: -> bool
  def done?: -> bool
  def may_complete?: -> bool
  def complete: -> bool
  def complete!: -> bool
  def open?: -> bool
  def approved?: -> bool
  def may_approve?: -> bool
  def approve: -> bool
  def approve!: -> bool
end
```

## Infer event with transitions block

### update

```ruby
class Package
  aasm do
    state :new
    state :shipped
    state :delivered

    event :ship do
      transitions from: :new, to: :shipped
    end

    event :deliver do
      transitions from: :shipped, to: :delivered
    end
  end
end
```

### result

```rbs
class Package
  def new?: -> bool
  def shipped?: -> bool
  def delivered?: -> bool
  def may_ship?: -> bool
  def ship: -> bool
  def ship!: -> bool
  def may_deliver?: -> bool
  def deliver: -> bool
  def deliver!: -> bool
end
```
