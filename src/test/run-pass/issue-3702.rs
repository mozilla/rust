pub fn main() {
  trait Text {
    fn to_string(&self) -> String;
  }

  fn to_string(t: Box<Text>) {
    println!("{}", (*t).to_string());
  }

}
