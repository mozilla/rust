#![feature(type_ascription)]

fn foo<'a>(arg : &'a [u32; 3]) -> &'a [u32] {
  arg : &[u32]
  //~^ ERROR type ascriptions are not
}

fn main() {
  let arr = [4,5,6];
  let _ = foo(&arr);
}
