#![feature(min_const_generics)]

struct Example<const N: usize>;

macro_rules! external_macro {
  () => {{
    //~^ ERROR expected type
    const X: usize = 1337;
    X
  }}
}

trait Marker<const N: usize> {}
impl<const N: usize> Marker<N> for Example<N> {}

fn make_marker() -> impl Marker<gimme_a_const!(marker)> {
  //~^ ERROR wrong number of const
  //~| ERROR wrong number of type
  Example::<gimme_a_const!(marker)>
  //~^ ERROR wrong number of const
  //~| ERROR wrong number of type
}

fn from_marker(_: impl Marker<{
    #[macro_export]
    macro_rules! inline { () => {{ 3 }} }; inline!()
}>) {}

fn main() {
  let _ok = Example::<{
    #[macro_export]
    macro_rules! gimme_a_const {
      ($rusty: ident) => {{ let $rusty = 3; *&$rusty }}
      //~^ ERROR expected type
      //~| ERROR expected type
    };
    gimme_a_const!(run)
  }>;

  let _fail = Example::<external_macro!()>;
  //~^ ERROR wrong number of const
  //~| ERROR wrong number of type

  let _fail = Example::<gimme_a_const!()>;
  //~^ ERROR wrong number of const
  //~| ERROR wrong number of type
  //~| ERROR unexpected end of macro invocation
}
