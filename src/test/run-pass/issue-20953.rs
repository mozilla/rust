fn main() {
    let mut shrinker: Box<Iterator<Item=i32>> = Box::new(vec![1].into_iter());
    println!("{:?}", shrinker.next());
    for v in shrinker { assert!(false); }

    let mut shrinker: &mut Iterator<Item=i32> = &mut vec![1].into_iter();
    println!("{:?}", shrinker.next());
    for v in shrinker { assert!(false); }
}
