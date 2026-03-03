# Rails / DSL / Shrine

## Generate accessors from Shrine::Attachment include

### update

```ruby
class ActiveRecord::Base
end

module Shrine
  module Attachment
  end
end

class Photo < ActiveRecord::Base
  include Shrine::Attachment(:image)
  include Shrine::Attachment(:thumb)
end
```

### result

```rbs
class Photo < ActiveRecord::Base
  def image: -> Shrine::UploadedFile
  def image=: ((String | Hash[untyped, untyped] | IO) image) -> Shrine::UploadedFile
  def image_attacher: -> Shrine::Attacher
  def image_changed: -> bool
  def image_url: -> String
  def thumb: -> Shrine::UploadedFile
  def thumb=: ((String | Hash[untyped, untyped] | IO) thumb) -> Shrine::UploadedFile
  def thumb_attacher: -> Shrine::Attacher
  def thumb_changed: -> bool
  def thumb_url: -> String
end
```
