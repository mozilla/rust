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
