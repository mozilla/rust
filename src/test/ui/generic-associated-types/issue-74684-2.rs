#![feature(generic_associated_types)]
  //~^ WARNING: the feature `generic_associated_types` is incomplete

trait Fun {
    type F<'a>: ?Sized;

    fn identity<'a>(t: &'a Self::F<'a>) -> &'a Self::F<'a> { t }
}

impl <T> Fun for T {
    type F<'a> = i32;
}

fn bug<'a, T: ?Sized + Fun<F<'a> = [u8]>>(t: Box<T>) -> &'static T::F<'a> {
    let a = [0; 1];
    let x = T::identity(&a);
    todo!()
}


fn main() {
    let x = 10;

    bug(Box::new(x));
      //~^ ERROR: type mismatch resolving `<{integer} as Fun>::F<'_> == [u8]`
}
