// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// Creates a `Vec` containing the arguments.
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
macro_rules! vec {
    ($x:expr; $y:expr) => (
        <[_] as $crate::slice::SliceExt>::into_vec(
            $crate::boxed::Box::new([$x; $y]))
    );
    ($($x:expr),*) => (
        <[_] as $crate::slice::SliceExt>::into_vec(
            $crate::boxed::Box::new([$($x),*]))
    );
    ($($x:expr,)*) => (vec![$($x),*])
}

macro_rules! impl_seq_fmt {
    ($seq:ident, $annotation:expr, $($Trait:ident => $fmt_fun:ident),+) => {
        $(
            impl<T: fmt::$Trait> fmt::$Trait for $seq <T> {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    try!(write!(f, "{} ", $annotation));
                    try!(write!(f, "["));
                    try!($fmt_fun(self.iter(), f));
                    write!(f, "]")
                }
            }
        )+
    };

    ($seq:ident, $($Trait:ident => $fmt_fun:ident),+) => {
        $(
            impl<T: fmt::$Trait> fmt::$Trait for $seq<T> {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    try!(write!(f, "["));
                    try!($fmt_fun(self.iter(), f));
                    write!(f, "]")
                }
            }
        )+
    }
}

macro_rules! impl_map_fmt {
    ($map:ident, $annotation:expr, $($Trait:ident => $fmt_fun:ident),+) => {
        $(
            impl<K: fmt::$Trait, V: fmt::$Trait> fmt::$Trait for $map<K, V> {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    try!(write!(f, "{} ", $annotation));
                    try!(write!(f, "{{"));
                    try!($fmt_fun(self.iter(), f));
                    write!(f, "}}")
                }
            }
        )+
    };

    (Fixed $map:ident, $annotation:expr, $($Trait:ident => $fmt_fun:ident),+) => {
        $(
            impl<T: fmt::$Trait> fmt::$Trait for $map <T> {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    try!(write!(f, "{} ", $annotation));
                    try!(write!(f, "{{"));
                    try!($fmt_fun(self.iter(), f));
                    write!(f, "}}")
                }
            }
        )+
    }
}
