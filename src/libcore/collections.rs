//! Collection types.
//!
//! See [`std::collections`](../std/collections/index.html) for a detailed
//! discussion of collections in Rust.

/// An endpoint of a range of keys.
///
/// # Examples
///
/// `Bound`s are range endpoints:
///
/// ```
/// #![feature(collections_range)]
///
/// use std::collections::range::RangeArgument;
/// use std::collections::Bound::*;
///
/// assert_eq!((..100).start(), Unbounded);
/// assert_eq!((1..12).start(), Included(&1));
/// assert_eq!((1..12).end(), Excluded(&12));
/// ```
///
/// Using a tuple of `Bound`s as an argument to [`BTreeMap::range`].
/// Note that in most cases, it's better to use range syntax (`1..5`) instead.
///
/// ```
/// use std::collections::BTreeMap;
/// use std::collections::Bound::{Excluded, Included, Unbounded};
///
/// let mut map = BTreeMap::new();
/// map.insert(3, "a");
/// map.insert(5, "b");
/// map.insert(8, "c");
///
/// for (key, value) in map.range((Excluded(3), Included(8))) {
///     println!("{}: {}", key, value);
/// }
///
/// assert_eq!(Some((&3, &"a")), map.range((Unbounded, Included(5))).next());
/// ```
///
/// [`BTreeMap::range`]: btree_map/struct.BTreeMap.html#method.range
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[stable(feature = "collections_bound", since = "1.17.0")]
pub enum Bound<T> {
    /// An inclusive bound.
    #[stable(feature = "collections_bound", since = "1.17.0")]
    Included(T),
    /// An exclusive bound.
    #[stable(feature = "collections_bound", since = "1.17.0")]
    Excluded(T),
    /// An infinite endpoint. Indicates that there is no bound in this direction.
    #[stable(feature = "collections_bound", since = "1.17.0")]
    Unbounded,
}
