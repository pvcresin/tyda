# Rails / Schema / Structure SQL

## Resolve column accessors from structure.sql

### update

`db/structure.sql`

```sql
CREATE TABLE public.posts (
    id bigint NOT NULL,
    title character varying,
    a_id bigint NOT NULL,
    published boolean DEFAULT false NOT NULL,
    created_at timestamp(6) without time zone NOT NULL
);
```

```ruby
class ApplicationRecord; end

class Post < ApplicationRecord
  def foo = a_id.succ

  def bar = title&.upcase

  def baz = published
end
```

### result

```rbs
class Post < ApplicationRecord
  def foo: -> Integer
  def bar: -> String?
  def baz: -> bool
end
```

## Infer namespaced models from structure.sql

### update

`db/structure.sql`

```sql
CREATE TABLE public.admin_users (
    id bigint NOT NULL,
    email character varying NOT NULL
);
```

`app/models/admin/user.rb`

```ruby
class Admin::User < ApplicationRecord
end
```

```ruby
class ApplicationRecord; end

class Admin::User < ApplicationRecord
  def foo = email.downcase
end
```

### result

```rbs
class Admin::User < ApplicationRecord
  def foo: -> String
end
```

## Infer association target from structure.sql foreign key

### update

`config/initializers/inflections.rb`

```ruby
ActiveSupport::Inflector.inflections(:en) do |inflect|
  inflect.irregular "person", "people"
end
```

`db/structure.sql`

```sql
CREATE TABLE public.posts (
    id bigint NOT NULL,
    person_id bigint NOT NULL
);

CREATE TABLE public.people (
    id bigint NOT NULL,
    name character varying NOT NULL
);

ALTER TABLE ONLY public.posts
    ADD CONSTRAINT posts_person_id_fkey FOREIGN KEY (person_id) REFERENCES public.people(id);
```

`app/models/person.rb`

```ruby
class Person < ApplicationRecord
end
```

```ruby
class ApplicationRecord; end

class Post < ApplicationRecord
  def foo = person.name
end
```

### result

```rbs
class Post < ApplicationRecord
  def foo: -> String
end
```
