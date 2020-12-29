use crate::ops::{self, Try};

/// Used to tell an operation whether it should exit early or go on as usual.
///
/// This is used when exposing things (like graph traversals or visitors) where
/// you want the user to be able to choose whether to exit early.
/// Having the enum makes it clearer -- no more wondering "wait, what did `false`
/// mean again?" -- and allows including a value.
///
/// # Examples
///
/// Early-exiting from [`Iterator::try_for_each`]:
/// ```
/// #![feature(control_flow_enum)]
/// use std::ops::ControlFlow;
///
/// let r = (2..100).try_for_each(|x| {
///     if 403 % x == 0 {
///         return ControlFlow::Break(x)
///     }
///
///     ControlFlow::Continue(())
/// });
/// assert_eq!(r, ControlFlow::Break(13));
/// ```
///
/// A basic tree traversal:
/// ```no_run
/// #![feature(control_flow_enum)]
/// use std::ops::ControlFlow;
///
/// pub struct TreeNode<T> {
///     value: T,
///     left: Option<Box<TreeNode<T>>>,
///     right: Option<Box<TreeNode<T>>>,
/// }
///
/// impl<T> TreeNode<T> {
///     pub fn traverse_inorder<B>(&self, mut f: impl FnMut(&T) -> ControlFlow<B>) -> ControlFlow<B> {
///         if let Some(left) = &self.left {
///             left.traverse_inorder(&mut f)?;
///         }
///         f(&self.value)?;
///         if let Some(right) = &self.right {
///             right.traverse_inorder(&mut f)?;
///         }
///         ControlFlow::Continue(())
///     }
/// }
/// ```
#[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlFlow<B, C = ()> {
    /// Move on to the next phase of the operation as normal.
    #[cfg_attr(not(bootstrap), lang = "Continue")]
    Continue(C),
    /// Exit the operation without running subsequent phases.
    #[cfg_attr(not(bootstrap), lang = "Break")]
    Break(B),
    // Yes, the order of the variants doesn't match the type parameters.
    // They're in this order so that `ControlFlow<A, B>` <-> `Result<B, A>`
    // is a no-op conversion in the `Try` implementation.
}

#[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
impl<B, C> ops::Try2015 for ControlFlow<B, C> {
    type Ok = C;
    type Error = B;
    #[inline]
    fn into_result(self) -> Result<Self::Ok, Self::Error> {
        match self {
            ControlFlow::Continue(y) => Ok(y),
            ControlFlow::Break(x) => Err(x),
        }
    }
    #[inline]
    fn from_error(v: Self::Error) -> Self {
        ControlFlow::Break(v)
    }
    #[inline]
    fn from_ok(v: Self::Ok) -> Self {
        ControlFlow::Continue(v)
    }
}

#[unstable(feature = "try_trait_v2", issue = "42327")]
impl<B, C> ops::Bubble for ControlFlow<B, C> {
    //type Continue = C;
    type Ok = C;
    type Holder = ControlFlow<B, !>;
    #[inline]
    fn continue_with(c: C) -> Self {
        ControlFlow::Continue(c)
    }
    #[inline]
    fn branch(self) -> ControlFlow<Self::Holder, C> {
        match self {
            ControlFlow::Continue(c) => ControlFlow::Continue(c),
            ControlFlow::Break(b) => ControlFlow::Break(ControlFlow::Break(b)),
        }
    }
}

#[unstable(feature = "try_trait_v2", issue = "42327")]
impl<C, B> ops::BreakHolder<C> for ControlFlow<B, !> {
    type Output = ControlFlow<B, C>;
    // fn expand(x: Self) -> Self::Output {
    //     match x {
    //         ControlFlow::Break(b) => ControlFlow::Break(b),
    //     }
    // }
}

#[unstable(feature = "try_trait_v2", issue = "42327")]
impl<B, C> ops::Try2021 for ControlFlow<B, C> {
    fn from_holder(x: Self::Holder) -> Self {
        match x {
            ControlFlow::Break(b) => ControlFlow::Break(b),
        }
    }
}

impl<B, C> ControlFlow<B, C> {
    /// Returns `true` if this is a `Break` variant.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(control_flow_enum)]
    /// use std::ops::ControlFlow;
    ///
    /// assert!(ControlFlow::<i32, String>::Break(3).is_break());
    /// assert!(!ControlFlow::<String, i32>::Continue(3).is_break());
    /// ```
    #[inline]
    #[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
    pub fn is_break(&self) -> bool {
        matches!(*self, ControlFlow::Break(_))
    }

    /// Returns `true` if this is a `Continue` variant.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(control_flow_enum)]
    /// use std::ops::ControlFlow;
    ///
    /// assert!(!ControlFlow::<i32, String>::Break(3).is_continue());
    /// assert!(ControlFlow::<String, i32>::Continue(3).is_continue());
    /// ```
    #[inline]
    #[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
    pub fn is_continue(&self) -> bool {
        matches!(*self, ControlFlow::Continue(_))
    }

    /// Converts the `ControlFlow` into an `Option` which is `Some` if the
    /// `ControlFlow` was `Break` and `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(control_flow_enum)]
    /// use std::ops::ControlFlow;
    ///
    /// assert_eq!(ControlFlow::<i32, String>::Break(3).break_value(), Some(3));
    /// assert_eq!(ControlFlow::<String, i32>::Continue(3).break_value(), None);
    /// ```
    #[inline]
    #[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
    pub fn break_value(self) -> Option<B> {
        match self {
            ControlFlow::Continue(..) => None,
            ControlFlow::Break(x) => Some(x),
        }
    }

    /// Maps `ControlFlow<B, C>` to `ControlFlow<T, C>` by applying a function
    /// to the break value in case it exists.
    #[inline]
    #[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
    pub fn map_break<T, F>(self, f: F) -> ControlFlow<T, C>
    where
        F: FnOnce(B) -> T,
    {
        match self {
            ControlFlow::Continue(x) => ControlFlow::Continue(x),
            ControlFlow::Break(x) => ControlFlow::Break(f(x)),
        }
    }
}

#[cfg(bootstrap)]
impl<R: Try> ControlFlow<R, R::Ok> {
    /// Create a `ControlFlow` from any type implementing `Try`.
    #[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
    #[inline]
    pub fn from_try(r: R) -> Self {
        match Try::into_result(r) {
            Ok(v) => ControlFlow::Continue(v),
            Err(v) => ControlFlow::Break(Try::from_error(v)),
        }
    }

    /// Convert a `ControlFlow` into any type implementing `Try`;
    #[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
    #[inline]
    pub fn into_try(self) -> R {
        match self {
            ControlFlow::Continue(v) => Try::from_ok(v),
            ControlFlow::Break(v) => v,
        }
    }
}

#[cfg(not(bootstrap))]
impl<R: Try> ControlFlow<R, R::Ok> {
    /// Create a `ControlFlow` from any type implementing `Try`.
    #[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
    #[inline]
    pub fn from_try(r: R) -> Self {
        match r.branch() {
            ControlFlow::Continue(v) => ControlFlow::Continue(v),
            ControlFlow::Break(h) => ControlFlow::Break(R::from_holder(h)),
        }
    }

    /// Convert a `ControlFlow` into any type implementing `Try`;
    #[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
    #[inline]
    pub fn into_try(self) -> R {
        match self {
            ControlFlow::Continue(v) => R::continue_with(v),
            ControlFlow::Break(v) => v,
        }
    }
}

impl<B> ControlFlow<B, ()> {
    /// It's frequently the case that there's no value needed with `Continue`,
    /// so this provides a way to avoid typing `(())`, if you prefer it.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(control_flow_enum)]
    /// use std::ops::ControlFlow;
    ///
    /// let mut partial_sum = 0;
    /// let last_used = (1..10).chain(20..25).try_for_each(|x| {
    ///     partial_sum += x;
    ///     if partial_sum > 100 { ControlFlow::Break(x) }
    ///     else { ControlFlow::CONTINUE }
    /// });
    /// assert_eq!(last_used.break_value(), Some(22));
    /// ```
    #[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
    pub const CONTINUE: Self = ControlFlow::Continue(());
}

impl<C> ControlFlow<(), C> {
    /// APIs like `try_for_each` don't need values with `Break`,
    /// so this provides a way to avoid typing `(())`, if you prefer it.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(control_flow_enum)]
    /// use std::ops::ControlFlow;
    ///
    /// let mut partial_sum = 0;
    /// (1..10).chain(20..25).try_for_each(|x| {
    ///     if partial_sum > 100 { ControlFlow::BREAK }
    ///     else { partial_sum += x; ControlFlow::CONTINUE }
    /// });
    /// assert_eq!(partial_sum, 108);
    /// ```
    #[unstable(feature = "control_flow_enum", reason = "new API", issue = "75744")]
    pub const BREAK: Self = ControlFlow::Break(());
}
