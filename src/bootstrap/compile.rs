// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation of compiling various phases of the compiler and standard
//! library.
//!
//! This module contains some of the real meat in the rustbuild build system
//! which is where Cargo is used to compiler the standard library, libtest, and
//! compiler. This module is also responsible for assembling the sysroot as it
//! goes along from the output of the previous stage.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
use std::cmp::min;

use build_helper::{output, up_to_date};

use fs;
use util::{copy, exe, is_dylib, libdir, read_stamp_file};
use {Compiler, Mode};
use native;

use cache::{Intern, Interned};
use builder::{Builder, RunConfig, ShouldRun, Step};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Std {
    pub target: Interned<String>,
    pub compiler: Compiler,
}

impl Step for Std {
    type Output = ();
    const DEFAULT: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.all_krates("std")
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(Std {
            compiler: run.builder.compiler(run.builder.top_stage, run.host),
            target: run.target,
        });
    }

    /// Build the standard library.
    ///
    /// This will build the standard library for a particular stage of the build
    /// using the `compiler` targeting the `target` architecture. The artifacts
    /// created will also be linked into the sysroot directory.
    fn run(self, builder: &Builder) {
        let target = self.target;
        let compiler = self.compiler;

        builder.ensure(StartupObjects { compiler, target });

        if builder.force_use_stage1(compiler, target) {
            let from = builder.compiler(1, builder.config.general.build);
            builder.ensure(Std {
                compiler: from,
                target,
            });
            println!("Uplifting stage1 std ({} -> {})", from.host, target);

            builder.ensure(StdLink {
                compiler: from,
                target_compiler: compiler,
                target,
            });
            return;
        }

        let _folder = builder.fold_output(|| format!("stage{}-std", compiler.stage));
        println!(
            "Building stage{} std artifacts ({} -> {})",
            compiler.stage, &compiler.host, target
        );

        builder.cargo(compiler, Mode::Libstd, target, "build").run();

        builder.ensure(StdLink {
            compiler: builder.compiler(compiler.stage, builder.config.general.build),
            target_compiler: compiler,
            target,
        });
    }
}

/// Copies the crt(1,i,n).o startup objects
///
/// Since musl supports fully static linking, we can cross link for it even
/// with a glibc-targeting toolchain, given we have the appropriate startup
/// files. As those shipped with glibc won't work, copy the ones provided by
/// musl so we have them on linux-gnu hosts.
fn copy_musl_third_party_objects(builder: &Builder, target: Interned<String>, into: &Path) {
    for &obj in &["crt1.o", "crti.o", "crtn.o"] {
        copy(
            &builder.musl_root(target).unwrap().join("lib").join(obj),
            &into.join(obj),
        );
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
struct StdLink {
    pub compiler: Compiler,
    pub target_compiler: Compiler,
    pub target: Interned<String>,
}

impl Step for StdLink {
    type Output = ();

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.never()
    }

    fn for_test(self, _builder: &Builder) {}

    /// Link all libstd rlibs/dylibs into the sysroot location.
    ///
    /// Links those artifacts generated by `compiler` to a the `stage` compiler's
    /// sysroot for the specified `host` and `target`.
    ///
    /// Note that this assumes that `compiler` has already generated the libstd
    /// libraries for `target`, and this method will find them in the relevant
    /// output directory.
    fn run(self, builder: &Builder) {
        let compiler = self.compiler;
        let target_compiler = self.target_compiler;
        let target = self.target;
        println!(
            "Copying stage{} std from stage{} ({} -> {} / {})",
            target_compiler.stage, compiler.stage, &compiler.host, target_compiler.host, target
        );
        let libdir = builder.sysroot_libdir(target_compiler, target);
        add_to_sysroot(&libdir, &builder.libstd_stamp(compiler, target));

        if builder.config.general.sanitizers && compiler.stage != 0
            && target == "x86_64-apple-darwin"
        {
            // The sanitizers are only built in stage1 or above, so the dylibs will
            // be missing in stage0 and causes panic. See the `std()` function above
            // for reason why the sanitizers are not built in stage0.
            copy_apple_sanitizer_dylibs(&builder.native_dir(target), "osx", &libdir);
        }

        if target.contains("musl") {
            let libdir = builder.sysroot_libdir(target_compiler, target);
            copy_musl_third_party_objects(builder, target, &libdir);
        }
    }
}

fn copy_apple_sanitizer_dylibs(native_dir: &Path, platform: &str, into: &Path) {
    for &sanitizer in &["asan", "tsan"] {
        let filename = format!("libclang_rt.{}_{}_dynamic.dylib", sanitizer, platform);
        let mut src_path = native_dir.join(sanitizer);
        src_path.push("build");
        src_path.push("lib");
        src_path.push("darwin");
        src_path.push(&filename);
        copy(&src_path, &into.join(filename));
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct StartupObjects {
    pub compiler: Compiler,
    pub target: Interned<String>,
}

impl Step for StartupObjects {
    type Output = ();

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.path("src/rtstartup")
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(StartupObjects {
            compiler: run.builder.compiler(run.builder.top_stage, run.host),
            target: run.target,
        });
    }

    /// Build and prepare startup objects like rsbegin.o and rsend.o
    ///
    /// These are primarily used on Windows right now for linking executables/dlls.
    /// They don't require any library support as they're just plain old object
    /// files, so we just use the nightly snapshot compiler to always build them (as
    /// no other compilers are guaranteed to be available).
    fn run(self, builder: &Builder) {
        let for_compiler = self.compiler;
        let target = self.target;
        if !target.contains("pc-windows-gnu") {
            return;
        }

        let src_dir = &builder.config.src.join("src/rtstartup");
        let dst_dir = &builder.native_dir(target).join("rtstartup");
        let sysroot_dir = &builder.sysroot_libdir(for_compiler, target);
        t!(fs::create_dir_all(dst_dir));

        for file in &["rsbegin", "rsend"] {
            let src_file = &src_dir.join(file.to_string() + ".rs");
            let dst_file = &dst_dir.join(file.to_string() + ".o");
            if !up_to_date(src_file, dst_file) {
                let mut cmd = Command::new(&builder.config.general.initial_rustc);
                builder.run(
                    cmd.env("RUSTC_BOOTSTRAP", "1")
                        .arg("--cfg")
                        .arg("stage0")
                        .arg("--target")
                        .arg(target)
                        .arg("--emit=obj")
                        .arg("-o")
                        .arg(dst_file)
                        .arg(src_file),
                );
            }

            copy(dst_file, &sysroot_dir.join(file.to_string() + ".o"));
        }

        for obj in ["crt2.o", "dllcrt2.o"].iter() {
            let src = compiler_file(builder, builder.cc(target), target, obj);
            copy(&src, &sysroot_dir.join(obj));
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Test {
    pub target: Interned<String>,
    pub compiler: Compiler,
}

impl Step for Test {
    type Output = ();
    const DEFAULT: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.all_krates("test")
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(Test {
            compiler: run.builder.compiler(run.builder.top_stage, run.host),
            target: run.target,
        });
    }

    /// Build libtest.
    ///
    /// This will build libtest and supporting libraries for a particular stage of
    /// the build using the `compiler` targeting the `target` architecture. The
    /// artifacts created will also be linked into the sysroot directory.
    fn run(self, builder: &Builder) {
        let target = self.target;
        let compiler = self.compiler;

        builder.ensure(Std { compiler, target });

        if builder.force_use_stage1(compiler, target) {
            builder.ensure(Test {
                compiler: builder.compiler(1, builder.config.general.build),
                target,
            });
            println!(
                "Uplifting stage1 test ({} -> {})",
                &builder.config.general.build, target
            );
            builder.ensure(TestLink {
                compiler: builder.compiler(1, builder.config.general.build),
                target_compiler: compiler,
                target,
            });
            return;
        }

        let _folder = builder.fold_output(|| format!("stage{}-test", compiler.stage));
        println!(
            "Building stage{} test artifacts ({} -> {})",
            compiler.stage, &compiler.host, target
        );
        builder.cargo(compiler, Mode::Libtest, target, "build").run();

        builder.ensure(TestLink {
            compiler: builder.compiler(compiler.stage, builder.config.general.build),
            target_compiler: compiler,
            target,
        });
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TestLink {
    pub compiler: Compiler,
    pub target_compiler: Compiler,
    pub target: Interned<String>,
}

impl Step for TestLink {
    type Output = ();

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.never()
    }

    fn for_test(self, _builder: &Builder) {}

    /// Same as `std_link`, only for libtest
    fn run(self, builder: &Builder) {
        let compiler = self.compiler;
        let target_compiler = self.target_compiler;
        let target = self.target;
        println!(
            "Copying stage{} test from stage{} ({} -> {} / {})",
            target_compiler.stage, compiler.stage, &compiler.host, target_compiler.host, target
        );
        add_to_sysroot(
            &builder.sysroot_libdir(target_compiler, target),
            &builder.libtest_stamp(compiler, target),
        );
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rustc {
    pub compiler: Compiler,
    pub target: Interned<String>,
}

impl Step for Rustc {
    type Output = ();
    const ONLY_HOSTS: bool = true;
    const DEFAULT: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.all_krates("rustc-main")
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(Rustc {
            compiler: run.builder.compiler(run.builder.top_stage, run.host),
            target: run.target,
        });
    }

    /// Build the compiler.
    ///
    /// This will build the compiler for a particular stage of the build using
    /// the `compiler` targeting the `target` architecture. The artifacts
    /// created will also be linked into the sysroot directory.
    fn run(self, builder: &Builder) {
        let compiler = self.compiler;
        let target = self.target;

        builder.ensure(Test { compiler, target });

        if builder.force_use_stage1(compiler, target) {
            builder.ensure(Rustc {
                compiler: builder.compiler(1, builder.config.general.build),
                target,
            });
            println!(
                "Uplifting stage1 rustc ({} -> {})",
                &builder.config.general.build, target
            );
            builder.ensure(RustcLink {
                compiler: builder.compiler(1, builder.config.general.build),
                target_compiler: compiler,
                target,
            });
            return;
        }

        // Ensure that build scripts have a std to link against.
        builder.ensure(Std {
            compiler: builder.compiler(self.compiler.stage, builder.config.general.build),
            target: builder.config.general.build,
        });

        let _folder = builder.fold_output(|| format!("stage{}-rustc", compiler.stage));
        println!(
            "Building stage{} compiler artifacts ({} -> {})",
            compiler.stage, &compiler.host, target
        );

        builder.cargo(compiler, Mode::Librustc, target, "build").run();

        builder.ensure(RustcLink {
            compiler: builder.compiler(compiler.stage, builder.config.general.build),
            target_compiler: compiler,
            target,
        });
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
struct RustcLink {
    pub compiler: Compiler,
    pub target_compiler: Compiler,
    pub target: Interned<String>,
}

impl Step for RustcLink {
    type Output = ();

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.never()
    }

    fn for_test(self, _builder: &Builder) {}

    /// Same as `std_link`, only for librustc
    fn run(self, builder: &Builder) {
        let compiler = self.compiler;
        let target_compiler = self.target_compiler;
        let target = self.target;
        println!(
            "Copying stage{} rustc from stage{} ({} -> {} / {})",
            target_compiler.stage, compiler.stage, &compiler.host, target_compiler.host, target
        );
        add_to_sysroot(
            &builder.sysroot_libdir(target_compiler, target),
            &builder.librustc_stamp(compiler, target),
        );
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct CodegenBackend {
    pub compiler: Compiler,
    pub target: Interned<String>,
    pub backend: Interned<String>,
}

impl Step for CodegenBackend {
    type Output = ();
    const ONLY_HOSTS: bool = true;
    const DEFAULT: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.all_krates("rustc_trans")
    }

    fn make_run(run: RunConfig) {
        let backend = run.builder.config.rust.codegen_backends.get(0);
        let backend = backend.cloned().unwrap_or_else(|| String::from("llvm"));
        let backend = backend.intern();
        run.builder.ensure(CodegenBackend {
            compiler: run.builder.compiler(run.builder.top_stage, run.host),
            target: run.target,
            backend,
        });
    }

    fn run(self, builder: &Builder) {
        let compiler = self.compiler;
        let target = self.target;

        builder.ensure(Rustc { compiler, target });

        if builder.force_use_stage1(compiler, target) {
            builder.ensure(CodegenBackend {
                compiler: builder.compiler(1, builder.config.general.build),
                target,
                backend: self.backend,
            });
            return;
        }

        let mut cargo = builder.cargo(
            compiler, Mode::CodegenBackend(self.backend), target, "build");

        match &*self.backend {
            "llvm" | "emscripten" => {
                // Build LLVM for our target. This will implicitly build the
                // host LLVM if necessary.
                let llvm_config = builder.ensure(native::Llvm {
                    target,
                    emscripten: self.backend == "emscripten",
                });

                let _folder =
                    builder.fold_output(|| format!("stage{}-rustc_trans", compiler.stage));
                println!(
                    "Building stage{} codegen artifacts ({} -> {}, {})",
                    compiler.stage, &compiler.host, target, self.backend
                );

                // Pass down configuration from the LLVM build into the build of
                // librustc_llvm and librustc_trans.
                if builder.is_rust_llvm(target) {
                    cargo.env("LLVM_RUSTLLVM", "1");
                }
                cargo.env("LLVM_CONFIG", &llvm_config);
                if self.backend != "emscripten" {
                    let target_config = builder.config.target_config.get(&target);
                    if let Some(s) = target_config.and_then(|c| c.llvm_config.as_ref()) {
                        cargo.env("CFG_LLVM_ROOT", s);
                    }
                }
                // Building with a static libstdc++ is only supported on linux right now,
                // not for MSVC or macOS
                if builder.config.llvm.static_libstdcpp && !target.contains("freebsd")
                    && !target.contains("windows") && !target.contains("apple")
                {
                    let file =
                        compiler_file(builder, builder.cxx(target).unwrap(), target, "libstdc++.a");
                    cargo.env("LLVM_STATIC_STDCPP", file);
                }
                if builder.config.llvm.link_shared {
                    cargo.env("LLVM_LINK_SHARED", "1");
                }
            }
            _ => panic!("unknown backend: {}", self.backend),
        }

        cargo.run();
    }
}

/// Creates the `codegen-backends` folder for a compiler that's about to be
/// assembled as a complete compiler.
///
/// This will take the codegen artifacts produced by `compiler` and link them
/// into an appropriate location for `target_compiler` to be a functional
/// compiler.
fn copy_codegen_backends_to_sysroot(
    builder: &Builder,
    compiler: Compiler,
    target_compiler: Compiler,
) {
    let target = target_compiler.host;

    // Note that this step is different than all the other `*Link` steps in
    // that it's not assembling a bunch of libraries but rather is primarily
    // moving the codegen backend into place. The codegen backend of rustc is
    // not linked into the main compiler by default but is rather dynamically
    // selected at runtime for inclusion.
    //
    // Here we're looking for the output dylib of the `CodegenBackend` step and
    // we're copying that into the `codegen-backends` folder.
    let dst = builder.sysroot_codegen_backends(target_compiler);
    t!(fs::create_dir_all(&dst));

    for backend in builder.config.rust.codegen_backends.iter() {
        let stamp = builder.codegen_backend_stamp(compiler, target, &backend);
        let mut saw_backend: Option<PathBuf> = None;
        for path in read_stamp_file(&stamp) {
            let filename = path.file_name().unwrap().to_str().unwrap();
            if is_dylib(filename) && filename.contains("rustc_trans-") {
                if let Some(past) = saw_backend {
                    panic!("found two codegen backends:\n{}\n{}",
                        path.display(),
                        past.display());
                }
                // change `librustc_trans-xxxxxx.so` to `librustc_trans-llvm.so`
                let target_filename = {
                    let dash = filename.find("-").unwrap();
                    let dot = filename.find(".").unwrap();
                    format!("{}-{}{}", &filename[..dash], backend, &filename[dot..])
                };
                copy(&path, &dst.join(target_filename));
                saw_backend = Some(path.clone());
            }
        }
    }
}

fn compiler_file(
    builder: &Builder,
    compiler: &Path,
    target: Interned<String>,
    file: &str,
) -> PathBuf {
    let mut cmd = Command::new(compiler);
    cmd.args(builder.cflags(target));
    cmd.arg(format!("-print-file-name={}", file));
    let out = output(&mut cmd);
    PathBuf::from(out.trim())
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Sysroot {
    pub compiler: Compiler,
}

impl Step for Sysroot {
    type Output = Interned<PathBuf>;

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.never()
    }

    /// Returns the sysroot for the `compiler` specified that *this build system
    /// generates*.
    ///
    /// That is, the sysroot for the stage0 compiler is not what the compiler
    /// thinks it is by default, but it's the same as the default for stages
    /// 1-3.
    fn run(self, builder: &Builder) -> Interned<PathBuf> {
        let compiler = self.compiler;
        let sysroot = if compiler.stage == 0 {
            builder
                .config
                .general
                .out
                .join(&compiler.host)
                .join("stage0-sysroot")
        } else {
            builder
                .config
                .general
                .out
                .join(&compiler.host)
                .join(format!("stage{}", compiler.stage))
        };
        let _ = fs::remove_dir_all(&sysroot);
        t!(fs::create_dir_all(&sysroot));
        sysroot.intern()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Assemble {
    /// The compiler which we will produce in this step. Assemble itself will
    /// take care of ensuring that the necessary prerequisites to do so exist,
    /// that is, this target can be a stage2 compiler and Assemble will build
    /// previous stages for you.
    pub target_compiler: Compiler,
}

impl Step for Assemble {
    type Output = Compiler;

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.all_krates("rustc-main")
    }

    /// Prepare a new compiler from the artifacts in `stage`
    ///
    /// This will assemble a compiler in `build/$host/stage$stage`. The compiler
    /// must have been previously produced by the `stage - 1` builder.build
    /// compiler.
    fn run(self, builder: &Builder) -> Compiler {
        let target_compiler = self.target_compiler;

        if target_compiler.stage == 0 {
            assert_eq!(
                builder.config.general.build, target_compiler.host,
                "Cannot obtain compiler for non-native build triple at stage 0"
            );
            // The stage 0 compiler for the build triple is always pre-built.
            return target_compiler;
        }

        // Get the compiler that we'll use to bootstrap ourselves.
        //
        // Note that this is where the recursive nature of the bootstrap
        // happens, as this will request the previous stage's compiler on
        // downwards to stage 0.
        //
        // Also note that we're building a compiler for the host platform. We
        // only assume that we can run `build` artifacts, which means that to
        // produce some other architecture compiler we need to start from
        // `build` to get there.
        //
        // FIXME: Perhaps we should download those libraries?
        //        It would make builds faster...
        //
        // FIXME: It may be faster if we build just a stage 1 compiler and then
        //        use that to bootstrap this compiler forward.
        let build_compiler =
            builder.compiler(target_compiler.stage - 1, builder.config.general.build);

        // Build the libraries for this compiler to link to (i.e., the libraries
        // it uses at runtime). NOTE: Crates the target compiler compiles don't
        // link to these. (FIXME: Is that correct? It seems to be correct most
        // of the time but I think we do link to these for stage2/bin compilers
        // when not performing a full bootstrap).
        if builder
            .build
            .config
            .keep_stage
            .map_or(false, |s| target_compiler.stage <= s)
        {
            builder.verbose("skipping compilation of compiler due to --keep-stage");
            let compiler = build_compiler;
            for stage in 0..min(target_compiler.stage, builder.config.keep_stage.unwrap()) {
                let target_compiler = builder.compiler(stage, target_compiler.host);
                let target = target_compiler.host;
                builder.ensure(StdLink {
                    compiler,
                    target_compiler,
                    target,
                });
                builder.ensure(TestLink {
                    compiler,
                    target_compiler,
                    target,
                });
                builder.ensure(RustcLink {
                    compiler,
                    target_compiler,
                    target,
                });
            }
        } else {
            builder.ensure(Rustc {
                compiler: build_compiler,
                target: target_compiler.host,
            });
            for backend in builder.config.rust.codegen_backends.iter() {
                builder.ensure(CodegenBackend {
                    compiler: build_compiler,
                    target: target_compiler.host,
                    backend: backend.intern(),
                });
            }
        }

        let stage = target_compiler.stage;
        let host = target_compiler.host;
        println!("Assembling stage{} compiler ({})", stage, host);

        // Link in all dylibs to the libdir
        let sysroot = builder.sysroot(target_compiler);
        let sysroot_libdir = sysroot.join(libdir(&*host));
        t!(fs::create_dir_all(&sysroot_libdir));
        let src_libdir = builder.sysroot_libdir(build_compiler, host);
        for f in t!(fs::read_dir(&src_libdir)).map(|f| t!(f)) {
            let filename = f.file_name().into_string().unwrap();
            if is_dylib(&filename) {
                copy(&f.path(), &sysroot_libdir.join(&filename));
            }
        }

        copy_codegen_backends_to_sysroot(builder, build_compiler, target_compiler);

        // Link the compiler binary itself into place
        let out_dir = builder.cargo_out(build_compiler, Mode::Librustc, host);
        let rustc = out_dir.join(exe("rustc", &*host));
        let bindir = sysroot.join("bin");
        t!(fs::create_dir_all(&bindir));
        let compiler = builder.rustc(target_compiler);
        let _ = fs::remove_file(&compiler);
        copy(&rustc, &compiler);

        target_compiler
    }
}

/// Link some files into a rustc sysroot.
///
/// For a particular stage this will link the file listed in `stamp` into the
/// `sysroot_dst` provided.
pub fn add_to_sysroot(sysroot_dst: &Path, stamp: &Path) {
    t!(fs::create_dir_all(&sysroot_dst));
    for path in read_stamp_file(stamp) {
        copy(&path, &sysroot_dst.join(path.file_name().unwrap()));
    }
}
