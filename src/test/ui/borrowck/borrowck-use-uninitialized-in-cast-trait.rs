// revisions: ast mir
//[mir]compile-flags: -Z borrowck=mir

// Variation on `borrowck-use-uninitialized-in-cast` in which we do a
// trait cast from an uninitialized source. Issue #20791.

trait Foo { fn dummy(&self) { } }
impl Foo for i32 { }

fn main() {
    let x: &i32;
    let y = x as *const Foo; //[ast]~ ERROR use of possibly uninitialized variable: `*x`
                             //[mir]~^ ERROR [E0381]
}
