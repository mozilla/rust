// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(advanced_slice_patterns)]

fn foldl<T, U, F>(values: &[T],
                  initial: U,
                  mut function: F)
                  -> U where
    U: Clone,
    F: FnMut(U, &T) -> U,
{
    match values {
        [ref head, tail..] =>
            foldl(tail, function(initial, head), function),
        [] => initial.clone()
    }
}

fn foldr<T, U, F>(values: &[T],
                  initial: U,
                  mut function: F)
                  -> U where
    U: Clone,
    F: FnMut(&T, U) -> U,
{
    match values {
        [head.., ref tail] =>
            foldr(head, function(tail, initial), function),
        [] => initial.clone()
    }
}

pub fn main() {
    let x = &[1i, 2, 3, 4, 5];

    let product = foldl(x, 1i, |a, b| a * *b);
    assert_eq!(product, 120);

    let sum = foldr(x, 0i, |a, b| *a + b);
    assert_eq!(sum, 15);
}
