struct NonOrd;

fn main() {
    let _: Box<Iterator<Item = _>> = Box::new(vec![NonOrd].into_iter());
}
