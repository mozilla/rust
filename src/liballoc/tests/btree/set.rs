use std::collections::BTreeSet;
use std::iter::FromIterator;

use super::DeterministicRng;

#[test]
fn test_clone_eq() {
    let mut m = BTreeSet::new();

    m.insert(1);
    m.insert(2);

    assert_eq!(m.clone(), m);
}

#[test]
fn test_hash() {
    use crate::hash;

    let mut x = BTreeSet::new();
    let mut y = BTreeSet::new();

    x.insert(1);
    x.insert(2);
    x.insert(3);

    y.insert(3);
    y.insert(2);
    y.insert(1);

    assert_eq!(hash(&x), hash(&y));
}

fn check<F>(a: &[i32], b: &[i32], expected: &[i32], f: F)
where
    F: FnOnce(&BTreeSet<i32>, &BTreeSet<i32>, &mut dyn FnMut(&i32) -> bool) -> bool,
{
    let mut set_a = BTreeSet::new();
    let mut set_b = BTreeSet::new();

    for x in a {
        assert!(set_a.insert(*x))
    }
    for y in b {
        assert!(set_b.insert(*y))
    }

    let mut i = 0;
    f(&set_a, &set_b, &mut |&x| {
        if i < expected.len() {
            assert_eq!(x, expected[i]);
        }
        i += 1;
        true
    });
    assert_eq!(i, expected.len());
}

#[test]
fn test_intersection() {
    fn check_intersection(a: &[i32], b: &[i32], expected: &[i32]) {
        check(a, b, expected, |x, y, f| x.intersection(y).all(f))
    }

    check_intersection(&[], &[], &[]);
    check_intersection(&[1, 2, 3], &[], &[]);
    check_intersection(&[], &[1, 2, 3], &[]);
    check_intersection(&[2], &[1, 2, 3], &[2]);
    check_intersection(&[1, 2, 3], &[2], &[2]);
    check_intersection(&[11, 1, 3, 77, 103, 5, -5], &[2, 11, 77, -9, -42, 5, 3], &[3, 5, 11, 77]);

    if cfg!(miri) {
        // Miri is too slow
        return;
    }

    let large = (0..100).collect::<Vec<_>>();
    check_intersection(&[], &large, &[]);
    check_intersection(&large, &[], &[]);
    check_intersection(&[-1], &large, &[]);
    check_intersection(&large, &[-1], &[]);
    check_intersection(&[0], &large, &[0]);
    check_intersection(&large, &[0], &[0]);
    check_intersection(&[99], &large, &[99]);
    check_intersection(&large, &[99], &[99]);
    check_intersection(&[100], &large, &[]);
    check_intersection(&large, &[100], &[]);
    check_intersection(&[11, 5000, 1, 3, 77, 8924], &large, &[1, 3, 11, 77]);
}

#[test]
fn test_intersection_size_hint() {
    let x: BTreeSet<i32> = [3, 4].iter().copied().collect();
    let y: BTreeSet<i32> = [1, 2, 3].iter().copied().collect();
    let mut iter = x.intersection(&y);
    assert_eq!(iter.size_hint(), (1, Some(1)));
    assert_eq!(iter.next(), Some(&3));
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(iter.next(), None);

    iter = y.intersection(&y);
    assert_eq!(iter.size_hint(), (0, Some(3)));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.size_hint(), (0, Some(2)));
}

#[test]
fn test_difference() {
    fn check_difference(a: &[i32], b: &[i32], expected: &[i32]) {
        check(a, b, expected, |x, y, f| x.difference(y).all(f))
    }

    check_difference(&[], &[], &[]);
    check_difference(&[1, 12], &[], &[1, 12]);
    check_difference(&[], &[1, 2, 3, 9], &[]);
    check_difference(&[1, 3, 5, 9, 11], &[3, 9], &[1, 5, 11]);
    check_difference(&[1, 3, 5, 9, 11], &[3, 6, 9], &[1, 5, 11]);
    check_difference(&[1, 3, 5, 9, 11], &[0, 1], &[3, 5, 9, 11]);
    check_difference(&[1, 3, 5, 9, 11], &[11, 12], &[1, 3, 5, 9]);
    check_difference(
        &[-5, 11, 22, 33, 40, 42],
        &[-12, -5, 14, 23, 34, 38, 39, 50],
        &[11, 22, 33, 40, 42],
    );

    if cfg!(miri) {
        // Miri is too slow
        return;
    }

    let large = (0..100).collect::<Vec<_>>();
    check_difference(&[], &large, &[]);
    check_difference(&[-1], &large, &[-1]);
    check_difference(&[0], &large, &[]);
    check_difference(&[99], &large, &[]);
    check_difference(&[100], &large, &[100]);
    check_difference(&[11, 5000, 1, 3, 77, 8924], &large, &[5000, 8924]);
    check_difference(&large, &[], &large);
    check_difference(&large, &[-1], &large);
    check_difference(&large, &[100], &large);
}

#[test]
fn test_difference_size_hint() {
    let s246: BTreeSet<i32> = [2, 4, 6].iter().copied().collect();
    let s23456: BTreeSet<i32> = (2..=6).collect();
    let mut iter = s246.difference(&s23456);
    assert_eq!(iter.size_hint(), (0, Some(3)));
    assert_eq!(iter.next(), None);

    let s12345: BTreeSet<i32> = (1..=5).collect();
    iter = s246.difference(&s12345);
    assert_eq!(iter.size_hint(), (0, Some(3)));
    assert_eq!(iter.next(), Some(&6));
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(iter.next(), None);

    let s34567: BTreeSet<i32> = (3..=7).collect();
    iter = s246.difference(&s34567);
    assert_eq!(iter.size_hint(), (0, Some(3)));
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.size_hint(), (0, Some(2)));
    assert_eq!(iter.next(), None);

    let s1: BTreeSet<i32> = (-9..=1).collect();
    iter = s246.difference(&s1);
    assert_eq!(iter.size_hint(), (3, Some(3)));

    let s2: BTreeSet<i32> = (-9..=2).collect();
    iter = s246.difference(&s2);
    assert_eq!(iter.size_hint(), (2, Some(2)));
    assert_eq!(iter.next(), Some(&4));
    assert_eq!(iter.size_hint(), (1, Some(1)));

    let s23: BTreeSet<i32> = (2..=3).collect();
    iter = s246.difference(&s23);
    assert_eq!(iter.size_hint(), (1, Some(3)));
    assert_eq!(iter.next(), Some(&4));
    assert_eq!(iter.size_hint(), (1, Some(1)));

    let s4: BTreeSet<i32> = (4..=4).collect();
    iter = s246.difference(&s4);
    assert_eq!(iter.size_hint(), (2, Some(3)));
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.size_hint(), (1, Some(2)));
    assert_eq!(iter.next(), Some(&6));
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(iter.next(), None);

    let s56: BTreeSet<i32> = (5..=6).collect();
    iter = s246.difference(&s56);
    assert_eq!(iter.size_hint(), (1, Some(3)));
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.size_hint(), (0, Some(2)));

    let s6: BTreeSet<i32> = (6..=19).collect();
    iter = s246.difference(&s6);
    assert_eq!(iter.size_hint(), (2, Some(2)));
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.size_hint(), (1, Some(1)));

    let s7: BTreeSet<i32> = (7..=19).collect();
    iter = s246.difference(&s7);
    assert_eq!(iter.size_hint(), (3, Some(3)));
}

#[test]
fn test_symmetric_difference() {
    fn check_symmetric_difference(a: &[i32], b: &[i32], expected: &[i32]) {
        check(a, b, expected, |x, y, f| x.symmetric_difference(y).all(f))
    }

    check_symmetric_difference(&[], &[], &[]);
    check_symmetric_difference(&[1, 2, 3], &[2], &[1, 3]);
    check_symmetric_difference(&[2], &[1, 2, 3], &[1, 3]);
    check_symmetric_difference(&[1, 3, 5, 9, 11], &[-2, 3, 9, 14, 22], &[-2, 1, 5, 11, 14, 22]);
}

#[test]
fn test_symmetric_difference_size_hint() {
    let x: BTreeSet<i32> = [2, 4].iter().copied().collect();
    let y: BTreeSet<i32> = [1, 2, 3].iter().copied().collect();
    let mut iter = x.symmetric_difference(&y);
    assert_eq!(iter.size_hint(), (0, Some(5)));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.size_hint(), (0, Some(4)));
    assert_eq!(iter.next(), Some(&3));
    assert_eq!(iter.size_hint(), (0, Some(1)));
}

#[test]
fn test_union() {
    fn check_union(a: &[i32], b: &[i32], expected: &[i32]) {
        check(a, b, expected, |x, y, f| x.union(y).all(f))
    }

    check_union(&[], &[], &[]);
    check_union(&[1, 2, 3], &[2], &[1, 2, 3]);
    check_union(&[2], &[1, 2, 3], &[1, 2, 3]);
    check_union(
        &[1, 3, 5, 9, 11, 16, 19, 24],
        &[-2, 1, 5, 9, 13, 19],
        &[-2, 1, 3, 5, 9, 11, 13, 16, 19, 24],
    );
}

#[test]
fn test_union_size_hint() {
    let x: BTreeSet<i32> = [2, 4].iter().copied().collect();
    let y: BTreeSet<i32> = [1, 2, 3].iter().copied().collect();
    let mut iter = x.union(&y);
    assert_eq!(iter.size_hint(), (3, Some(5)));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.size_hint(), (2, Some(4)));
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.size_hint(), (1, Some(2)));
}

#[test]
// Only tests the simple function definition with respect to intersection
fn test_is_disjoint() {
    let one = [1].iter().collect::<BTreeSet<_>>();
    let two = [2].iter().collect::<BTreeSet<_>>();
    assert!(one.is_disjoint(&two));
}

#[test]
// Also implicitly tests the trivial function definition of is_superset
fn test_is_subset() {
    fn is_subset(a: &[i32], b: &[i32]) -> bool {
        let set_a = a.iter().collect::<BTreeSet<_>>();
        let set_b = b.iter().collect::<BTreeSet<_>>();
        set_a.is_subset(&set_b)
    }

    assert_eq!(is_subset(&[], &[]), true);
    assert_eq!(is_subset(&[], &[1, 2]), true);
    assert_eq!(is_subset(&[0], &[1, 2]), false);
    assert_eq!(is_subset(&[1], &[1, 2]), true);
    assert_eq!(is_subset(&[2], &[1, 2]), true);
    assert_eq!(is_subset(&[3], &[1, 2]), false);
    assert_eq!(is_subset(&[1, 2], &[1]), false);
    assert_eq!(is_subset(&[1, 2], &[1, 2]), true);
    assert_eq!(is_subset(&[1, 2], &[2, 3]), false);
    assert_eq!(
        is_subset(&[-5, 11, 22, 33, 40, 42], &[-12, -5, 11, 14, 22, 23, 33, 34, 38, 39, 40, 42]),
        true
    );
    assert_eq!(is_subset(&[-5, 11, 22, 33, 40, 42], &[-12, -5, 11, 14, 22, 23, 34, 38]), false);

    if cfg!(miri) {
        // Miri is too slow
        return;
    }

    let large = (0..100).collect::<Vec<_>>();
    assert_eq!(is_subset(&[], &large), true);
    assert_eq!(is_subset(&large, &[]), false);
    assert_eq!(is_subset(&[-1], &large), false);
    assert_eq!(is_subset(&[0], &large), true);
    assert_eq!(is_subset(&[1, 2], &large), true);
    assert_eq!(is_subset(&[99, 100], &large), false);
}

#[test]
fn test_clear() {
    let mut x = BTreeSet::new();
    x.insert(1);

    x.clear();
    assert!(x.is_empty());
}

#[test]
fn test_zip() {
    let mut x = BTreeSet::new();
    x.insert(5);
    x.insert(12);
    x.insert(11);

    let mut y = BTreeSet::new();
    y.insert("foo");
    y.insert("bar");

    let x = x;
    let y = y;
    let mut z = x.iter().zip(&y);

    assert_eq!(z.next().unwrap(), (&5, &("bar")));
    assert_eq!(z.next().unwrap(), (&11, &("foo")));
    assert!(z.next().is_none());
}

#[test]
fn test_from_iter() {
    let xs = [1, 2, 3, 4, 5, 6, 7, 8, 9];

    let set: BTreeSet<_> = xs.iter().cloned().collect();

    for x in &xs {
        assert!(set.contains(x));
    }
}

#[test]
fn test_show() {
    let mut set = BTreeSet::new();
    let empty = BTreeSet::<i32>::new();

    set.insert(1);
    set.insert(2);

    let set_str = format!("{:?}", set);

    assert_eq!(set_str, "{1, 2}");
    assert_eq!(format!("{:?}", empty), "{}");
}

#[test]
fn test_extend_ref() {
    let mut a = BTreeSet::new();
    a.insert(1);

    a.extend(&[2, 3, 4]);

    assert_eq!(a.len(), 4);
    assert!(a.contains(&1));
    assert!(a.contains(&2));
    assert!(a.contains(&3));
    assert!(a.contains(&4));

    let mut b = BTreeSet::new();
    b.insert(5);
    b.insert(6);

    a.extend(&b);

    assert_eq!(a.len(), 6);
    assert!(a.contains(&1));
    assert!(a.contains(&2));
    assert!(a.contains(&3));
    assert!(a.contains(&4));
    assert!(a.contains(&5));
    assert!(a.contains(&6));
}

#[test]
fn test_recovery() {
    use std::cmp::Ordering;

    #[derive(Debug)]
    struct Foo(&'static str, i32);

    impl PartialEq for Foo {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl Eq for Foo {}

    impl PartialOrd for Foo {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            self.0.partial_cmp(&other.0)
        }
    }

    impl Ord for Foo {
        fn cmp(&self, other: &Self) -> Ordering {
            self.0.cmp(&other.0)
        }
    }

    let mut s = BTreeSet::new();
    assert_eq!(s.replace(Foo("a", 1)), None);
    assert_eq!(s.len(), 1);
    assert_eq!(s.replace(Foo("a", 2)), Some(Foo("a", 1)));
    assert_eq!(s.len(), 1);

    {
        let mut it = s.iter();
        assert_eq!(it.next(), Some(&Foo("a", 2)));
        assert_eq!(it.next(), None);
    }

    assert_eq!(s.get(&Foo("a", 1)), Some(&Foo("a", 2)));
    assert_eq!(s.take(&Foo("a", 1)), Some(Foo("a", 2)));
    assert_eq!(s.len(), 0);

    assert_eq!(s.get(&Foo("a", 1)), None);
    assert_eq!(s.take(&Foo("a", 1)), None);

    assert_eq!(s.iter().next(), None);
}

#[test]
#[allow(dead_code)]
fn test_variance() {
    use std::collections::btree_set::{IntoIter, Iter, Range};

    fn set<'new>(v: BTreeSet<&'static str>) -> BTreeSet<&'new str> {
        v
    }
    fn iter<'a, 'new>(v: Iter<'a, &'static str>) -> Iter<'a, &'new str> {
        v
    }
    fn into_iter<'new>(v: IntoIter<&'static str>) -> IntoIter<&'new str> {
        v
    }
    fn range<'a, 'new>(v: Range<'a, &'static str>) -> Range<'a, &'new str> {
        v
    }
}

#[test]
fn test_append() {
    let mut a = BTreeSet::new();
    a.insert(1);
    a.insert(2);
    a.insert(3);

    let mut b = BTreeSet::new();
    b.insert(3);
    b.insert(4);
    b.insert(5);

    a.append(&mut b);

    assert_eq!(a.len(), 5);
    assert_eq!(b.len(), 0);

    assert_eq!(a.contains(&1), true);
    assert_eq!(a.contains(&2), true);
    assert_eq!(a.contains(&3), true);
    assert_eq!(a.contains(&4), true);
    assert_eq!(a.contains(&5), true);
}

#[test]
fn test_first_last() {
    let mut a = BTreeSet::new();
    assert_eq!(a.first(), None);
    assert_eq!(a.last(), None);
    a.insert(1);
    assert_eq!(a.first(), Some(&1));
    assert_eq!(a.last(), Some(&1));
    a.insert(2);
    assert_eq!(a.first(), Some(&1));
    assert_eq!(a.last(), Some(&2));
    a.insert(3);
    assert_eq!(a.first(), Some(&1));
    assert_eq!(a.last(), Some(&3));

    assert_eq!(a.len(), 3);
    assert_eq!(a.pop_first(), Some(1));
    assert_eq!(a.len(), 2);
    assert_eq!(a.pop_last(), Some(3));
    assert_eq!(a.len(), 1);
    assert_eq!(a.pop_first(), Some(2));
    assert_eq!(a.len(), 0);
    assert_eq!(a.pop_last(), None);
    assert_eq!(a.len(), 0);
    assert_eq!(a.pop_first(), None);
    assert_eq!(a.len(), 0);
}

fn rand_data(len: usize) -> Vec<u32> {
    let mut rng = DeterministicRng::new();
    Vec::from_iter((0..len).map(|_| rng.next()))
}

#[test]
fn test_split_off_empty_right() {
    let mut data = rand_data(173);

    let mut set = BTreeSet::from_iter(data.clone());
    let right = set.split_off(&(data.iter().max().unwrap() + 1));

    data.sort();
    assert!(set.into_iter().eq(data));
    assert!(right.into_iter().eq(None));
}

#[test]
fn test_split_off_empty_left() {
    let mut data = rand_data(314);

    let mut set = BTreeSet::from_iter(data.clone());
    let right = set.split_off(data.iter().min().unwrap());

    data.sort();
    assert!(set.into_iter().eq(None));
    assert!(right.into_iter().eq(data));
}

#[test]
fn test_split_off_large_random_sorted() {
    #[cfg(not(miri))] // Miri is too slow
    let mut data = rand_data(1529);
    #[cfg(miri)]
    let mut data = rand_data(529);
    // special case with maximum height.
    data.sort();

    let mut set = BTreeSet::from_iter(data.clone());
    let key = data[data.len() / 2];
    let right = set.split_off(&key);

    assert!(set.into_iter().eq(data.clone().into_iter().filter(|x| *x < key)));
    assert!(right.into_iter().eq(data.into_iter().filter(|x| *x >= key)));
}
