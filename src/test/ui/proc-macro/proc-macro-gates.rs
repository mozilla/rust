// aux-build:test-macros.rs
// gate-test-proc_macro_hygiene

#![feature(stmt_expr_attributes)]

#[macro_use]
extern crate test_macros;

fn _test_inner() {
    #![empty_attr] //~ ERROR: non-builtin inner attributes are unstable
}

mod _test2_inner {
    #![empty_attr] //~ ERROR: non-builtin inner attributes are unstable
}

#[empty_attr = "y"] //~ ERROR: key-value macro attributes are not supported
fn _test3() {}

fn attrs() {
    // Statement, item
    #[empty_attr] // OK
    struct S;

    // Statement, macro
    #[empty_attr] //~ ERROR: custom attributes cannot be applied to statements
    println!();

    // Statement, semi
    #[empty_attr] //~ ERROR: custom attributes cannot be applied to statements
    S;

    // Statement, local
    #[empty_attr] //~ ERROR: custom attributes cannot be applied to statements
    let _x = 2;

    // Expr
    let _x = #[identity_attr] 2; //~ ERROR: custom attributes cannot be applied to expressions

    // Opt expr
    let _x = [#[identity_attr] 2]; //~ ERROR: custom attributes cannot be applied to expressions

    // Expr macro
    let _x = #[identity_attr] println!();
    //~^ ERROR: custom attributes cannot be applied to expressions
}

fn main() {
    if let identity!(Some(_x)) = Some(3) {}
    //~^ ERROR: procedural macros cannot be expanded to patterns

    empty!(struct S;); //~ ERROR: procedural macros cannot be expanded to statements
    empty!(let _x = 3;); //~ ERROR: procedural macros cannot be expanded to statements

    let _x = identity!(3); //~ ERROR: procedural macros cannot be expanded to expressions
    let _x = [empty!(3)]; //~ ERROR: procedural macros cannot be expanded to expressions
}
