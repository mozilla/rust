// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! MSVC-specific logic for linkers and such.
//!
//! This module contains a cross-platform interface but has a blank unix
//! implementation. The Windows implementation builds on top of Windows native
//! libraries (reading registry keys), so it otherwise wouldn't link on unix.
//!
//! Note that we don't have much special logic for finding the system linker on
//! any other platforms, so it may seem a little odd to single out MSVC to have
//! a good deal of code just to find the linker. Unlike Unix systems, however,
//! the MSVC linker is not in the system PATH by default. It also additionally
//! needs a few environment variables or command line flags to be able to link
//! against system libraries.
//!
//! In order to have a nice smooth experience on Windows, the logic in this file
//! is here to find the MSVC linker and set it up in the default configuration
//! one would need to set up anyway. This means that the Rust compiler can be
//! run not only in the developer shells of MSVC but also the standard cmd.exe
//! shell or MSYS shells.
//!
//! As a high-level note, all logic in this module for looking up various
//! paths/files is based on Microsoft's logic in their vcvars bat files, but
//! comments can also be found below leading through the various code paths.

#[cfg(windows)]
mod registry;
#[cfg(windows)]
mod arch;

#[cfg(windows)]
mod platform {
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use session::Session;
    use super::arch::{host_arch, Arch};
    use super::registry::LOCAL_MACHINE;

    // First we need to figure out whether the environment is already correctly
    // configured by vcvars. We do this by looking at the environment variable
    // `VCINSTALLDIR` which is always set by vcvars, and unlikely to be set
    // otherwise. If it is defined, then we find `link.exe` in `PATH and trust
    // that everything else is configured correctly.
    //
    // If `VCINSTALLDIR` wasn't defined (or we couldn't find the linker where
    // it claimed it should be), then we resort to finding everything
    // ourselves. First we find where the latest version of MSVC is installed
    // and what version it is. Then based on the version we find the
    // appropriate SDKs.
    //
    // For versions are are unable to detect the user has to execute the
    // appropriate vcvars bat file themselves to configure the environment.
    //
    // If despite our best efforts we are still unable to find MSVC then we
    // just blindly call `link.exe` and hope for the best.
    pub fn link_exe_cmd(sess: &Session) -> (Command, Option<PathBuf>) {
        let arch = &sess.target.target.arch;
        env::var_os("VCINSTALLDIR").and_then(|_| {
            debug!("Detected that vcvars was already run.");
            env::var_os("PATH").and_then(|path| {
                // Mingw has its own link which is not the linkwe want so we
                // look for `cl.exe` too as a precaution.
                env::split_paths(&path).find(|path| {
                    path.join("cl.exe").is_file()
                        && path.join("link.exe").is_file()
                }).map(|path| {
                    (Command::new(path.join("link.exe")), None)
                })
            })
        }).or_else(|| {
            find_msvc_14(arch).or_else(|| {
                find_msvc_12(arch)
            }).or_else(|| {
                find_msvc_11(arch)
            }).map(|(cmd, path)| (cmd, Some(path)))
        }).unwrap_or_else(|| {
            debug!("Failed to locate linker.");
            (Command::new("link.exe"), None)
        })
    }

    // For MSVC 14 we need to find the Universal CRT as well as either the
    // Windows 10 SDK or Windows 8.1 SDK.
    fn find_msvc_14(arch: &str) -> Option<(Command, PathBuf)> {
        get_vc_dir("14.0").and_then(|path| {
            get_linker(&path, arch).into_iter().zip(lib_subdir(arch)).next()
        }).and_then(|((mut cmd, host), sub)| {
            get_ucrt_dir().map(|dir| {
                debug!("Found Universal CRT {:?}", dir);
                add_lib(&mut cmd, &dir.join("ucrt").join(sub));
            }).and_then(|_| {
                get_sdk10_dir().map(|dir| {
                    debug!("Found Win10 SDK {:?}", dir);
                    add_lib(&mut cmd, &dir.join("um").join(sub));
                }).or_else(|| {
                    get_sdk81_dir().map(|dir| {
                        debug!("Found Win8.1 SDK {:?}", dir);
                        add_lib(&mut cmd, &dir.join("um").join(sub));
                    })
                })
            }).map(move |_| (cmd, host))
        })
    }

    // For MSVC 12 we only need to find the Windows 8.1 SDK.
    fn find_msvc_12(arch: &str) -> Option<(Command, PathBuf)> {
        get_vc_dir("12.0").and_then(|path| {
            get_linker(&path, arch).into_iter().zip(lib_subdir(arch)).next()
        }).and_then(|((mut cmd, host), sub)| {
            get_sdk81_dir().map(|dir| {
                debug!("Found Win8.1 SDK {:?}", dir);
                add_lib(&mut cmd, &dir.join("um").join(sub));
            }).map(move |_| (cmd, host))
        })
    }

    // For MSVC 11 we only need to find the Windows 8 SDK.
    fn find_msvc_11(arch: &str) -> Option<(Command, PathBuf)> {
        get_vc_dir("11.0").and_then(|path| {
            get_linker(&path, arch).into_iter().zip(lib_subdir(arch)).next()
        }).and_then(|((mut cmd, host), sub)| {
            get_sdk8_dir().map(|dir| {
                debug!("Found Win8 SDK {:?}", dir);
                add_lib(&mut cmd, &dir.join("um").join(sub));
            }).map(move |_| (cmd, host))
        })
    }

    // A convenience function to append library paths.
    fn add_lib(cmd: &mut Command, lib: &Path) {
        let mut arg: OsString = "/LIBPATH:".into();
        arg.push(lib);
        cmd.arg(arg);
    }

    // Given a possible MSVC installation directory, we look for the linker and
    // then add the MSVC library path.
    fn get_linker(path: &Path, arch: &str) -> Option<(Command, PathBuf)> {
        debug!("Looking for linker in {:?}", path);
        bin_subdir(arch).into_iter().map(|(sub, host)| {
            (path.join("bin").join(sub).join("link.exe"),
             path.join("bin").join(host))
        }).filter(|&(ref path, _)| {
            path.is_file()
        }).map(|(path, host)| {
            (Command::new(path), host)
        }).filter_map(|(mut cmd, host)| {
            vc_lib_subdir(arch).map(move |sub| {
                add_lib(&mut cmd, &path.join("lib").join(sub));
                (cmd, host)
            })
        }).next()
    }

    // To find MSVC we look in a specific registry key for the version we are
    // trying to find.
    fn get_vc_dir(ver: &str) -> Option<PathBuf> {
        LOCAL_MACHINE.open(r"SOFTWARE\Microsoft\VisualStudio\SxS\VC7".as_ref())
        .ok().and_then(|key| {
            key.query_str(ver).ok()
        }).map(|path| {
            path.into()
        })
    }

    // To find the Universal CRT we look in a specific registry key for where
    // all the Universal CRTs are located and then sort them asciibetically to
    // find the newest version. While this sort of sorting isn't ideal,  it is
    // what vcvars does so that's good enough for us.
    fn get_ucrt_dir() -> Option<PathBuf> {
        LOCAL_MACHINE.open(r"SOFTWARE\Microsoft\Windows Kits\Installed Roots".as_ref())
        .ok().and_then(|key| {
            key.query_str("KitsRoot10").ok()
        }).and_then(|root| {
            fs::read_dir(Path::new(&root).join("lib")).ok()
        }).and_then(|readdir| {
            readdir.filter_map(|dir| {
                dir.ok()
            }).map(|dir| {
                dir.path()
            }).filter(|dir| {
                dir.components().last().and_then(|c| {
                    c.as_os_str().to_str()
                }).map(|c| c.starts_with("10.")).unwrap_or(false)
            }).max()
        })
    }

    // Vcvars finds the correct version of the Windows 10 SDK by looking
    // for the include `um\Windows.h` because sometimes a given version will
    // only have UCRT bits without the rest of the SDK. Since we only care about
    // libraries and not includes, we instead look for `um\x64\kernel32.lib`.
    // Like we do for the Universal CRT, we sort the possibilities
    // asciibetically to find the newest one as that is what vcvars does.
    fn get_sdk10_dir() -> Option<PathBuf> {
        LOCAL_MACHINE.open(r"SOFTWARE\Microsoft\Microsoft SDKs\Windows\v10.0".as_ref())
        .ok().and_then(|key| {
            key.query_str("InstallationFolder").ok()
        }).and_then(|root| {
            fs::read_dir(Path::new(&root).join("lib")).ok()
        }).and_then(|readdir| {
            let mut dirs: Vec<_> = readdir.filter_map(|dir| dir.ok())
                .map(|dir| dir.path()).collect();
            dirs.sort();
            dirs.into_iter().rev().filter(|dir| {
                dir.join("um").join("x64").join("kernel32.lib").is_file()
            }).next()
        })
    }

    // Interestingly there are several subdirectories, `win7` `win8` and
    // `winv6.3`. Vcvars seems to only care about `winv6.3` though, so the same
    // applies to us. Note that if we were targetting kernel mode drivers
    // instead of user mode applications, we would care.
    fn get_sdk81_dir() -> Option<PathBuf> {
        LOCAL_MACHINE.open(r"SOFTWARE\Microsoft\Microsoft SDKs\Windows\v8.1".as_ref())
        .ok().and_then(|key| {
            key.query_str("InstallationFolder").ok()
        }).map(|root| {
            Path::new(&root).join("lib").join("winv6.3")
        })
    }

    fn get_sdk8_dir() -> Option<PathBuf> {
        LOCAL_MACHINE.open(r"SOFTWARE\Microsoft\Microsoft SDKs\Windows\v8.0".as_ref())
        .ok().and_then(|key| {
            key.query_str("InstallationFolder").ok()
        }).map(|root| {
            Path::new(&root).join("lib").join("win8")
        })
    }

    // When choosing the linker toolchain to use, we have to choose the one
    // which matches the host architecture. Otherwise we end up in situations
    // where someone on 32-bit Windows is trying to cross compile to 64-bit and
    // it tries to invoke the native 64-bit linker which won't work.
    //
    // FIXME - Figure out what happens when the host architecture is arm.
    fn bin_subdir(arch: &str) -> Vec<(&'static str, &'static str)> {
        if let Some(host) = host_arch() {
            match (arch, host) {
                ("x86", Arch::X86) => vec![("", "")],
                ("x86", Arch::Amd64) => vec![("amd64_x86", "amd64"), ("", "")],
                ("x86_64", Arch::X86) => vec![("x86_amd64", "")],
                ("x86_64", Arch::Amd64) => vec![("amd64", "amd64"), ("x86_amd64", "")],
                ("arm", Arch::X86) => vec![("x86_arm", "")],
                ("arm", Arch::Amd64) => vec![("amd64_arm", "amd64"), ("x86_arm", "")],
                _ => vec![],
            }
        } else { vec![] }
    }

    fn lib_subdir(arch: &str) -> Option<&'static str> {
        match arch {
            "x86" => Some("x86"),
            "x86_64" => Some("x64"),
            "arm" => Some("arm"),
            _ => None,
        }
    }

    // MSVC's x86 libraries are not in a subfolder
    fn vc_lib_subdir(arch: &str) -> Option<&'static str> {
        match arch {
            "x86" => Some(""),
            "x86_64" => Some("amd64"),
            "arm" => Some("arm"),
            _ => None,
        }
    }
}

// If we're not on Windows, then there's no registry to search through and MSVC
// wouldn't be able to run, so we just call `link.exe` and hope for the best.
#[cfg(not(windows))]
mod platform {
    use std::path::PathBuf;
    use std::process::Command;
    use session::Session;
    pub fn link_exe_cmd(_sess: &Session) -> (Command, Option<PathBuf>) {
        (Command::new("link.exe"), None)
    }
}

pub use self::platform::*;
