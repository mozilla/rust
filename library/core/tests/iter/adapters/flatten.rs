use super::*;
use core::iter::*;

#[test]
fn test_iterator_flatten() {
    let xs = [0, 3, 6];
    let ys = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    let it = xs.iter().map(|&x| (x..).step_by(1).take(3)).flatten();
    let mut i = 0;
    for x in it {
        assert_eq!(x, ys[i]);
        i += 1;
    }
    assert_eq!(i, ys.len());
}

/// Tests `Flatten::fold` with items already picked off the front and back,
/// to make sure all parts of the `Flatten` are folded correctly.
#[test]
fn test_iterator_flatten_fold() {
    let xs = [0, 3, 6];
    let ys = [1, 2, 3, 4, 5, 6, 7];
    let mut it = xs.iter().map(|&x| x..x + 3).flatten();
    assert_eq!(it.next(), Some(0));
    assert_eq!(it.next_back(), Some(8));
    let i = it.fold(0, |i, x| {
        assert_eq!(x, ys[i]);
        i + 1
    });
    assert_eq!(i, ys.len());

    let mut it = xs.iter().map(|&x| x..x + 3).flatten();
    assert_eq!(it.next(), Some(0));
    assert_eq!(it.next_back(), Some(8));
    let i = it.rfold(ys.len(), |i, x| {
        assert_eq!(x, ys[i - 1]);
        i - 1
    });
    assert_eq!(i, 0);
}

#[test]
fn test_flatten_try_folds() {
    let f = &|acc, x| i32::checked_add(acc * 2 / 3, x);
    let mr = &|x| (5 * x)..(5 * x + 5);
    assert_eq!((0..10).map(mr).flatten().try_fold(7, f), (0..50).try_fold(7, f));
    assert_eq!((0..10).map(mr).flatten().try_rfold(7, f), (0..50).try_rfold(7, f));
    let mut iter = (0..10).map(mr).flatten();
    iter.next();
    iter.next_back(); // have front and back iters in progress
    assert_eq!(iter.try_rfold(7, f), (1..49).try_rfold(7, f));

    let mut iter = (0..10).map(|x| (4 * x)..(4 * x + 4)).flatten();
    assert_eq!(iter.try_fold(0, i8::checked_add), None);
    assert_eq!(iter.next(), Some(17));
    assert_eq!(iter.try_rfold(0, i8::checked_add), None);
    assert_eq!(iter.next_back(), Some(35));
}

#[test]
fn test_flatten_non_fused_outer() {
    let mut iter = NonFused::new(once(0..2)).flatten();

    assert_eq!(iter.next_back(), Some(1));
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);

    let mut iter = NonFused::new(once(0..2)).flatten();

    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next_back(), Some(1));
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next_back(), None);
}

#[test]
fn test_flatten_non_fused_inner() {
    let mut iter = once(0..1).chain(once(1..3)).flat_map(NonFused::new);

    assert_eq!(iter.next_back(), Some(2));
    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next(), Some(1));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);

    let mut iter = once(0..1).chain(once(1..3)).flat_map(NonFused::new);

    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.next_back(), Some(2));
    assert_eq!(iter.next_back(), Some(1));
    assert_eq!(iter.next_back(), None);
    assert_eq!(iter.next_back(), None);
}

#[test]
fn test_double_ended_flatten() {
    let u = [0, 1];
    let v = [5, 6, 7, 8];
    let mut it = u.iter().map(|x| &v[*x..v.len()]).flatten();
    assert_eq!(it.next_back().unwrap(), &8);
    assert_eq!(it.next().unwrap(), &5);
    assert_eq!(it.next_back().unwrap(), &7);
    assert_eq!(it.next_back().unwrap(), &6);
    assert_eq!(it.next_back().unwrap(), &8);
    assert_eq!(it.next().unwrap(), &6);
    assert_eq!(it.next_back().unwrap(), &7);
    assert_eq!(it.next_back(), None);
    assert_eq!(it.next(), None);
    assert_eq!(it.next_back(), None);
}
