// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// error-pattern:reached the recursion limit during monomorphization

// Verify the compiler fails with an error on infinite function
// recursions.

struct Data(Box<Option<Data>>);

fn generic<T>( _ : Vec<(Data,T)> ) {
    let rec : Vec<(Data,(bool,T))> = Vec::new();
    generic( rec );
}


fn main () {
    // Use generic<T> at least once to trigger instantiation.
    let input : Vec<(Data,())> = Vec::new();
    generic(input);
}
