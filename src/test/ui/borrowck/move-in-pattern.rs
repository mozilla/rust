// run-rustfix
// Issue #63988
#[derive(Debug)]
struct S;
fn foo(_: Option<S>) {}

fn main() {
    let s = Some(S);
    if let Some(x) = s {
        let _ = x;
    }
    foo(s); //~ ERROR use of moved value: `s`
}
