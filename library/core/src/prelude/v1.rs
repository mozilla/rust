//! The core prelude
//!
//! This module is intended for users of libcore which do not link to libstd as
//! well. This module is imported by default when `#![no_std]` is used in the
//! same manner as the standard library's prelude.

#![stable(feature = "core_prelude", since = "1.4.0")]

// Re-exported core operators
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::marker::{Copy, Send, Sized, Sync, Unpin};
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::ops::{Drop, Fn, FnMut, FnOnce};

// Re-exported functions
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::mem::drop;

// Re-exported types and traits
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::clone::Clone;
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::cmp::{Eq, Ord, PartialEq, PartialOrd};
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::convert::{AsMut, AsRef, From, Into};
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::default::Default;
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::iter::{DoubleEndedIterator, ExactSizeIterator};
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::iter::{Extend, IntoIterator, Iterator};
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::option::Option::{self, None, Some};
#[allow(missing_docs)]
#[stable(feature = "core_prelude", since = "1.4.0")]
#[doc(no_inline)]
pub use crate::result::Result::{self, Err, Ok};

// Re-exported built-in macros
#[allow(missing_docs)]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[doc(no_inline)]
pub use crate::fmt::macros::Debug;
#[allow(missing_docs)]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[doc(no_inline)]
pub use crate::hash::macros::Hash;

#[allow(missing_docs)]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[allow(deprecated)]
#[doc(no_inline)]
pub use crate::{
    asm, assert, cfg, column, compile_error, concat, concat_idents, env, file, format_args,
    format_args_nl, global_asm, include, include_bytes, include_str, line, llvm_asm, log_syntax,
    module_path, option_env, stringify, trace_macros,
};

#[allow(missing_docs)]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[allow(deprecated)]
#[doc(no_inline)]
pub use crate::macros::builtin::{
    bench, global_allocator, test, test_case, RustcDecodable, RustcEncodable,
};

#[allow(missing_docs)]
#[unstable(
    feature = "cfg_accessible",
    issue = "64797",
    reason = "`cfg_accessible` is not fully implemented"
)]
#[doc(no_inline)]
pub use crate::macros::builtin::cfg_accessible;
