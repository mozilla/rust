// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Serialized configuration of a build.
//!
//! This module implements parsing `config.toml` configuration files to tweak
//! how the build runs.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::cmp;

use num_cpus;
use channel;
use toml;
use util::exe;
use cache::{INTERNER, Interned};
use flags::Flags;
use build_helper::output;
pub use flags::Subcommand;

/// Global configuration for the entire build and/or bootstrap.
///
/// This structure is derived from a combination of both `config.toml` and
/// `config.mk`. As of the time of this writing it's unlikely that `config.toml`
/// is used all that much, so this is primarily filled out by `config.mk` which
/// is generated from `./configure`.
///
/// Note that this structure is not decoded directly into, but rather it is
/// filled out from the decoded forms of the structs below. For documentation
/// each field, see the corresponding fields in
/// `config.toml.example`.
#[derive(Default)]
pub struct Config {
    pub ccache: Option<String>,
    pub ninja: bool,
    pub verbose: usize,
    pub submodules: bool,
    pub compiler_docs: bool,
    pub docs: bool,
    pub locked_deps: bool,
    pub vendor: bool,
    pub target_config: HashMap<Interned<String>, Target>,
    pub full_bootstrap: bool,
    pub extended: bool,
    pub tools: Option<HashSet<String>>,
    pub sanitizers: bool,
    pub profiler: bool,
    pub ignore_git: bool,
    pub exclude: Vec<PathBuf>,
    pub rustc_error_format: Option<String>,

    pub run_host_only: bool,
    pub is_sudo: bool,

    pub on_fail: Option<String>,
    pub stage: Option<u32>,
    pub keep_stage: Option<u32>,
    pub src: PathBuf,
    pub out: PathBuf,
    pub jobs: Option<u32>,
    pub cmd: Subcommand,
    pub paths: Vec<PathBuf>,
    pub incremental: bool,

    // llvm codegen options
    pub llvm_enabled: bool,
    pub llvm_assertions: bool,
    pub llvm_optimize: bool,
    pub llvm_release_debuginfo: bool,
    pub llvm_version_check: bool,
    pub llvm_static_stdcpp: bool,
    pub llvm_link_shared: bool,
    pub llvm_targets: Option<String>,
    pub llvm_experimental_targets: String,
    pub llvm_link_jobs: Option<u32>,

    // rust codegen options
    pub rust_optimize: bool,
    pub rust_codegen_units: Option<u32>,
    pub rust_thinlto: bool,
    pub rust_debug_assertions: bool,
    pub rust_debuginfo: bool,
    pub rust_debuginfo_lines: bool,
    pub rust_debuginfo_only_std: bool,
    pub rust_rpath: bool,
    pub rustc_parallel_queries: bool,
    pub rustc_default_linker: Option<String>,
    pub rust_optimize_tests: bool,
    pub rust_debuginfo_tests: bool,
    pub rust_dist_src: bool,
    pub rust_codegen_backends: Vec<Interned<String>>,

    pub build: Interned<String>,
    pub hosts: Vec<Interned<String>>,
    pub targets: Vec<Interned<String>>,
    pub local_rebuild: bool,

    // dist misc
    pub dist_sign_folder: Option<PathBuf>,
    pub dist_upload_addr: Option<String>,
    pub dist_gpg_password_file: Option<PathBuf>,

    // libstd features
    pub debug_jemalloc: bool,
    pub use_jemalloc: bool,
    pub backtrace: bool, // support for RUST_BACKTRACE
    pub wasm_syscall: bool,

    // misc
    pub low_priority: bool,
    pub channel: String,
    pub quiet_tests: bool,
    pub test_miri: bool,
    pub save_toolstates: Option<PathBuf>,

    // Fallback musl-root for all targets
    pub musl_root: Option<PathBuf>,
    pub prefix: Option<PathBuf>,
    pub sysconfdir: Option<PathBuf>,
    pub docdir: Option<PathBuf>,
    pub bindir: Option<PathBuf>,
    pub libdir: Option<PathBuf>,
    pub mandir: Option<PathBuf>,
    pub codegen_tests: bool,
    pub nodejs: Option<PathBuf>,
    pub gdb: Option<PathBuf>,
    pub python: Option<PathBuf>,
    pub openssl_static: bool,
    pub configure_args: Vec<String>,

    // These are either the stage0 downloaded binaries or the locally installed ones.
    pub initial_cargo: PathBuf,
    pub initial_rustc: PathBuf,
}

/// Per-target configuration stored in the global configuration structure.
#[derive(Default)]
pub struct Target {
    /// Some(path to llvm-config) if using an external LLVM.
    pub llvm_config: Option<PathBuf>,
    pub jemalloc: Option<PathBuf>,
    pub cc: Option<PathBuf>,
    pub cxx: Option<PathBuf>,
    pub ar: Option<PathBuf>,
    pub linker: Option<PathBuf>,
    pub ndk: Option<PathBuf>,
    pub crt_static: Option<bool>,
    pub musl_root: Option<PathBuf>,
    pub qemu_rootfs: Option<PathBuf>,
}

/// Structure of the `config.toml` file that configuration is read from.
///
/// This structure uses `Decodable` to automatically decode a TOML configuration
/// file into this format, and then this is traversed and written into the above
/// `Config` structure.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct TomlConfig {
    build: Build,
    install: Install,
    llvm: Llvm,
    rust: Rust,
    target: HashMap<String, TomlTarget>,
    dist: Dist,
}

/// TOML representation of various global build decisions.
#[derive(Deserialize, Clone)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct Build {
    build: Option<String>,
    host: Vec<String>,
    target: Vec<String>,
    cargo: Option<String>,
    rustc: Option<String>,
    low_priority: bool,
    compiler_docs: bool,
    docs: bool,
    submodules: bool,
    gdb: Option<PathBuf>,
    locked_deps: bool,
    vendor: bool,
    nodejs: Option<PathBuf>,
    python: Option<PathBuf>,
    full_bootstrap: bool,
    extended: bool,
    tools: Option<HashSet<String>>,
    verbose: usize,
    sanitizers: bool,
    profiler: bool,
    openssl_static: bool,
    configure_args: Vec<String>,
    local_rebuild: bool,
}

impl Default for Build {
    fn default() -> Build {
        Build {
            build: None,
            host: Vec::new(),
            target: Vec::new(),
            cargo: None,
            rustc: None,
            low_priority: false,
            compiler_docs: false,
            docs: true,
            submodules: true,
            gdb: None,
            locked_deps: false,
            vendor: false,
            nodejs: None,
            python: None,
            full_bootstrap: false,
            extended: false,
            tools: None,
            verbose: 0,
            sanitizers: false,
            profiler: false,
            openssl_static: false,
            configure_args: Vec::new(),
            local_rebuild: false,
        }
    }
}

/// TOML representation of various global install decisions.
#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Install {
    prefix: Option<PathBuf>,
    sysconfdir: Option<PathBuf>,
    docdir: Option<PathBuf>,
    bindir: Option<PathBuf>,
    libdir: Option<PathBuf>,
    mandir: Option<PathBuf>,

    // standard paths, currently unused
    datadir: Option<PathBuf>,
    infodir: Option<PathBuf>,
    localstatedir: Option<PathBuf>,
}

/// TOML representation of how the LLVM build is configured.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct Llvm {
    enabled: bool,
    ccache: Option<StringOrBool>,
    ninja: bool,
    assertions: bool,
    optimize: bool,
    release_debuginfo: bool,
    version_check: bool,
    static_libstdcpp: bool,
    targets: Option<String>,
    experimental_targets: String,
    link_jobs: Option<u32>,
    link_shared: bool,
}

impl Default for Llvm {
    fn default() -> Llvm {
        Llvm {
            enabled: true,
            ccache: None,
            ninja: false,
            assertions: false,
            optimize: true,
            release_debuginfo: false,
            version_check: true,
            targets: None,
            experimental_targets: String::from("WebAssembly"),
            link_jobs: None,
            static_libstdcpp: false,
            link_shared: false,
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct Dist {
    sign_folder: Option<PathBuf>,
    gpg_password_file: Option<PathBuf>,
    upload_addr: Option<String>,
    src_tarball: bool,
}

impl Default for Dist {
    fn default() -> Dist {
        Dist {
            sign_folder: None,
            gpg_password_file: None,
            upload_addr: None,
            src_tarball: true,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrBool {
    String(String),
    Bool(bool),
}

impl Default for StringOrBool {
    fn default() -> StringOrBool {
        StringOrBool::Bool(false)
    }
}

/// TOML representation of how the Rust build is configured.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct Rust {
    optimize: Option<bool>,
    codegen_units: Option<u32>,
    thinlto: bool,
    debug_assertions: Option<bool>,
    debuginfo: Option<bool>,
    debuginfo_lines: Option<bool>,
    debuginfo_only_std: Option<bool>,
    experimental_parallel_queries: bool,
    debug_jemalloc: Option<bool>,
    use_jemalloc: bool,
    backtrace: bool,
    default_linker: Option<String>,
    channel: String,
    musl_root: Option<PathBuf>,
    rpath: bool,
    optimize_tests: bool,
    debuginfo_tests: bool,
    codegen_tests: bool,
    ignore_git: Option<bool>,
    debug: Option<bool>,
    quiet_tests: bool,
    test_miri: bool,
    save_toolstates: Option<PathBuf>,
    codegen_backends: Vec<String>,
    wasm_syscall: bool,
}

impl Default for Rust {
    fn default() -> Rust {
        Rust {
            debug: None,
            debug_assertions: None,
            debuginfo: None,
            debuginfo_lines: None,
            debuginfo_only_std: None,
            optimize: None,
            ignore_git: None,
            debug_jemalloc: None,
            thinlto: true,
            optimize_tests: true,
            debuginfo_tests: false,
            codegen_tests: true,
            rpath: true,
            use_jemalloc: true,
            backtrace: true,
            channel: String::from("dev"),
            quiet_tests: false,
            test_miri: false,
            wasm_syscall: false,
            codegen_backends: vec![String::from("llvm")],
            codegen_units: None,
            default_linker: None,
            experimental_parallel_queries: false,
            musl_root: None,
            save_toolstates: None,
        }
    }
}

/// TOML representation of how each build target is configured.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct TomlTarget {
    llvm_config: Option<PathBuf>,
    jemalloc: Option<PathBuf>,
    cc: Option<PathBuf>,
    cxx: Option<PathBuf>,
    ar: Option<PathBuf>,
    linker: Option<PathBuf>,
    android_ndk: Option<PathBuf>,
    crt_static: Option<bool>,
    musl_root: Option<PathBuf>,
    qemu_rootfs: Option<PathBuf>,
}

impl Config {
    pub fn parse(args: &[String]) -> Config {
        let flags = Flags::parse(&args);
        let file = flags.config.clone();
        let mut config = Config::default();
        config.exclude = flags.exclude;
        config.paths = flags.paths;
        config.rustc_error_format = flags.rustc_error_format;
        config.on_fail = flags.on_fail;
        config.stage = flags.stage;
        config.src = flags.src;
        config.jobs = flags.jobs;
        config.cmd = flags.cmd;
        config.incremental = flags.incremental;
        config.keep_stage = flags.keep_stage;

        // If --target was specified but --host wasn't specified, don't run any host-only tests.
        config.run_host_only = !(flags.host.is_empty() && !flags.target.is_empty());

        config.is_sudo = match env::var_os("SUDO_USER") {
            Some(sudo_user) => {
                match env::var_os("USER") {
                    Some(user) => user != sudo_user,
                    None => false,
                }
            }
            None => false,
        };

        let toml = file.map(|file| {
            let mut f = t!(File::open(&file));
            let mut contents = String::new();
            t!(f.read_to_string(&mut contents));
            match toml::from_str(&contents) {
                Ok(table) => table,
                Err(err) => {
                    println!("failed to parse TOML configuration '{}': {}",
                        file.display(), err);
                    process::exit(2);
                }
            }
        }).unwrap_or_else(|| TomlConfig::default());

        let build = toml.build;
        set(&mut config.build, build.build.clone().map(|x| INTERNER.intern_string(x)));
        set(&mut config.build, flags.build);
        if config.build.is_empty() {
            // set by bootstrap.py
            config.build = INTERNER.intern_str(&env::var("BUILD").unwrap());
        }
        config.hosts.push(config.build.clone());
        for host in build.host.iter() {
            let host = INTERNER.intern_str(host);
            if !config.hosts.contains(&host) {
                config.hosts.push(host);
            }
        }
        for target in config.hosts.iter().cloned()
            .chain(build.target.iter().map(|s| INTERNER.intern_str(s)))
        {
            if !config.targets.contains(&target) {
                config.targets.push(target);
            }
        }
        config.hosts = if !flags.host.is_empty() {
            flags.host
        } else {
            config.hosts
        };
        config.targets = if !flags.target.is_empty() {
            flags.target
        } else {
            config.targets
        };


        config.nodejs = build.nodejs;
        config.gdb = build.gdb;
        config.python = build.python;
        config.low_priority = build.low_priority;
        config.compiler_docs = build.compiler_docs;
        config.docs = build.docs;
        config.submodules = build.submodules;
        config.locked_deps = build.locked_deps;
        config.vendor = build.vendor;
        config.full_bootstrap = build.full_bootstrap;
        config.extended = build.extended;
        config.tools = build.tools;
        config.verbose = cmp::max(build.verbose, flags.verbose);
        config.sanitizers = build.sanitizers;
        config.profiler = build.profiler;
        config.openssl_static = build.openssl_static;
        config.configure_args = build.configure_args;
        // will get auto-detected later
        config.local_rebuild = build.local_rebuild;

        config.prefix = toml.install.prefix;
        config.sysconfdir = toml.install.sysconfdir;
        config.docdir = toml.install.docdir;
        config.bindir = toml.install.bindir;
        config.libdir = toml.install.libdir;
        config.mandir = toml.install.mandir;

        let llvm = &toml.llvm;
        match llvm.ccache {
            Some(StringOrBool::String(ref s)) => {
                config.ccache = Some(s.to_string())
            }
            Some(StringOrBool::Bool(true)) => {
                config.ccache = Some("ccache".to_string());
            }
            Some(StringOrBool::Bool(false)) | None => {}
        }
        config.ninja = llvm.ninja;
        config.llvm_enabled = llvm.enabled;
        config.llvm_assertions = llvm.assertions;
        config.llvm_optimize = llvm.optimize;
        config.llvm_release_debuginfo = llvm.release_debuginfo;
        config.llvm_version_check = llvm.version_check;
        config.llvm_static_stdcpp = llvm.static_libstdcpp;
        config.llvm_link_shared = llvm.link_shared;
        config.llvm_targets = llvm.targets.clone();
        config.llvm_experimental_targets = llvm.experimental_targets.clone();
        config.llvm_link_jobs = llvm.link_jobs;

        // Store off these values as options because if they're not
        // provided we'll infer default values for them later
        let debuginfo_lines = toml.rust.debuginfo_lines;
        let debuginfo_only_std = toml.rust.debuginfo_only_std;
        let debug = toml.rust.debug;
        let debug_jemalloc = toml.rust.debug_jemalloc;
        let debuginfo = toml.rust.debuginfo;
        let debug_assertions = toml.rust.debug_assertions;
        let optimize = toml.rust.optimize;
        let ignore_git = toml.rust.ignore_git;

        config.rust_optimize_tests = toml.rust.optimize_tests;
        config.rust_debuginfo_tests = toml.rust.debuginfo_tests;
        config.codegen_tests = toml.rust.codegen_tests;
        config.rust_rpath = toml.rust.rpath;
        config.use_jemalloc = toml.rust.use_jemalloc;
        config.backtrace = toml.rust.backtrace;
        config.channel = toml.rust.channel.clone();
        config.quiet_tests = toml.rust.quiet_tests;
        config.test_miri = toml.rust.test_miri;
        config.wasm_syscall = toml.rust.wasm_syscall;
        config.rustc_parallel_queries = toml.rust.experimental_parallel_queries;
        config.rustc_default_linker = toml.rust.default_linker.clone();
        config.musl_root = toml.rust.musl_root.clone();
        config.save_toolstates = toml.rust.save_toolstates.clone();
        config.rust_thinlto = toml.rust.thinlto;

        config.rust_codegen_backends = toml.rust.codegen_backends.iter()
            .map(|s| INTERNER.intern_str(s))
            .collect();

        match toml.rust.codegen_units {
            Some(0) => config.rust_codegen_units = Some(num_cpus::get() as u32),
            Some(n) => config.rust_codegen_units = Some(n),
            None => {}
        }

        for (triple, cfg) in toml.target {
            let mut target = Target::default();

            if let Some(ref s) = cfg.llvm_config {
                target.llvm_config = Some(env::current_dir().unwrap().join(s));
            }
            if let Some(ref s) = cfg.jemalloc {
                target.jemalloc = Some(env::current_dir().unwrap().join(s));
            }
            if let Some(ref s) = cfg.android_ndk {
                target.ndk = Some(env::current_dir().unwrap().join(s));
            }
            target.cc = cfg.cc.clone();
            target.cxx = cfg.cxx.clone();
            target.ar = cfg.ar.clone();
            target.linker = cfg.linker.clone();
            target.crt_static = cfg.crt_static.clone();
            target.musl_root = cfg.musl_root.clone();
            target.qemu_rootfs = cfg.qemu_rootfs.clone();

            config.target_config.insert(INTERNER.intern_string(triple.clone()), target);
        }

        config.dist_sign_folder = toml.dist.sign_folder.clone();
        config.dist_gpg_password_file = toml.dist.gpg_password_file.clone();
        config.dist_upload_addr = toml.dist.upload_addr.clone();
        config.rust_dist_src = toml.dist.src_tarball;

        let cwd = t!(env::current_dir());
        let out = cwd.join("build");
        config.out = out.clone();

        let stage0_root = out.join(&config.build).join("stage0/bin");
        config.initial_rustc = match build.rustc {
            Some(s) => PathBuf::from(s),
            None => stage0_root.join(exe("rustc", &config.build)),
        };
        // If local-rust is the same major.minor as the current version, then force a local-rebuild
        let local_version_verbose = output(
            Command::new(&config.initial_rustc).arg("--version").arg("--verbose"));
        let local_release = local_version_verbose
            .lines().filter(|x| x.starts_with("release:"))
            .next().unwrap().trim_left_matches("release:").trim();
        let my_version = channel::CFG_RELEASE_NUM;
        if local_release.split('.').take(2).eq(my_version.split('.').take(2)) {
            eprintln!("auto-detected local rebuild");
            config.local_rebuild = true;
        }
        config.initial_cargo = match build.cargo {
            Some(s) => PathBuf::from(s),
            None => stage0_root.join(exe("cargo", &config.build)),
        };

        // Now that we've reached the end of our configuration, infer the
        // default values for all options that we haven't otherwise stored yet.
        let default = match &config.channel[..] {
            "stable" | "beta" | "nightly" => true,
            _ => false,
        };
        config.rust_debuginfo_lines = debuginfo_lines.unwrap_or(default);
        config.rust_debuginfo_only_std = debuginfo_only_std.unwrap_or(default);

        let default = debug == Some(true);
        config.debug_jemalloc = debug_jemalloc.unwrap_or(default);
        config.rust_debuginfo = debuginfo.unwrap_or(default);
        config.rust_debug_assertions = debug_assertions.unwrap_or(default);
        config.rust_optimize = optimize.unwrap_or(!default);

        let default = config.channel == "dev";
        config.ignore_git = ignore_git.unwrap_or(default);

        config
    }

    /// Try to find the relative path of `libdir`.
    pub fn libdir_relative(&self) -> Option<&Path> {
        let libdir = self.libdir.as_ref()?;
        if libdir.is_relative() {
            Some(libdir)
        } else {
            // Try to make it relative to the prefix.
            libdir.strip_prefix(self.prefix.as_ref()?).ok()
        }
    }

    pub fn verbose(&self) -> bool {
        self.verbose > 0
    }

    pub fn very_verbose(&self) -> bool {
        self.verbose > 1
    }
}

fn set<T>(field: &mut T, val: Option<T>) {
    if let Some(v) = val {
        *field = v;
    }
}
