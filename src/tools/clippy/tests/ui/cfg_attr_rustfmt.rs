// run-rustfix
#![feature(stmt_expr_attributes)]

#![allow(unused, clippy::no_effect)]
#![warn(clippy::deprecated_cfg_attr)]

// This doesn't get linted, see known problems
#![cfg_attr(rustfmt, rustfmt_skip)]

#[rustfmt::skip]
trait Foo
{
fn foo(
);
}

fn skip_on_statements() {
    #[cfg_attr(rustfmt, rustfmt::skip)]
    5+3;
}

#[cfg_attr(rustfmt, rustfmt_skip)]
fn main() {
    foo::f();
}

mod foo {
    #![cfg_attr(rustfmt, rustfmt_skip)]

    pub fn f() {}
}
