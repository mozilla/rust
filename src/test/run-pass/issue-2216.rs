pub fn main() {
    let mut x = 0;

    'foo: loop {
        'bar: loop {
            'quux: loop {
                if 1 == 2 {
                    break 'foo;
                }
                else {
                    break 'bar;
                }
            }
            continue 'foo;
        }
        x = 42;
        break;
    }

    println!("{}", x);
    assert_eq!(x, 42);
}
