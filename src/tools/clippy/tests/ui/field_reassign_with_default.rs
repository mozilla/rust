// aux-build:proc_macro_derive.rs
// aux-build:macro_rules.rs

#![warn(clippy::field_reassign_with_default)]

#[macro_use]
extern crate proc_macro_derive;
#[macro_use]
extern crate macro_rules;

// Don't lint on derives that derive `Default`
// See https://github.com/rust-lang/rust-clippy/issues/6545
#[derive(FieldReassignWithDefault)]
struct DerivedStruct;

#[derive(Default)]
struct A {
    i: i32,
    j: i64,
}

struct B {
    i: i32,
    j: i64,
}

#[derive(Default)]
struct C {
    i: Vec<i32>,
    j: i64,
}

#[derive(Default)]
struct D {
    a: Option<i32>,
    b: Option<i32>,
}

macro_rules! m {
    ($key:ident: $value:tt) => {{
        let mut data = $crate::D::default();
        data.$key = Some($value);
        data
    }};
}

/// Implements .next() that returns a different number each time.
struct SideEffect(i32);

impl SideEffect {
    fn new() -> SideEffect {
        SideEffect(0)
    }
    fn next(&mut self) -> i32 {
        self.0 += 1;
        self.0
    }
}

fn main() {
    // wrong, produces first error in stderr
    let mut a: A = Default::default();
    a.i = 42;

    // right
    let mut a: A = Default::default();

    // right
    let a = A {
        i: 42,
        ..Default::default()
    };

    // right
    let mut a: A = Default::default();
    if a.i == 0 {
        a.j = 12;
    }

    // right
    let mut a: A = Default::default();
    let b = 5;

    // right
    let mut b = 32;
    let mut a: A = Default::default();
    b = 2;

    // right
    let b: B = B { i: 42, j: 24 };

    // right
    let mut b: B = B { i: 42, j: 24 };
    b.i = 52;

    // right
    let mut b = B { i: 15, j: 16 };
    let mut a: A = Default::default();
    b.i = 2;

    // wrong, produces second error in stderr
    let mut a: A = Default::default();
    a.j = 43;
    a.i = 42;

    // wrong, produces third error in stderr
    let mut a: A = Default::default();
    a.i = 42;
    a.j = 43;
    a.j = 44;

    // wrong, produces fourth error in stderr
    let mut a = A::default();
    a.i = 42;

    // wrong, but does not produce an error in stderr, because we can't produce a correct kind of
    // suggestion with current implementation
    let mut c: (i32, i32) = Default::default();
    c.0 = 42;
    c.1 = 21;

    // wrong, produces the fifth error in stderr
    let mut a: A = Default::default();
    a.i = Default::default();

    // wrong, produces the sixth error in stderr
    let mut a: A = Default::default();
    a.i = Default::default();
    a.j = 45;

    // right, because an assignment refers to another field
    let mut x = A::default();
    x.i = 42;
    x.j = 21 + x.i as i64;

    // right, we bail out if there's a reassignment to the same variable, since there is a risk of
    // side-effects affecting the outcome
    let mut x = A::default();
    let mut side_effect = SideEffect::new();
    x.i = side_effect.next();
    x.j = 2;
    x.i = side_effect.next();

    // don't lint - some private fields
    let mut x = m::F::default();
    x.a = 1;

    // don't expand macros in the suggestion (#6522)
    let mut a: C = C::default();
    a.i = vec![1];

    // Don't lint in external macros
    field_reassign_with_default!();

    // be sure suggestion is correct with generics
    let mut a: Wrapper<bool> = Default::default();
    a.i = true;

    let mut a: WrapperMulti<i32, i64> = Default::default();
    a.i = 42;

    // Don't lint in macros
    m! {
        a: 42
    };
}

mod m {
    #[derive(Default)]
    pub struct F {
        pub a: u64,
        b: u64,
    }
}

#[derive(Default)]
struct Wrapper<T> {
    i: T,
}

#[derive(Default)]
struct WrapperMulti<T, U> {
    i: T,
    j: U,
}
