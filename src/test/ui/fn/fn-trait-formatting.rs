#![feature(box_syntax)]

fn needs_fn<F>(x: F) where F: Fn(isize) -> isize {}

fn main() {
    let _: () = (box |_: isize| {}) as Box<dyn FnOnce(isize)>;
    //~^ ERROR mismatched types
    //~| expected unit type `()`
    //~| found struct `Box<dyn FnOnce(isize)>`
    let _: () = (box |_: isize, isize| {}) as Box<dyn Fn(isize, isize)>;
    //~^ ERROR mismatched types
    //~| expected unit type `()`
    //~| found struct `Box<dyn Fn(isize, isize)>`
    let _: () = (box || -> isize { unimplemented!() }) as Box<dyn FnMut() -> isize>;
    //~^ ERROR mismatched types
    //~| expected unit type `()`
    //~| found struct `Box<dyn FnMut() -> isize>`

    needs_fn(1);
    //~^ ERROR expected a `Fn<(isize,)>` closure, found `{integer}`
}
