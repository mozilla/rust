// check-pass

#![feature(destructuring_assignment)]
#![feature(more_qualified_paths)]

enum E { V() }

fn main() {
    <E>::V() = E::V(); // OK, destructuring assignment
    <E>::V {} = E::V(); // OK, destructuring assignment
}
