//

type FontTableTag = u32;

trait FontTableTagConversions {
  fn tag_to_string(self);
}

impl FontTableTagConversions for FontTableTag {
  fn tag_to_string(self) {
    &self;
  }
}

pub fn main() {
    5.tag_to_string();
}
