fn main() {
    let x = true;
    if x { let mut i = 10; while i > 0 { i -= 1; } }
    match x { true => { debug!("right"); } false => { debug!("wrong"); } }
}
