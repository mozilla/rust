use rustc::dep_graph::DepGraph;
use rustc::hir;
use rustc::hir::lowering::lower_crate;
use rustc::hir::map as hir_map;
use rustc::hir::def_id::LOCAL_CRATE;
use rustc::lint;
use rustc::middle::{self, reachable, resolve_lifetime, stability};
use rustc::ty::{self, AllArenas, Resolutions, TyCtxt};
use rustc::traits;
use rustc::util::common::{install_panic_hook, time, ErrorReported};
use rustc::util::profiling::ProfileCategory;
use rustc::session::{CompileResult, Session};
use rustc::session::CompileIncomplete;
use rustc::session::config::{self, Input, OutputFilenames, OutputType};
use rustc::session::search_paths::PathKind;
use rustc_allocator as allocator;
use rustc_borrowck as borrowck;
use rustc_codegen_utils::codegen_backend::CodegenBackend;
use rustc_data_structures::sync::{self, Lock};
use rustc_incremental;
use rustc_metadata::creader::CrateLoader;
use rustc_metadata::cstore::{self, CStore};
use rustc_mir as mir;
use rustc_passes::{self, ast_validation, hir_stats};
use rustc_plugin as plugin;
use rustc_plugin::registry::Registry;
use rustc_privacy;
use rustc_resolve::{Resolver, ResolverArenas};
use rustc_traits;
use rustc_typeck as typeck;
use syntax::{self, ast, diagnostics, visit};
use syntax::early_buffered_lints::BufferedEarlyLint;
use syntax::ext::base::ExtCtxt;
use syntax::mut_visit::MutVisitor;
use syntax::parse::{self, PResult};
use syntax::util::node_count::NodeCounter;
use syntax_pos::hygiene;
use syntax_ext;

use serialize::json;

use std::any::Any;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::iter;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use rustc_interface::{util, profile, passes};
use super::Compilation;

#[cfg(not(parallel_compiler))]
pub fn spawn_thread_pool<F: FnOnce(config::Options) -> R + sync::Send, R: sync::Send>(
    opts: config::Options,
    f: F
) -> R {
    ty::tls::GCX_PTR.set(&Lock::new(0), || {
        f(opts)
    })
}

#[cfg(parallel_compiler)]
pub fn spawn_thread_pool<F: FnOnce(config::Options) -> R + sync::Send, R: sync::Send>(
    opts: config::Options,
    f: F
) -> R {
    use syntax;
    use syntax_pos;
    use rayon::{ThreadPoolBuilder, ThreadPool};

    let gcx_ptr = &Lock::new(0);

    let config = ThreadPoolBuilder::new()
        .num_threads(Session::threads_from_count(opts.debugging_opts.threads))
        .deadlock_handler(|| unsafe { ty::query::handle_deadlock() })
        .stack_size(::STACK_SIZE);

    let with_pool = move |pool: &ThreadPool| {
        pool.install(move || f(opts))
    };

    syntax::GLOBALS.with(|syntax_globals| {
        syntax_pos::GLOBALS.with(|syntax_pos_globals| {
            // The main handler run for each Rayon worker thread and sets up
            // the thread local rustc uses. syntax_globals and syntax_pos_globals are
            // captured and set on the new threads. ty::tls::with_thread_locals sets up
            // thread local callbacks from libsyntax
            let main_handler = move |worker: &mut dyn FnMut()| {
                syntax::GLOBALS.set(syntax_globals, || {
                    syntax_pos::GLOBALS.set(syntax_pos_globals, || {
                        ty::tls::with_thread_locals(|| {
                            ty::tls::GCX_PTR.set(gcx_ptr, || {
                                worker()
                            })
                        })
                    })
                })
            };

            ThreadPool::scoped_pool(config, main_handler, with_pool).unwrap()
        })
    })
}

pub fn compile_input(
    codegen_backend: Box<dyn CodegenBackend>,
    sess: &Session,
    cstore: &CStore,
    input_path: &Option<PathBuf>,
    input: &Input,
    outdir: &Option<PathBuf>,
    output: &Option<PathBuf>,
    addl_plugins: Option<Vec<String>>,
    control: &CompileController,
) -> CompileResult {
    macro_rules! controller_entry_point {
        ($point: ident, $tsess: expr, $make_state: expr, $phase_result: expr) => {{
            let state = &mut $make_state;
            let phase_result: &CompileResult = &$phase_result;
            if phase_result.is_ok() || control.$point.run_callback_on_error {
                (control.$point.callback)(state);
            }

            if control.$point.stop == Compilation::Stop {
                // FIXME: shouldn't this return Err(CompileIncomplete::Stopped)
                // if there are no errors?
                return $tsess.compile_status();
            }
        }}
    }

    if sess.profile_queries() {
        profile::begin(sess);
    }

    // We need nested scopes here, because the intermediate results can keep
    // large chunks of memory alive and we want to free them as soon as
    // possible to keep the peak memory usage low
    let (outputs, ongoing_codegen, dep_graph) = {
        let krate = match phase_1_parse_input(control, sess, input) {
            Ok(krate) => krate,
            Err(mut parse_error) => {
                parse_error.emit();
                return Err(CompileIncomplete::Errored(ErrorReported));
            }
        };

        let (krate, registry) = {
            let mut compile_state =
                CompileState::state_after_parse(input, sess, outdir, output, krate, &cstore);
            controller_entry_point!(after_parse, sess, compile_state, Ok(()));

            (compile_state.krate.unwrap(), compile_state.registry)
        };

        let outputs = util::build_output_filenames(input, outdir, output, &krate.attrs, sess);
        let crate_name =
            ::rustc_codegen_utils::link::find_crate_name(Some(sess), &krate.attrs, input);
        install_panic_hook();

        let ExpansionResult {
            expanded_crate,
            defs,
            resolutions,
            mut hir_forest,
        } = {
            phase_2_configure_and_expand(
                sess,
                &cstore,
                krate,
                registry,
                &crate_name,
                addl_plugins,
                |expanded_crate| {
                    let mut state = CompileState::state_after_expand(
                        input,
                        sess,
                        outdir,
                        output,
                        &cstore,
                        expanded_crate,
                        &crate_name,
                    );
                    controller_entry_point!(after_expand, sess, state, Ok(()));
                    Ok(())
                },
            )?
        };

        let output_paths = passes::generated_output_paths(
            sess,
            &outputs,
            output.is_some(),
            &crate_name
        );

        // Ensure the source file isn't accidentally overwritten during compilation.
        if let Some(ref input_path) = *input_path {
            if sess.opts.will_create_output_file() {
                if passes::output_contains_path(&output_paths, input_path) {
                    sess.err(&format!(
                        "the input file \"{}\" would be overwritten by the generated \
                         executable",
                        input_path.display()
                    ));
                    return Err(CompileIncomplete::Stopped);
                }
                if let Some(dir_path) = passes::output_conflicts_with_dir(&output_paths) {
                    sess.err(&format!(
                        "the generated executable for the input file \"{}\" conflicts with the \
                         existing directory \"{}\"",
                        input_path.display(),
                        dir_path.display()
                    ));
                    return Err(CompileIncomplete::Stopped);
                }
            }
        }

        passes::write_out_deps(sess, &outputs, &output_paths);
        if sess.opts.output_types.contains_key(&OutputType::DepInfo)
            && sess.opts.output_types.len() == 1
        {
            return Ok(());
        }

        if let &Some(ref dir) = outdir {
            if fs::create_dir_all(dir).is_err() {
                sess.err("failed to find or create the directory specified by --out-dir");
                return Err(CompileIncomplete::Stopped);
            }
        }

        // Construct the HIR map
        let hir_map = time(sess, "indexing hir", || {
            hir_map::map_crate(sess, cstore, &mut hir_forest, &defs)
        });

        {
            hir_map.dep_graph.assert_ignored();
            controller_entry_point!(
                after_hir_lowering,
                sess,
                CompileState::state_after_hir_lowering(
                    input,
                    sess,
                    outdir,
                    output,
                    &cstore,
                    &hir_map,
                    &resolutions,
                    &expanded_crate,
                    &hir_map.krate(),
                    &outputs,
                    &crate_name
                ),
                Ok(())
            );
        }

        let opt_crate = if control.keep_ast {
            Some(&expanded_crate)
        } else {
            drop(expanded_crate);
            None
        };

        let mut arenas = AllArenas::new();

        phase_3_run_analysis_passes(
            &*codegen_backend,
            control,
            sess,
            cstore,
            hir_map,
            resolutions,
            &mut arenas,
            &crate_name,
            &outputs,
            |tcx, rx, result| {
                {
                    // Eventually, we will want to track plugins.
                    tcx.dep_graph.with_ignore(|| {
                        let mut state = CompileState::state_after_analysis(
                            input,
                            sess,
                            outdir,
                            output,
                            opt_crate,
                            tcx.hir().krate(),
                            tcx,
                            &crate_name,
                        );
                        (control.after_analysis.callback)(&mut state);
                    });

                    // Plugins like clippy and rust-semverver stop the analysis early,
                    // but want to still return an error if errors during the analysis
                    // happened:
                    tcx.sess.compile_status()?;

                    if control.after_analysis.stop == Compilation::Stop {
                        return result.and_then(|_| Err(CompileIncomplete::Stopped));
                    }
                }

                result?;

                if log_enabled!(::log::Level::Info) {
                    println!("Pre-codegen");
                    tcx.print_debug_stats();
                }

                let ongoing_codegen = phase_4_codegen(&*codegen_backend, tcx, rx);

                if log_enabled!(::log::Level::Info) {
                    println!("Post-codegen");
                    tcx.print_debug_stats();
                }

                if tcx.sess.opts.output_types.contains_key(&OutputType::Mir) {
                    if let Err(e) = mir::transform::dump_mir::emit_mir(tcx, &outputs) {
                        sess.err(&format!("could not emit MIR: {}", e));
                        sess.abort_if_errors();
                    }
                }

                if tcx.sess.opts.debugging_opts.query_stats {
                    tcx.queries.print_stats();
                }

                Ok((outputs.clone(), ongoing_codegen, tcx.dep_graph.clone()))
            },
        )?
    };

    if sess.opts.debugging_opts.print_type_sizes {
        sess.code_stats.borrow().print_type_sizes();
    }

    codegen_backend.join_codegen_and_link(ongoing_codegen, sess, &dep_graph, &outputs)?;

    if sess.opts.debugging_opts.perf_stats {
        sess.print_perf_stats();
    }

    controller_entry_point!(
        compilation_done,
        sess,
        CompileState::state_when_compilation_done(input, sess, outdir, output),
        Ok(())
    );

    Ok(())
}

/// CompileController is used to customize compilation, it allows compilation to
/// be stopped and/or to call arbitrary code at various points in compilation.
/// It also allows for various flags to be set to influence what information gets
/// collected during compilation.
///
/// This is a somewhat higher level controller than a Session - the Session
/// controls what happens in each phase, whereas the CompileController controls
/// whether a phase is run at all and whether other code (from outside the
/// compiler) is run between phases.
///
/// Note that if compilation is set to stop and a callback is provided for a
/// given entry point, the callback is called before compilation is stopped.
///
/// Expect more entry points to be added in the future.
pub struct CompileController<'a> {
    pub after_parse: PhaseController<'a>,
    pub after_expand: PhaseController<'a>,
    pub after_hir_lowering: PhaseController<'a>,
    pub after_analysis: PhaseController<'a>,
    pub compilation_done: PhaseController<'a>,

    // FIXME we probably want to group the below options together and offer a
    // better API, rather than this ad-hoc approach.
    // Whether the compiler should keep the ast beyond parsing.
    pub keep_ast: bool,
    // -Zcontinue-parse-after-error
    pub continue_parse_after_error: bool,

    /// Allows overriding default rustc query providers,
    /// after `default_provide` has installed them.
    pub provide: Box<dyn Fn(&mut ty::query::Providers) + 'a + sync::Send>,
    /// Same as `provide`, but only for non-local crates,
    /// applied after `default_provide_extern`.
    pub provide_extern: Box<dyn Fn(&mut ty::query::Providers) + 'a + sync::Send>,
}

impl<'a> CompileController<'a> {
    pub fn basic() -> CompileController<'a> {
        sync::assert_send::<Self>();
        CompileController {
            after_parse: PhaseController::basic(),
            after_expand: PhaseController::basic(),
            after_hir_lowering: PhaseController::basic(),
            after_analysis: PhaseController::basic(),
            compilation_done: PhaseController::basic(),
            keep_ast: false,
            continue_parse_after_error: false,
            provide: box |_| {},
            provide_extern: box |_| {},
        }
    }
}

/// This implementation makes it easier to create a custom driver when you only want to hook
/// into callbacks from `CompileController`.
///
/// # Example
///
/// ```no_run
/// # extern crate rustc_driver;
/// # use rustc_driver::driver::CompileController;
/// let mut controller = CompileController::basic();
/// controller.after_analysis.callback = Box::new(move |_state| {});
/// rustc_driver::run_compiler(&[], Box::new(controller), None, None);
/// ```
impl<'a> ::CompilerCalls<'a> for CompileController<'a> {
    fn early_callback(
        &mut self,
        matches: &::getopts::Matches,
        sopts: &config::Options,
        cfg: &ast::CrateConfig,
        descriptions: &::errors::registry::Registry,
        output: ::ErrorOutputType,
    ) -> Compilation {
        ::RustcDefaultCalls.early_callback(
            matches,
            sopts,
            cfg,
            descriptions,
            output,
        )
    }
    fn no_input(
        &mut self,
        matches: &::getopts::Matches,
        sopts: &config::Options,
        cfg: &ast::CrateConfig,
        odir: &Option<PathBuf>,
        ofile: &Option<PathBuf>,
        descriptions: &::errors::registry::Registry,
    ) -> Option<(Input, Option<PathBuf>)> {
        ::RustcDefaultCalls.no_input(
            matches,
            sopts,
            cfg,
            odir,
            ofile,
            descriptions,
        )
    }
    fn late_callback(
        &mut self,
        codegen_backend: &dyn (::CodegenBackend),
        matches: &::getopts::Matches,
        sess: &Session,
        cstore: &CStore,
        input: &Input,
        odir: &Option<PathBuf>,
        ofile: &Option<PathBuf>,
    ) -> Compilation {
        ::RustcDefaultCalls
            .late_callback(codegen_backend, matches, sess, cstore, input, odir, ofile)
    }
    fn build_controller(
        self: Box<Self>,
        _: &Session,
        _: &::getopts::Matches
    ) -> CompileController<'a> {
        *self
    }
}

pub struct PhaseController<'a> {
    pub stop: Compilation,
    // If true then the compiler will try to run the callback even if the phase
    // ends with an error. Note that this is not always possible.
    pub run_callback_on_error: bool,
    pub callback: Box<dyn Fn(&mut CompileState) + 'a + sync::Send>,
}

impl<'a> PhaseController<'a> {
    pub fn basic() -> PhaseController<'a> {
        PhaseController {
            stop: Compilation::Continue,
            run_callback_on_error: false,
            callback: box |_| {},
        }
    }
}

/// State that is passed to a callback. What state is available depends on when
/// during compilation the callback is made. See the various constructor methods
/// (`state_*`) in the impl to see which data is provided for any given entry point.
pub struct CompileState<'a, 'tcx: 'a> {
    pub input: &'a Input,
    pub session: &'tcx Session,
    pub krate: Option<ast::Crate>,
    pub registry: Option<Registry<'a>>,
    pub cstore: Option<&'tcx CStore>,
    pub crate_name: Option<&'a str>,
    pub output_filenames: Option<&'a OutputFilenames>,
    pub out_dir: Option<&'a Path>,
    pub out_file: Option<&'a Path>,
    pub expanded_crate: Option<&'a ast::Crate>,
    pub hir_crate: Option<&'a hir::Crate>,
    pub hir_map: Option<&'a hir_map::Map<'tcx>>,
    pub resolutions: Option<&'a Resolutions>,
    pub tcx: Option<TyCtxt<'a, 'tcx, 'tcx>>,
}

impl<'a, 'tcx> CompileState<'a, 'tcx> {
    fn empty(input: &'a Input, session: &'tcx Session, out_dir: &'a Option<PathBuf>) -> Self {
        CompileState {
            input,
            session,
            out_dir: out_dir.as_ref().map(|s| &**s),
            out_file: None,
            krate: None,
            registry: None,
            cstore: None,
            crate_name: None,
            output_filenames: None,
            expanded_crate: None,
            hir_crate: None,
            hir_map: None,
            resolutions: None,
            tcx: None,
        }
    }

    fn state_after_parse(
        input: &'a Input,
        session: &'tcx Session,
        out_dir: &'a Option<PathBuf>,
        out_file: &'a Option<PathBuf>,
        krate: ast::Crate,
        cstore: &'tcx CStore,
    ) -> Self {
        CompileState {
            // Initialize the registry before moving `krate`
            registry: Some(Registry::new(&session, krate.span)),
            krate: Some(krate),
            cstore: Some(cstore),
            out_file: out_file.as_ref().map(|s| &**s),
            ..CompileState::empty(input, session, out_dir)
        }
    }

    fn state_after_expand(
        input: &'a Input,
        session: &'tcx Session,
        out_dir: &'a Option<PathBuf>,
        out_file: &'a Option<PathBuf>,
        cstore: &'tcx CStore,
        expanded_crate: &'a ast::Crate,
        crate_name: &'a str,
    ) -> Self {
        CompileState {
            crate_name: Some(crate_name),
            cstore: Some(cstore),
            expanded_crate: Some(expanded_crate),
            out_file: out_file.as_ref().map(|s| &**s),
            ..CompileState::empty(input, session, out_dir)
        }
    }

    fn state_after_hir_lowering(
        input: &'a Input,
        session: &'tcx Session,
        out_dir: &'a Option<PathBuf>,
        out_file: &'a Option<PathBuf>,
        cstore: &'tcx CStore,
        hir_map: &'a hir_map::Map<'tcx>,
        resolutions: &'a Resolutions,
        krate: &'a ast::Crate,
        hir_crate: &'a hir::Crate,
        output_filenames: &'a OutputFilenames,
        crate_name: &'a str,
    ) -> Self {
        CompileState {
            crate_name: Some(crate_name),
            cstore: Some(cstore),
            hir_map: Some(hir_map),
            resolutions: Some(resolutions),
            expanded_crate: Some(krate),
            hir_crate: Some(hir_crate),
            output_filenames: Some(output_filenames),
            out_file: out_file.as_ref().map(|s| &**s),
            ..CompileState::empty(input, session, out_dir)
        }
    }

    fn state_after_analysis(
        input: &'a Input,
        session: &'tcx Session,
        out_dir: &'a Option<PathBuf>,
        out_file: &'a Option<PathBuf>,
        krate: Option<&'a ast::Crate>,
        hir_crate: &'a hir::Crate,
        tcx: TyCtxt<'a, 'tcx, 'tcx>,
        crate_name: &'a str,
    ) -> Self {
        CompileState {
            tcx: Some(tcx),
            expanded_crate: krate,
            hir_crate: Some(hir_crate),
            crate_name: Some(crate_name),
            out_file: out_file.as_ref().map(|s| &**s),
            ..CompileState::empty(input, session, out_dir)
        }
    }

    fn state_when_compilation_done(
        input: &'a Input,
        session: &'tcx Session,
        out_dir: &'a Option<PathBuf>,
        out_file: &'a Option<PathBuf>,
    ) -> Self {
        CompileState {
            out_file: out_file.as_ref().map(|s| &**s),
            ..CompileState::empty(input, session, out_dir)
        }
    }
}

pub fn phase_1_parse_input<'a>(
    control: &CompileController,
    sess: &'a Session,
    input: &Input,
) -> PResult<'a, ast::Crate> {
    sess.diagnostic()
        .set_continue_after_error(control.continue_parse_after_error);
    hygiene::set_default_edition(sess.edition());

    if sess.profile_queries() {
        profile::begin(sess);
    }

    sess.profiler(|p| p.start_activity(ProfileCategory::Parsing));
    let krate = time(sess, "parsing", || match *input {
        Input::File(ref file) => parse::parse_crate_from_file(file, &sess.parse_sess),
        Input::Str {
            ref input,
            ref name,
        } => parse::parse_crate_from_source_str(name.clone(), input.clone(), &sess.parse_sess),
    })?;
    sess.profiler(|p| p.end_activity(ProfileCategory::Parsing));

    sess.diagnostic().set_continue_after_error(true);

    if sess.opts.debugging_opts.ast_json_noexpand {
        println!("{}", json::as_json(&krate));
    }

    if sess.opts.debugging_opts.input_stats {
        println!(
            "Lines of code:             {}",
            sess.source_map().count_lines()
        );
        println!("Pre-expansion node count:  {}", count_nodes(&krate));
    }

    if let Some(ref s) = sess.opts.debugging_opts.show_span {
        syntax::show_span::run(sess.diagnostic(), s, &krate);
    }

    if sess.opts.debugging_opts.hir_stats {
        hir_stats::print_ast_stats(&krate, "PRE EXPANSION AST STATS");
    }

    Ok(krate)
}

fn count_nodes(krate: &ast::Crate) -> usize {
    let mut counter = NodeCounter::new();
    visit::walk_crate(&mut counter, krate);
    counter.count
}

// For continuing compilation after a parsed crate has been
// modified

pub struct ExpansionResult {
    pub expanded_crate: ast::Crate,
    pub defs: hir_map::Definitions,
    pub resolutions: Resolutions,
    pub hir_forest: hir_map::Forest,
}

pub struct InnerExpansionResult<'a> {
    pub expanded_crate: ast::Crate,
    pub resolver: Resolver<'a>,
    pub hir_forest: hir_map::Forest,
}

/// Runs the "early phases" of the compiler: initial `cfg` processing,
/// loading compiler plugins (including those from `addl_plugins`),
/// syntax expansion, secondary `cfg` expansion, synthesis of a test
/// harness if one is to be provided, injection of a dependency on the
/// standard library and prelude, and name resolution.
///
/// Returns `None` if we're aborting after handling -W help.
pub fn phase_2_configure_and_expand<F>(
    sess: &Session,
    cstore: &CStore,
    krate: ast::Crate,
    registry: Option<Registry>,
    crate_name: &str,
    addl_plugins: Option<Vec<String>>,
    after_expand: F,
) -> Result<ExpansionResult, CompileIncomplete>
where
    F: FnOnce(&ast::Crate) -> CompileResult,
{
    // Currently, we ignore the name resolution data structures for the purposes of dependency
    // tracking. Instead we will run name resolution and include its output in the hash of each
    // item, much like we do for macro expansion. In other words, the hash reflects not just
    // its contents but the results of name resolution on those contents. Hopefully we'll push
    // this back at some point.
    let mut crate_loader = CrateLoader::new(sess, &cstore, &crate_name);
    let resolver_arenas = Resolver::arenas();
    let result = phase_2_configure_and_expand_inner(
        sess,
        cstore,
        krate,
        registry,
        crate_name,
        addl_plugins,
        &resolver_arenas,
        &mut crate_loader,
        after_expand,
    );
    match result {
        Ok(InnerExpansionResult {
            expanded_crate,
            resolver,
            hir_forest,
        }) => Ok(ExpansionResult {
            expanded_crate,
            defs: resolver.definitions,
            hir_forest,
            resolutions: Resolutions {
                freevars: resolver.freevars,
                export_map: resolver.export_map,
                trait_map: resolver.trait_map,
                glob_map: resolver.glob_map,
                maybe_unused_trait_imports: resolver.maybe_unused_trait_imports,
                maybe_unused_extern_crates: resolver.maybe_unused_extern_crates,
                extern_prelude: resolver.extern_prelude.iter().map(|(ident, entry)| {
                    (ident.name, entry.introduced_by_item)
                }).collect(),
            },
        }),
        Err(x) => Err(x),
    }
}

/// Same as phase_2_configure_and_expand, but doesn't let you keep the resolver
/// around
pub fn phase_2_configure_and_expand_inner<'a, F>(
    sess: &'a Session,
    cstore: &'a CStore,
    mut krate: ast::Crate,
    registry: Option<Registry>,
    crate_name: &str,
    addl_plugins: Option<Vec<String>>,
    resolver_arenas: &'a ResolverArenas<'a>,
    crate_loader: &'a mut CrateLoader<'a>,
    after_expand: F,
) -> Result<InnerExpansionResult<'a>, CompileIncomplete>
where
    F: FnOnce(&ast::Crate) -> CompileResult,
{
    krate = time(sess, "attributes injection", || {
        syntax::attr::inject(krate, &sess.parse_sess, &sess.opts.debugging_opts.crate_attr)
    });

    let (mut krate, features) = syntax::config::features(
        krate,
        &sess.parse_sess,
        sess.edition(),
    );
    // these need to be set "early" so that expansion sees `quote` if enabled.
    sess.init_features(features);

    let crate_types = util::collect_crate_types(sess, &krate.attrs);
    sess.crate_types.set(crate_types);

    let disambiguator = util::compute_crate_disambiguator(sess);
    sess.crate_disambiguator.set(disambiguator);
    rustc_incremental::prepare_session_directory(sess, &crate_name, disambiguator);

    if sess.opts.incremental.is_some() {
        time(sess, "garbage collect incremental cache directory", || {
            if let Err(e) = rustc_incremental::garbage_collect_session_directories(sess) {
                warn!(
                    "Error while trying to garbage collect incremental \
                     compilation cache directory: {}",
                    e
                );
            }
        });
    }

    // If necessary, compute the dependency graph (in the background).
    let future_dep_graph = if sess.opts.build_dep_graph() {
        Some(rustc_incremental::load_dep_graph(sess))
    } else {
        None
    };

    time(sess, "recursion limit", || {
        middle::recursion_limit::update_limits(sess, &krate);
    });

    krate = time(sess, "crate injection", || {
        let alt_std_name = sess.opts.alt_std_name.as_ref().map(|s| &**s);
        syntax::std_inject::maybe_inject_crates_ref(krate, alt_std_name, sess.edition())
    });

    let mut addl_plugins = Some(addl_plugins);
    let registrars = time(sess, "plugin loading", || {
        plugin::load::load_plugins(
            sess,
            &cstore,
            &krate,
            crate_name,
            addl_plugins.take().unwrap(),
        )
    });

    let mut registry = registry.unwrap_or_else(|| Registry::new(sess, krate.span));

    time(sess, "plugin registration", || {
        if sess.features_untracked().rustc_diagnostic_macros {
            registry.register_macro(
                "__diagnostic_used",
                diagnostics::plugin::expand_diagnostic_used,
            );
            registry.register_macro(
                "__register_diagnostic",
                diagnostics::plugin::expand_register_diagnostic,
            );
            registry.register_macro(
                "__build_diagnostic_array",
                diagnostics::plugin::expand_build_diagnostic_array,
            );
        }

        for registrar in registrars {
            registry.args_hidden = Some(registrar.args);
            (registrar.fun)(&mut registry);
        }
    });

    let Registry {
        syntax_exts,
        early_lint_passes,
        late_lint_passes,
        lint_groups,
        llvm_passes,
        attributes,
        ..
    } = registry;

    sess.track_errors(|| {
        let mut ls = sess.lint_store.borrow_mut();
        for pass in early_lint_passes {
            ls.register_early_pass(Some(sess), true, false, pass);
        }
        for pass in late_lint_passes {
            ls.register_late_pass(Some(sess), true, pass);
        }

        for (name, (to, deprecated_name)) in lint_groups {
            ls.register_group(Some(sess), true, name, deprecated_name, to);
        }

        *sess.plugin_llvm_passes.borrow_mut() = llvm_passes;
        *sess.plugin_attributes.borrow_mut() = attributes.clone();
    })?;

    // Lint plugins are registered; now we can process command line flags.
    if sess.opts.describe_lints {
        super::describe_lints(&sess, &sess.lint_store.borrow(), true);
        return Err(CompileIncomplete::Stopped);
    }

    time(sess, "pre ast expansion lint checks", || {
        lint::check_ast_crate(
            sess,
            &krate,
            true,
            rustc_lint::BuiltinCombinedPreExpansionLintPass::new());
    });

    let mut resolver = Resolver::new(
        sess,
        cstore,
        &krate,
        crate_name,
        crate_loader,
        &resolver_arenas,
    );
    syntax_ext::register_builtins(&mut resolver, syntax_exts);

    // Expand all macros
    sess.profiler(|p| p.start_activity(ProfileCategory::Expansion));
    krate = time(sess, "expansion", || {
        // Windows dlls do not have rpaths, so they don't know how to find their
        // dependencies. It's up to us to tell the system where to find all the
        // dependent dlls. Note that this uses cfg!(windows) as opposed to
        // targ_cfg because syntax extensions are always loaded for the host
        // compiler, not for the target.
        //
        // This is somewhat of an inherently racy operation, however, as
        // multiple threads calling this function could possibly continue
        // extending PATH far beyond what it should. To solve this for now we
        // just don't add any new elements to PATH which are already there
        // within PATH. This is basically a targeted fix at #17360 for rustdoc
        // which runs rustc in parallel but has been seen (#33844) to cause
        // problems with PATH becoming too long.
        let mut old_path = OsString::new();
        if cfg!(windows) {
            old_path = env::var_os("PATH").unwrap_or(old_path);
            let mut new_path = sess.host_filesearch(PathKind::All).search_path_dirs();
            for path in env::split_paths(&old_path) {
                if !new_path.contains(&path) {
                    new_path.push(path);
                }
            }
            env::set_var(
                "PATH",
                &env::join_paths(
                    new_path
                        .iter()
                        .filter(|p| env::join_paths(iter::once(p)).is_ok()),
                ).unwrap(),
            );
        }

        // Create the config for macro expansion
        let features = sess.features_untracked();
        let cfg = syntax::ext::expand::ExpansionConfig {
            features: Some(&features),
            recursion_limit: *sess.recursion_limit.get(),
            trace_mac: sess.opts.debugging_opts.trace_macros,
            should_test: sess.opts.test,
            ..syntax::ext::expand::ExpansionConfig::default(crate_name.to_string())
        };

        let mut ecx = ExtCtxt::new(&sess.parse_sess, cfg, &mut resolver);

        // Expand macros now!
        let krate = time(sess, "expand crate", || {
            ecx.monotonic_expander().expand_crate(krate)
        });

        // The rest is error reporting

        time(sess, "check unused macros", || {
            ecx.check_unused_macros();
        });

        let mut missing_fragment_specifiers: Vec<_> = ecx.parse_sess
            .missing_fragment_specifiers
            .borrow()
            .iter()
            .cloned()
            .collect();
        missing_fragment_specifiers.sort();

        for span in missing_fragment_specifiers {
            let lint = lint::builtin::MISSING_FRAGMENT_SPECIFIER;
            let msg = "missing fragment specifier";
            sess.buffer_lint(lint, ast::CRATE_NODE_ID, span, msg);
        }
        if cfg!(windows) {
            env::set_var("PATH", &old_path);
        }
        krate
    });
    sess.profiler(|p| p.end_activity(ProfileCategory::Expansion));

    time(sess, "maybe building test harness", || {
        syntax::test::modify_for_testing(
            &sess.parse_sess,
            &mut resolver,
            sess.opts.test,
            &mut krate,
            sess.diagnostic(),
            &sess.features_untracked(),
        )
    });

    // If we're actually rustdoc then there's no need to actually compile
    // anything, so switch everything to just looping
    if sess.opts.actually_rustdoc {
        util::ReplaceBodyWithLoop::new(sess).visit_crate(&mut krate);
    }

    let (has_proc_macro_decls, has_global_allocator) = time(sess, "AST validation", || {
        ast_validation::check_crate(sess, &krate)
    });

    // If we're in rustdoc we're always compiling as an rlib, but that'll trip a
    // bunch of checks in the `modify` function below. For now just skip this
    // step entirely if we're rustdoc as it's not too useful anyway.
    if !sess.opts.actually_rustdoc {
        krate = time(sess, "maybe creating a macro crate", || {
            let crate_types = sess.crate_types.borrow();
            let num_crate_types = crate_types.len();
            let is_proc_macro_crate = crate_types.contains(&config::CrateType::ProcMacro);
            let is_test_crate = sess.opts.test;
            syntax_ext::proc_macro_decls::modify(
                &sess.parse_sess,
                &mut resolver,
                krate,
                is_proc_macro_crate,
                has_proc_macro_decls,
                is_test_crate,
                num_crate_types,
                sess.diagnostic(),
            )
        });
    }

    if has_global_allocator {
        // Expand global allocators, which are treated as an in-tree proc macro
        time(sess, "creating allocators", || {
            allocator::expand::modify(
                &sess.parse_sess,
                &mut resolver,
                &mut krate,
                crate_name.to_string(),
                sess.diagnostic(),
            )
        });
    }

    // Done with macro expansion!

    after_expand(&krate)?;

    if sess.opts.debugging_opts.input_stats {
        println!("Post-expansion node count: {}", count_nodes(&krate));
    }

    if sess.opts.debugging_opts.hir_stats {
        hir_stats::print_ast_stats(&krate, "POST EXPANSION AST STATS");
    }

    if sess.opts.debugging_opts.ast_json {
        println!("{}", json::as_json(&krate));
    }

    time(sess, "name resolution", || {
        resolver.resolve_crate(&krate);
    });

    // Needs to go *after* expansion to be able to check the results of macro expansion.
    time(sess, "complete gated feature checking", || {
        syntax::feature_gate::check_crate(
            &krate,
            &sess.parse_sess,
            &sess.features_untracked(),
            &attributes,
            sess.opts.unstable_features,
        );
    });

    // Add all buffered lints from the `ParseSess` to the `Session`.
    sess.parse_sess.buffered_lints.with_lock(|buffered_lints| {
        info!("{} parse sess buffered_lints", buffered_lints.len());
        for BufferedEarlyLint{id, span, msg, lint_id} in buffered_lints.drain(..) {
            let lint = lint::Lint::from_parser_lint_id(lint_id);
            sess.buffer_lint(lint, id, span, &msg);
        }
    });

    // Lower ast -> hir.
    // First, we need to collect the dep_graph.
    let dep_graph = match future_dep_graph {
        None => DepGraph::new_disabled(),
        Some(future) => {
            let (prev_graph, prev_work_products) =
                time(sess, "blocked while dep-graph loading finishes", || {
                    future
                        .open()
                        .unwrap_or_else(|e| rustc_incremental::LoadResult::Error {
                            message: format!("could not decode incremental cache: {:?}", e),
                        })
                        .open(sess)
                });
            DepGraph::new(prev_graph, prev_work_products)
        }
    };
    let hir_forest = time(sess, "lowering ast -> hir", || {
        let hir_crate = lower_crate(sess, cstore, &dep_graph, &krate, &mut resolver);

        if sess.opts.debugging_opts.hir_stats {
            hir_stats::print_hir_stats(&hir_crate);
        }

        hir_map::Forest::new(hir_crate, &dep_graph)
    });

    time(sess, "early lint checks", || {
        lint::check_ast_crate(sess, &krate, false, rustc_lint::BuiltinCombinedEarlyLintPass::new())
    });

    // Discard hygiene data, which isn't required after lowering to HIR.
    if !sess.opts.debugging_opts.keep_hygiene_data {
        syntax::ext::hygiene::clear_markings();
    }

    Ok(InnerExpansionResult {
        expanded_crate: krate,
        resolver,
        hir_forest,
    })
}

pub fn default_provide(providers: &mut ty::query::Providers) {
    rustc_interface::passes::provide(providers);
    plugin::build::provide(providers);
    hir::provide(providers);
    borrowck::provide(providers);
    mir::provide(providers);
    reachable::provide(providers);
    resolve_lifetime::provide(providers);
    rustc_privacy::provide(providers);
    typeck::provide(providers);
    ty::provide(providers);
    traits::provide(providers);
    stability::provide(providers);
    middle::intrinsicck::provide(providers);
    middle::liveness::provide(providers);
    reachable::provide(providers);
    rustc_passes::provide(providers);
    rustc_traits::provide(providers);
    middle::region::provide(providers);
    middle::entry::provide(providers);
    cstore::provide(providers);
    lint::provide(providers);
}

pub fn default_provide_extern(providers: &mut ty::query::Providers) {
    cstore::provide_extern(providers);
}

/// Runs the resolution, type-checking, region checking and other
/// miscellaneous analysis passes on the crate. Return various
/// structures carrying the results of the analysis.
pub fn phase_3_run_analysis_passes<'tcx, F, R>(
    codegen_backend: &dyn CodegenBackend,
    control: &CompileController,
    sess: &'tcx Session,
    cstore: &'tcx CStore,
    hir_map: hir_map::Map<'tcx>,
    resolutions: Resolutions,
    arenas: &'tcx mut AllArenas<'tcx>,
    name: &str,
    output_filenames: &OutputFilenames,
    f: F,
) -> R
where
    F: for<'a> FnOnce(
        TyCtxt<'a, 'tcx, 'tcx>,
        mpsc::Receiver<Box<dyn Any + Send>>,
        CompileResult,
    ) -> R,
{
    let query_result_on_disk_cache = time(sess, "load query result cache", || {
        rustc_incremental::load_query_result_cache(sess)
    });

    let mut local_providers = ty::query::Providers::default();
    default_provide(&mut local_providers);
    codegen_backend.provide(&mut local_providers);
    (control.provide)(&mut local_providers);

    let mut extern_providers = local_providers;
    default_provide_extern(&mut extern_providers);
    codegen_backend.provide_extern(&mut extern_providers);
    (control.provide_extern)(&mut extern_providers);

    let (tx, rx) = mpsc::channel();

    TyCtxt::create_and_enter(
        sess,
        cstore,
        local_providers,
        extern_providers,
        arenas,
        resolutions,
        hir_map,
        query_result_on_disk_cache,
        name,
        tx,
        output_filenames,
        |tcx| {
            // Do some initialization of the DepGraph that can only be done with the
            // tcx available.
            time(sess, "dep graph tcx init", || rustc_incremental::dep_graph_tcx_init(tcx));

            tcx.analysis(LOCAL_CRATE).ok();

            f(tcx, rx, tcx.sess.compile_status())
        },
    )
}

/// Runs the codegen backend, after which the AST and analysis can
/// be discarded.
pub fn phase_4_codegen<'a, 'tcx>(
    codegen_backend: &dyn CodegenBackend,
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    rx: mpsc::Receiver<Box<dyn Any + Send>>,
) -> Box<dyn Any> {
    time(tcx.sess, "resolving dependency formats", || {
        ::rustc::middle::dependency_format::calculate(tcx)
    });

    tcx.sess.profiler(|p| p.start_activity(ProfileCategory::Codegen));
    let codegen = time(tcx.sess, "codegen", move || codegen_backend.codegen_crate(tcx, rx));
    tcx.sess.profiler(|p| p.end_activity(ProfileCategory::Codegen));
    if tcx.sess.profile_queries() {
        profile::dump(&tcx.sess, "profile_queries".to_string())
    }

    codegen
}
