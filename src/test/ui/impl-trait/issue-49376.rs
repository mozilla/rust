// compile-pass

// Tests for nested self-reference which caused a stack overflow.

use std::fmt::Debug;
use std::ops::*;

fn gen() -> impl PartialOrd + PartialEq + Debug { }

struct Bar {}
trait Foo<T = Self> {}
impl Foo for Bar {}

fn foo() -> impl Foo {
    Bar {}
}

fn test_impl_ops() -> impl Add + Sub + Mul + Div { 1 }
fn test_impl_assign_ops() -> impl AddAssign + SubAssign + MulAssign + DivAssign { 1 }

fn main() {}
