// build-fail
 //~^ overflow evaluating
// compile-flags: -Zproject_under_binders=no
 #![recursion_limit="45"]
fn main() {
    let mut iter = 0u8..1;
    func(&mut iter)
}

fn func<T: Iterator<Item = u8>>(iter: &mut T) {
    func(&mut iter.map(|x| x + 1))
}
