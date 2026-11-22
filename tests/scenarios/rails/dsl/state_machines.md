# Rails / DSL / State Machines

## Infer the state machine reader and generated state name

### update

```ruby
class StateMachineReader
  state_machine :status do
    state :ready
  end

  def machine_states = self.class.state_machine.states
  def status_label = status_name
end
```

### result

```rbs
class StateMachineReader
  def self.state_machine: -> untyped
  def status_name: -> Symbol
  def ready?: -> bool
  def machine_states: -> untyped
  def status_label: -> Symbol
end
```

## Generate state_machines name and event helpers

### update

```ruby
class Vehicle
  state_machine :status, initial: :parked do
    state :parked, :idling

    event :ignite do
      transition parked: :idling
    end
  end
end
```

### result

```rbs
class Vehicle
  def self.state_machine: -> untyped
  def status_name: -> Symbol
  def parked?: -> bool
  def idling?: -> bool
  def can_ignite?: -> bool
  def ignite_transition: -> StateMachines::Transition?
  def ignite: -> bool
  def ignite!: -> bool
end
```
