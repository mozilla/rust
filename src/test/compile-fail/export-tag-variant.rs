// xfail-boot
// xfail-stage0
// error-pattern: unresolved name

mod foo {
  export x;

  fn x() {
  }

  tag y {
    y1;
  }
}

fn main() {
  auto z = foo.y1;
}
