# Rails / DSL / Config

## Infer setting accessor type from default

### update

```ruby
class Config::Options
end

class Settings < Config::Options
  setting :timeout, default: 30
  setting :features, default: { search: true, retries: 2 }
  setting :hosts, default: ["app", "worker"]
end
```

### result

```rbs
class Settings < Config::Options
  def timeout: -> Integer
  def timeout=: (Integer timeout) -> Integer
  def features: -> { "search" => bool, "retries" => Integer }
  def features=: ({ "search" => bool, "retries" => Integer } features) -> { "search" => bool, "retries" => Integer }
  def hosts: -> Array[String]
  def hosts=: (Array[String] hosts) -> Array[String]
end
```
