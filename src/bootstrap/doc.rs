// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Documentation generation for rustbuilder.
//!
//! This module implements generation for all bits and pieces of documentation
//! for the Rust project. This notably includes suites like the rust book, the
//! nomicon, rust by example, standalone documentation, etc.
//!
//! Everything here is basically just a shim around calling either `rustbook` or
//! `rustdoc`.

use std::io;
use std::path::{Path, PathBuf};

use Mode;
use build_helper::up_to_date;

use fs;
use util::{cp_r, symlink_dir};
use builder::{Builder, Compiler, RunConfig, ShouldRun, Step};
use tool::Tool;
use compile;
use cache::{Intern, Interned};

macro_rules! book {
    ($($name:ident, $path:expr, $book_name:expr;)+) => {
        $(
            #[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
        pub struct $name {
            target: Interned<String>,
        }

        impl Step for $name {
            type Output = ();
            const DEFAULT: bool = true;

            fn should_run(run: ShouldRun) -> ShouldRun {
                let builder = run.builder;
                run.path($path).default_condition(builder.config.general.docs)
            }

            fn make_run(run: RunConfig) {
                run.builder.ensure($name {
                    target: run.target,
                });
            }

            fn run(self, builder: &Builder) {
                builder.ensure(Rustbook {
                    target: self.target,
                    name: $book_name.intern(),
                })
            }
        }
        )+
    }
}

book!(
    Nomicon, "src/doc/nomicon", "nomicon";
    Reference, "src/doc/reference", "reference";
    Rustdoc, "src/doc/rustdoc", "rustdoc";
    RustByExample, "src/doc/rust-by-example", "rust-by-example";
);

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
struct Rustbook {
    target: Interned<String>,
    name: Interned<String>,
}

impl Step for Rustbook {
    type Output = ();

    // rustbook is never directly called, and only serves as a shim for the nomicon and the
    // reference.
    fn should_run(run: ShouldRun) -> ShouldRun {
        run.never()
    }

    /// Invoke `rustbook` for `target` for the doc book `name`.
    ///
    /// This will not actually generate any documentation if the documentation has
    /// already been generated.
    fn run(self, builder: &Builder) {
        let src = builder.config.src.join("src/doc");
        builder.ensure(RustbookSrc {
            target: self.target,
            name: self.name,
            src: src.intern(),
        });
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct UnstableBook {
    target: Interned<String>,
}

impl Step for UnstableBook {
    type Output = ();
    const DEFAULT: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        let builder = run.builder;
        run.path("src/doc/unstable-book")
            .default_condition(builder.config.general.docs)
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(UnstableBook { target: run.target });
    }

    fn run(self, builder: &Builder) {
        builder.ensure(UnstableBookGen {
            target: self.target,
        });
        builder.ensure(RustbookSrc {
            target: self.target,
            name: "unstable-book".intern(),
            src: builder.md_doc_out(self.target),
        })
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct CargoBook {
    target: Interned<String>,
    name: Interned<String>,
}

impl Step for CargoBook {
    type Output = ();
    const DEFAULT: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        let builder = run.builder;
        run.path("src/tools/cargo/src/doc/book")
            .default_condition(builder.config.general.docs)
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(CargoBook {
            target: run.target,
            name: "cargo".intern(),
        });
    }

    fn run(self, builder: &Builder) {
        let target = self.target;
        let name = self.name;
        let src = builder.config.src.join("src/tools/cargo/src/doc");

        let out = builder.doc_out(target);
        t!(fs::create_dir_all(&out));

        let out = out.join(name);

        println!("Cargo Book ({}) - {}", target, name);

        let _ = fs::remove_dir_all(&out);

        builder.run(
            builder
                .tool_cmd(Tool::Rustbook)
                .arg("build")
                .arg(&src)
                .arg("-d")
                .arg(out),
        );
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
struct RustbookSrc {
    target: Interned<String>,
    name: Interned<String>,
    src: Interned<PathBuf>,
}

impl Step for RustbookSrc {
    type Output = ();

    fn should_run(run: ShouldRun) -> ShouldRun {
        run.never()
    }

    /// Invoke `rustbook` for `target` for the doc book `name` from the `src` path.
    ///
    /// This will not actually generate any documentation if the documentation has
    /// already been generated.
    fn run(self, builder: &Builder) {
        let target = self.target;
        let name = self.name;
        let src = self.src;
        let out = builder.doc_out(target);
        t!(fs::create_dir_all(&out));

        let out = out.join(name);
        let src = src.join(name);
        let index = out.join("index.html");
        let rustbook = builder.tool_exe(Tool::Rustbook);
        if up_to_date(&src, &index) && up_to_date(&rustbook, &index) {
            return;
        }
        println!("Rustbook ({}) - {}", target, name);
        let _ = fs::remove_dir_all(&out);
        builder.run(
            builder
                .tool_cmd(Tool::Rustbook)
                .arg("build")
                .arg(&src)
                .arg("-d")
                .arg(out),
        );
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct TheBook {
    compiler: Compiler,
    target: Interned<String>,
    name: &'static str,
}

impl Step for TheBook {
    type Output = ();
    const DEFAULT: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        let builder = run.builder;
        run.path("src/doc/book")
            .default_condition(builder.config.general.docs)
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(TheBook {
            compiler: run.builder
                .compiler(run.builder.top_stage, run.builder.config.general.build),
            target: run.target,
            name: "book",
        });
    }

    /// Build the book and associated stuff.
    ///
    /// We need to build:
    ///
    /// * Book (first edition)
    /// * Book (second edition)
    /// * Version info and CSS
    /// * Index page
    /// * Redirect pages
    fn run(self, builder: &Builder) {
        let compiler = self.compiler;
        let target = self.target;
        let name = self.name;
        // build book first edition
        builder.ensure(Rustbook {
            target,
            name: format!("{}/first-edition", name).intern(),
        });

        // build book second edition
        builder.ensure(Rustbook {
            target,
            name: format!("{}/second-edition", name).intern(),
        });

        // build the version info page and CSS
        builder.ensure(Standalone { compiler, target });

        // build the index page
        let index = format!("{}/index.md", name);
        println!("Documenting book index ({})", target);
        invoke_rustdoc(builder, compiler, target, &index);

        // build the redirect pages
        println!("Documenting book redirect pages ({})", target);
        for file in t!(fs::read_dir(
            builder.config.src.join("src/doc/book/redirects")
        )) {
            let file = t!(file);
            let path = file.path();
            let path = path.to_str().unwrap();

            invoke_rustdoc(builder, compiler, target, path);
        }
    }
}

fn invoke_rustdoc(builder: &Builder, compiler: Compiler, target: Interned<String>, markdown: &str) {
    let out = builder.doc_out(target);

    let path = builder.config.src.join("src/doc").join(markdown);

    let favicon = builder.config.src.join("src/doc/favicon.inc");
    let footer = builder.config.src.join("src/doc/footer.inc");
    let version_info = out.join("version_info.html");

    let mut cmd = builder.rustdoc_cmd(compiler.host);

    let out = out.join("book");

    cmd.arg("--html-after-content")
        .arg(&footer)
        .arg("--html-before-content")
        .arg(&version_info)
        .arg("--html-in-header")
        .arg(&favicon)
        .arg("--markdown-playground-url")
        .arg("https://play.rust-lang.org/")
        .arg("-o")
        .arg(&out)
        .arg(&path)
        .arg("--markdown-css")
        .arg("../rust.css");

    builder.run(&mut cmd);
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct Standalone {
    compiler: Compiler,
    target: Interned<String>,
}

impl Step for Standalone {
    type Output = ();
    const DEFAULT: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        let builder = run.builder;
        run.path("src/doc")
            .default_condition(builder.config.general.docs)
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(Standalone {
            compiler: run.builder
                .compiler(run.builder.top_stage, run.builder.config.general.build),
            target: run.target,
        });
    }

    /// Generates all standalone documentation as compiled by the rustdoc in `stage`
    /// for the `target` into `out`.
    ///
    /// This will list all of `src/doc` looking for markdown files and appropriately
    /// perform transformations like substituting `VERSION`, `SHORT_HASH`, and
    /// `STAMP` along with providing the various header/footer HTML we've customized.
    ///
    /// In the end, this is just a glorified wrapper around rustdoc!
    fn run(self, builder: &Builder) {
        let target = self.target;
        let compiler = self.compiler;
        println!("Documenting standalone ({})", target);
        let out = builder.doc_out(target);
        t!(fs::create_dir_all(&out));

        let favicon = builder.config.src.join("src/doc/favicon.inc");
        let footer = builder.config.src.join("src/doc/footer.inc");
        let full_toc = builder.config.src.join("src/doc/full-toc.inc");
        t!(fs::copy(
            builder.config.src.join("src/doc/rust.css"),
            out.join("rust.css")
        ));

        let version_input = builder
            .config
            .src
            .join("src/doc/version_info.html.template");
        let version_info = out.join("version_info.html");

        if !up_to_date(&version_input, &version_info) {
            let info = t!(fs::read_string(&version_input));
            let info = info.replace("VERSION", &builder.rust_release())
                .replace("SHORT_HASH", builder.rust_info.sha_short().unwrap_or(""))
                .replace("STAMP", builder.rust_info.sha().unwrap_or(""));
            t!(fs::write(&version_info, info))
        }

        for file in t!(fs::read_dir(builder.config.src.join("src/doc"))) {
            let file = t!(file);
            let path = file.path();
            let filename = path.file_name().unwrap().to_str().unwrap();
            if !filename.ends_with(".md") || filename == "README.md" {
                continue;
            }

            let html = out.join(filename).with_extension("html");
            let rustdoc = builder.rustdoc(compiler.host);
            if up_to_date(&path, &html) && up_to_date(&footer, &html) && up_to_date(&favicon, &html)
                && up_to_date(&full_toc, &html) && up_to_date(&version_info, &html)
                && up_to_date(&rustdoc, &html)
            {
                continue;
            }

            let mut cmd = builder.rustdoc_cmd(compiler.host);
            cmd.arg("--html-after-content")
                .arg(&footer)
                .arg("--html-before-content")
                .arg(&version_info)
                .arg("--html-in-header")
                .arg(&favicon)
                .arg("--markdown-playground-url")
                .arg("https://play.rust-lang.org/")
                .arg("-o")
                .arg(&out)
                .arg(&path);

            if filename == "not_found.md" {
                cmd.arg("--markdown-no-toc")
                    .arg("--markdown-css")
                    .arg("https://doc.rust-lang.org/rust.css");
            } else {
                cmd.arg("--markdown-css").arg("rust.css");
            }
            builder.run(&mut cmd);
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct Std {
    pub stage: u32,
    pub target: Interned<String>,
}

impl Step for Std {
    type Output = ();
    const DEFAULT: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        let builder = run.builder;
        run.all_krates("std")
            .default_condition(builder.config.general.docs)
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(Std {
            stage: run.builder.top_stage,
            target: run.target,
        });
    }

    /// Compile all standard library documentation.
    ///
    /// This will generate all documentation for the standard library and its
    /// dependencies. This is largely just a wrapper around `cargo doc`.
    fn run(self, builder: &Builder) {
        let stage = self.stage;
        let target = self.target;
        println!("Documenting stage{} std ({})", stage, target);
        let out = builder.doc_out(target);
        t!(fs::create_dir_all(&out));
        let compiler = builder.compiler(stage, builder.config.general.build);
        let rustdoc = builder.rustdoc(compiler.host);
        let compiler = if builder.force_use_stage1(compiler, target) {
            builder.compiler(1, compiler.host)
        } else {
            compiler
        };

        builder.ensure(compile::Std { compiler, target });
        let out_dir = builder
            .stage_out(compiler, Mode::Libstd)
            .join(target)
            .join("doc");

        // Here what we're doing is creating a *symlink* (directory junction on
        // Windows) to the final output location. This is not done as an
        // optimization but rather for correctness. We've got three trees of
        // documentation, one for std, one for test, and one for rustc. It's then
        // our job to merge them all together.
        //
        // Unfortunately rustbuild doesn't know nearly as well how to merge doc
        // trees as rustdoc does itself, so instead of actually having three
        // separate trees we just have rustdoc output to the same location across
        // all of them.
        //
        // This way rustdoc generates output directly into the output, and rustdoc
        // will also directly handle merging.
        let my_out = builder.crate_doc_out(target);
        builder.clear_if_dirty(&my_out, &rustdoc);
        t!(symlink_dir_force(&my_out, &out_dir));

        let mut cargo = builder.cargo(compiler, Mode::Libstd, target, "doc");

        // We don't want to build docs for internal std dependencies unless
        // in compiler-docs mode. When not in that mode, we whitelist the crates
        // for which docs must be built.
        if !builder.config.general.compiler_docs {
            cargo.arg("--no-deps");
            for krate in &["alloc", "core", "std", "std_unicode"] {
                cargo.arg("-p").arg(krate);
                // Create all crate output directories first to make sure rustdoc uses
                // relative links.
                // FIXME: Cargo should probably do this itself.
                t!(fs::create_dir_all(out_dir.join(krate)));
            }
        }

        builder.run(&mut cargo);
        cp_r(&my_out, &out);
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct Test {
    stage: u32,
    target: Interned<String>,
}

impl Step for Test {
    type Output = ();
    const DEFAULT: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        let builder = run.builder;
        run.krate("test")
            .default_condition(builder.config.general.compiler_docs)
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(Test {
            stage: run.builder.top_stage,
            target: run.target,
        });
    }

    /// Compile all libtest documentation.
    ///
    /// This will generate all documentation for libtest and its dependencies. This
    /// is largely just a wrapper around `cargo doc`.
    fn run(self, builder: &Builder) {
        let stage = self.stage;
        let target = self.target;
        println!("Documenting stage{} test ({})", stage, target);
        let out = builder.doc_out(target);
        t!(fs::create_dir_all(&out));
        let compiler = builder.compiler(stage, builder.config.general.build);
        let rustdoc = builder.rustdoc(compiler.host);
        let compiler = if builder.force_use_stage1(compiler, target) {
            builder.compiler(1, compiler.host)
        } else {
            compiler
        };

        // Build libstd docs so that we generate relative links
        builder.ensure(Std { stage, target });

        builder.ensure(compile::Test { compiler, target });
        let out_dir = builder
            .stage_out(compiler, Mode::Libtest)
            .join(target)
            .join("doc");

        // See docs in std above for why we symlink
        let my_out = builder.crate_doc_out(target);
        builder.clear_if_dirty(&my_out, &rustdoc);
        t!(symlink_dir_force(&my_out, &out_dir));

        let mut cargo = builder.cargo(compiler, Mode::Libtest, target, "doc");
        builder.run(&mut cargo);
        cp_r(&my_out, &out);
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct Rustc {
    stage: u32,
    target: Interned<String>,
}

impl Step for Rustc {
    type Output = ();
    const DEFAULT: bool = true;
    const ONLY_HOSTS: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        let builder = run.builder;
        run.krate("rustc-main")
            .default_condition(builder.config.general.docs)
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(Rustc {
            stage: run.builder.top_stage,
            target: run.target,
        });
    }

    /// Generate all compiler documentation.
    ///
    /// This will generate all documentation for the compiler libraries and their
    /// dependencies. This is largely just a wrapper around `cargo doc`.
    fn run(self, builder: &Builder) {
        let stage = self.stage;
        let target = self.target;
        println!("Documenting stage{} compiler ({})", stage, target);
        let out = builder.doc_out(target);
        t!(fs::create_dir_all(&out));
        let compiler = builder.compiler(stage, builder.config.general.build);
        let rustdoc = builder.rustdoc(compiler.host);
        let compiler = if builder.force_use_stage1(compiler, target) {
            builder.compiler(1, compiler.host)
        } else {
            compiler
        };

        // Build libstd docs so that we generate relative links
        builder.ensure(Std { stage, target });

        builder.ensure(compile::Rustc { compiler, target });
        let out_dir = builder
            .stage_out(compiler, Mode::Librustc)
            .join(target)
            .join("doc");

        // See docs in std above for why we symlink
        let my_out = builder.crate_doc_out(target);
        builder.clear_if_dirty(&my_out, &rustdoc);
        t!(symlink_dir_force(&my_out, &out_dir));

        let mut cargo = builder.cargo(compiler, Mode::Librustc, target, "doc");

        if builder.config.general.compiler_docs {
            // src/rustc/Cargo.toml contains a bin crate called rustc which
            // would otherwise overwrite the docs for the real rustc lib crate.
            cargo.arg("-p").arg("rustc_driver");
        } else {
            // Like with libstd above if compiler docs aren't enabled then we're not
            // documenting internal dependencies, so we have a whitelist.
            cargo.arg("--no-deps");
            for krate in &["proc_macro"] {
                cargo.arg("-p").arg(krate);
            }
        }

        builder.run(&mut cargo);
        cp_r(&my_out, &out);
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct ErrorIndex {
    target: Interned<String>,
}

impl Step for ErrorIndex {
    type Output = ();
    const DEFAULT: bool = true;
    const ONLY_HOSTS: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        let builder = run.builder;
        run.path("src/tools/error_index_generator")
            .default_condition(builder.config.general.docs)
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(ErrorIndex { target: run.target });
    }

    /// Generates the HTML rendered error-index by running the
    /// `error_index_generator` tool.
    fn run(self, builder: &Builder) {
        let target = self.target;

        println!("Documenting error index ({})", target);
        let out = builder.doc_out(target);
        t!(fs::create_dir_all(&out));
        let mut index = builder.tool_cmd(Tool::ErrorIndex);
        index.arg("html");
        index.arg(out.join("error-index.html"));

        // FIXME: shouldn't have to pass this env var
        index
            .env("CFG_BUILD", &builder.config.general.build)
            .env("RUSTC_ERROR_METADATA_DST", builder.extended_error_dir());

        builder.run(&mut index);
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct UnstableBookGen {
    target: Interned<String>,
}

impl Step for UnstableBookGen {
    type Output = ();
    const DEFAULT: bool = true;
    const ONLY_HOSTS: bool = true;

    fn should_run(run: ShouldRun) -> ShouldRun {
        let builder = run.builder;
        run.path("src/tools/unstable-book-gen")
            .default_condition(builder.config.general.docs)
    }

    fn make_run(run: RunConfig) {
        run.builder.ensure(UnstableBookGen { target: run.target });
    }

    fn run(self, builder: &Builder) {
        let target = self.target;

        builder.ensure(compile::Std {
            compiler: builder.compiler(builder.top_stage, builder.config.general.build),
            target,
        });

        println!("Generating unstable book md files ({})", target);
        let out = builder.md_doc_out(target).join("unstable-book");
        t!(fs::create_dir_all(&out));
        t!(fs::remove_dir_all(&out));
        let mut cmd = builder.tool_cmd(Tool::UnstableBookGen);
        cmd.arg(builder.config.src.join("src"));
        cmd.arg(out);

        builder.run(&mut cmd);
    }
}

fn symlink_dir_force(src: &Path, dst: &Path) -> io::Result<()> {
    if cfg!(test) { return Ok(()); }
    if let Ok(m) = fs::symlink_metadata(dst) {
        if m.file_type().is_dir() {
            try!(fs::remove_dir_all(dst));
        } else {
            // handle directory junctions on windows by falling back to
            // `remove_dir`.
            try!(fs::remove_file(dst).or_else(|_| fs::remove_dir(dst)));
        }
    }

    symlink_dir(src, dst)
}
