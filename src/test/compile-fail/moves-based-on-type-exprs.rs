// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Tests that references to move-by-default values trigger moves when
// they occur as part of various kinds of expressions.


struct Foo<A> { f: A }
fn guard(_s: String) -> bool {panic!()}
fn touch<A>(_a: &A) {}

fn f10() {
    let x = "hi".to_string();
    let _y = Foo { f:x };
    touch(&x); //~ ERROR use of moved value: `x`
}

fn f20() {
    let x = "hi".to_string();
    let _y = (x, 3is);
    touch(&x); //~ ERROR use of moved value: `x`
}

fn f21() {
    let x = vec!(1is, 2, 3);
    let _y = (x[0], 3is);
    touch(&x);
}

fn f30(cond: bool) {
    let x = "hi".to_string();
    let y = "ho".to_string();
    let _y = if cond {
        x
    } else {
        y
    };
    touch(&x); //~ ERROR use of moved value: `x`
    touch(&y); //~ ERROR use of moved value: `y`
}

fn f40(cond: bool) {
    let x = "hi".to_string();
    let y = "ho".to_string();
    let _y = match cond {
        true => x,
        false => y
    };
    touch(&x); //~ ERROR use of moved value: `x`
    touch(&y); //~ ERROR use of moved value: `y`
}

fn f50(cond: bool) {
    let x = "hi".to_string();
    let y = "ho".to_string();
    let _y = match cond {
        _ if guard(x) => 10is,
        true => 10is,
        false => 20is,
    };
    touch(&x); //~ ERROR use of moved value: `x`
    touch(&y);
}

fn f70() {
    let x = "hi".to_string();
    let _y = [x];
    touch(&x); //~ ERROR use of moved value: `x`
}

fn f80() {
    let x = "hi".to_string();
    let _y = vec!(x);
    touch(&x); //~ ERROR use of moved value: `x`
}

fn f100() {
    let x = vec!("hi".to_string());
    let _y = x.into_iter().next().unwrap();
    touch(&x); //~ ERROR use of moved value: `x`
}

fn f110() {
    let x = vec!("hi".to_string());
    let _y = [x.into_iter().next().unwrap(); 1];
    touch(&x); //~ ERROR use of moved value: `x`
}

fn f120() {
    let mut x = vec!("hi".to_string(), "ho".to_string());
    x.as_mut_slice().swap(0, 1);
    touch(&x[0]);
    touch(&x[1]);
}

fn main() {}
