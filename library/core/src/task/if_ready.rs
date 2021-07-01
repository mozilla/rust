/// Extracts the successful type of a [`Poll<T>`].
///
/// This macro bakes in propagation of [`Pending`] signals by returning early.
///
/// [`Poll<T>`]: crate::task::Poll
/// [`Pending`]: crate::task::Poll::Pending
///
/// # Examples
///
/// ```
/// use std::task::{if_ready, Context, Poll};
/// use std::future::{self, Future};
/// use std::pin::Pin;
///
/// pub fn do_poll(cx: &mut Context<'_>) -> Poll<()> {
///     let mut fut = future::ready(42);
///     let fut = Pin::new(&mut fut);
///
///     let num = if_ready!(fut.poll(cx));
///     # drop(num);
///     // ... use num
///
///     Poll::Ready(())
/// }
/// ```
///
/// The `if_ready!` call expands to:
///
/// ```
/// # use std::task::{Context, Poll};
/// # use std::future::{self, Future};
/// # use std::pin::Pin;
/// #
/// # pub fn do_poll(cx: &mut Context<'_>) -> Poll<()> {
///     # let mut fut = future::ready(42);
///     # let fut = Pin::new(&mut fut);
///     #
/// let num = match fut.poll(cx) {
///     Poll::Ready(t) => t,
///     Poll::Pending => return Poll::Pending,
/// };
///     # drop(num);
///     # // ... use num
///     #
///     # Poll::Ready(())
/// # }
/// ```
#[stable(feature = "ready_macro", since = "1.55.0")]
#[rustc_macro_transparency = "semitransparent"]
pub macro if_ready($e:expr) {
    match $e {
        $crate::task::Poll::Ready(t) => t,
        $crate::task::Poll::Pending => {
            return $crate::task::Poll::Pending;
        }
    }
}
