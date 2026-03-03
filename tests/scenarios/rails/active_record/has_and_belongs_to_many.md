# Rails / Active Record / Has And Belongs To Many

## Generate has_and_belongs_to_many collection methods

### update

```ruby
class Article
  has_and_belongs_to_many :tags
end
```

### result

```rbs
class Article
  def tags: -> ActiveRecord::Associations::CollectionProxy[Tag]
  def tag_ids: -> Array[Integer]
  def tag_ids=: (Array[Integer] tag_ids) -> Array[Integer]
  def tags=: (Array[Tag] tags) -> ActiveRecord::Associations::CollectionProxy[Tag]
end
```

## has_and_belongs_to_many class_name sets the class

### update

```ruby
class Course
  has_and_belongs_to_many :students, class_name: "User"
end
```

### result

```rbs
class Course
  def students: -> ActiveRecord::Associations::CollectionProxy[User]
  def student_ids: -> Array[Integer]
  def student_ids=: (Array[Integer] student_ids) -> Array[Integer]
  def students=: (Array[User] students) -> ActiveRecord::Associations::CollectionProxy[User]
end
```
