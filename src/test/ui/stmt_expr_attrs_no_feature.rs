#![feature(rustc_attrs)]

macro_rules! stmt_mac {
    () => {
        fn b() {}
    }
}

fn main() {
    #[rustc_dummy]
    fn a() {}

    // Bug: built-in attrs like `rustc_dummy` are not gated on blocks, but other attrs are.
    #[rustfmt::skip] //~ ERROR attributes on expressions are experimental
    {

    }

    #[rustc_dummy]
    5;

    #[rustc_dummy]
    stmt_mac!();
}

// Check that cfg works right

#[cfg(unset)]
fn c() {
    #[rustc_dummy]
    5;
}

#[cfg(not(unset))]
fn j() {
    #[rustc_dummy]
    5;
}

#[cfg_attr(not(unset), cfg(unset))]
fn d() {
    #[rustc_dummy]
    8;
}

#[cfg_attr(not(unset), cfg(not(unset)))]
fn i() {
    #[rustc_dummy]
    8;
}

// check that macro expansion and cfg works right

macro_rules! item_mac {
    ($e:ident) => {
        fn $e() {
            #[rustc_dummy]
            42;

            #[cfg(unset)]
            fn f() {
                #[rustc_dummy]
                5;
            }

            #[cfg(not(unset))]
            fn k() {
                #[rustc_dummy]
                5;
            }

            #[cfg_attr(not(unset), cfg(unset))]
            fn g() {
                #[rustc_dummy]
                8;
            }

            #[cfg_attr(not(unset), cfg(not(unset)))]
            fn h() {
                #[rustc_dummy]
                8;
            }

        }
    }
}

item_mac!(e);

// check that the gate visitor works right:

extern "C" {
    #[cfg(unset)]
    fn x(a: [u8; #[rustc_dummy] 5]);
    fn y(a: [u8; #[rustc_dummy] 5]); //~ ERROR attributes on expressions are experimental
}

struct Foo;
impl Foo {
    #[cfg(unset)]
    const X: u8 = #[rustc_dummy] 5;
    const Y: u8 = #[rustc_dummy] 5; //~ ERROR attributes on expressions are experimental
}

trait Bar {
    #[cfg(unset)]
    const X: [u8; #[rustc_dummy] 5];
    const Y: [u8; #[rustc_dummy] 5]; //~ ERROR attributes on expressions are experimental
}

struct Joyce {
    #[cfg(unset)]
    field: [u8; #[rustc_dummy] 5],
    field2: [u8; #[rustc_dummy] 5] //~ ERROR attributes on expressions are experimental
}

struct Walky(
    #[cfg(unset)] [u8; #[rustc_dummy] 5],
    [u8; #[rustc_dummy] 5] //~ ERROR attributes on expressions are experimental
);

enum Mike {
    Happy(
        #[cfg(unset)] [u8; #[rustc_dummy] 5],
        [u8; #[rustc_dummy] 5] //~ ERROR attributes on expressions are experimental
    ),
    Angry {
        #[cfg(unset)]
        field: [u8; #[rustc_dummy] 5],
        field2: [u8; #[rustc_dummy] 5] //~ ERROR attributes on expressions are experimental
    }
}

fn pat() {
    match 5 {
        #[cfg(unset)]
        5 => #[rustc_dummy] (),
        6 => #[rustc_dummy] (), //~ ERROR attributes on expressions are experimental
        _ => (),
    }
}
