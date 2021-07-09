//! For introspecting and converting between fieldless enums and numbers.

/// Converts an enum to its underlying repr.
///
/// After the next beta is cut (to work around bootstrapping issues), this type will be automatically
/// implemented for all data-free enum types with an explicit repr.
///
/// # Safety
///
/// This trait must only be implemented for types which are transmutable to their repr. Callers may
/// assume that it is safe to transmute an instance of `AsRepr` into its associated `Repr`
/// type.
#[unstable(feature = "enum_as_repr", issue = "86772")]
pub unsafe trait AsRepr: Sized {
    /// The underlying repr type of the enum.
    type Repr;

    /// Convert the enum to its underlying repr.
    fn as_repr(&self) -> Self::Repr {
        // SAFETY: Guaranteed to be safe from the safety constraints of the unsafe trait itself.
        let value = unsafe { crate::mem::transmute_copy(self) };
        drop(self);
        value
    }
}

/// Derive macro generating an impl of the trait `AsRepr` for enums.
///
/// Note that (after the next beta is cut, to work around bootstrapping issues), this is
/// automatically derived for all enum types with an explicit repr, and does not need to be manually
/// derived.
#[cfg(not(bootstrap))]
#[rustc_builtin_macro]
#[unstable(feature = "enum_as_repr", issue = "86772")]
pub macro AsRepr($item:item) {
    /* compiler built-in */
}

/// Converts an enum from its underlying repr.
///
/// # Safety
///
/// This trait must only be implemented for types which are transmutable from their repr for
/// inhabited repr values. Callers may assume that it is safe to transmute an instance of `Repr`
/// into its associated `Repr` type if that value is inhabited for this type.
#[unstable(feature = "enum_as_repr", issue = "86772")]
pub unsafe trait FromRepr: AsRepr {
    /// Tries to convert an enum from its underlying repr type.
    fn try_from_repr(from: Self::Repr) -> Result<Self, TryFromReprError<Self::Repr>>;

    /// Converts from the enum's underlying repr type to this enum.
    ///
    /// # Safety
    ///
    /// This is only safe to call if it is known that the value being converted has a matching
    /// variant of this enum. Attempting to convert a value which doesn't correspond to an enum
    /// variant causes undefined behavior.
    unsafe fn from_repr(from: Self::Repr) -> Self {
        // SAFETY: Guaranteed to be safe from the safety constraints of the unsafe trait itself.
        let value = unsafe { crate::mem::transmute_copy(&from) };
        drop(from);
        value
    }
}

/// Derive macro generating an impl of the trait `FromRepr` for enums.
#[cfg(not(bootstrap))]
#[rustc_builtin_macro]
#[unstable(feature = "enum_as_repr", issue = "86772")]
pub macro FromRepr($item:item) {
    /* compiler built-in */
}

/// The error type returned when a checked integral type conversion fails.
/// Ideally this would be the same as `core::num::TryFromIntError` but it's not publicly
/// constructable.
#[unstable(feature = "enum_as_repr", issue = "86772")]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TryFromReprError<T: Sized>(pub T);
