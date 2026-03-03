# Rails / Schema / Dynamic Attr From Column Names

## `attr_accessor(*Model.column_names - [...])` defines column accessors

### update

`db/structure.sql`

```sql
CREATE TABLE public.cards (
    id bigint NOT NULL,
    card_id bigint NOT NULL,
    owner_id bigint NOT NULL
);
```

```ruby
class ApplicationRecord; end

class CardArchive < ApplicationRecord
end

class Card
  ATTRS = CardArchive.column_names - ['id']
  attr_accessor(*ATTRS)

  def a = card_id
  def b = owner_id
end
```

### result

```rbs
class Card
  ATTRS: untyped

  def a: -> Integer
  def b: -> Integer
end
```
