//! OS-specific functionality.

#![stable(feature = "os", since = "1.0.0")]
#![allow(missing_docs, nonstandard_style, missing_debug_implementations)]

cfg_if! {
    if #[cfg(rustdoc)] {

        // When documenting libstd we want to show unix/windows/linux modules as
        // these are the "main modules" that are used across platforms. This
        // should help show platform-specific functionality in a hopefully
        // cross-platform way in the documentation

        #[stable(feature = "rust1", since = "1.0.0")]
        pub use sys::unix_ext as unix;

        #[stable(feature = "rust1", since = "1.0.0")]
        pub use sys::windows_ext as windows;

        #[doc(cfg(target_os = "linux"))]
        pub mod linux;
    } else {

        // If we're not documenting libstd then we just expose the main modules
        // as we otherwise would.

        #[cfg(any(target_os = "redox", unix))]
        #[stable(feature = "rust1", since = "1.0.0")]
        pub use sys::ext as unix;

        #[cfg(windows)]
        #[stable(feature = "rust1", since = "1.0.0")]
        pub use sys::ext as windows;

        #[cfg(any(target_os = "linux", target_os = "l4re"))]
        pub mod linux;

    }
}

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "bitrig")]
pub mod bitrig;
#[cfg(target_os = "dragonfly")]
pub mod dragonfly;
#[cfg(target_os = "emscripten")]
pub mod emscripten;
#[cfg(all(target_vendor = "fortanix", target_env = "sgx"))]
pub mod fortanix_sgx;
#[cfg(target_os = "freebsd")]
pub mod freebsd;
#[cfg(target_os = "fuchsia")]
pub mod fuchsia;
#[cfg(target_os = "haiku")]
pub mod haiku;
#[cfg(target_os = "hermit")]
pub mod hermit;
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "netbsd")]
pub mod netbsd;
#[cfg(target_os = "openbsd")]
pub mod openbsd;
#[cfg(target_os = "solaris")]
pub mod solaris;

pub mod raw;
