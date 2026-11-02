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
  def status_name: -> String
  def ready?: -> bool
  def machine_states: -> untyped
  def status_label: -> String
end
```
