// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cmp::Ordering::{Equal, Less, Greater};

#[test]
fn test_clone() {
    let a = (1i, "2");
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn test_tuple_cmp() {
    let (small, big) = ((1u, 2u, 3u), (3u, 2u, 1u));

    let nan = 0.0f64/0.0;

    // PartialEq
    assert_eq!(small, small);
    assert_eq!(big, big);
    assert!(small != big);
    assert!(big != small);

    // PartialOrd
    assert!(small < big);
    assert!(!(small < small));
    assert!(!(big < small));
    assert!(!(big < big));

    assert!(small <= small);
    assert!(big <= big);

    assert!(big > small);
    assert!(small >= small);
    assert!(big >= small);
    assert!(big >= big);

    assert!(!((1.0f64, 2.0f64) < (nan, 3.0)));
    assert!(!((1.0f64, 2.0f64) <= (nan, 3.0)));
    assert!(!((1.0f64, 2.0f64) > (nan, 3.0)));
    assert!(!((1.0f64, 2.0f64) >= (nan, 3.0)));
    assert!(((1.0f64, 2.0f64) < (2.0, nan)));
    assert!(!((2.0f64, 2.0f64) < (2.0, nan)));

    // Ord
    assert!(small.cmp(&small) == Equal);
    assert!(big.cmp(&big) == Equal);
    assert!(small.cmp(&big) == Less);
    assert!(big.cmp(&small) == Greater);
}

#[test]
fn test_show() {
    let s = format!("{:?}", (1i,));
    assert_eq!(s, "(1i,)");
    let s = format!("{:?}", (1i, true));
    assert_eq!(s, "(1i, true)");
    let s = format!("{:?}", (1i, "hi", true));
    assert_eq!(s, "(1i, \"hi\", true)");
}
